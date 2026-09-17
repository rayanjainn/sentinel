//! The core runtime: owns the providers, runs the sampler thread, and answers `SystemQueries`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

use crate::error::{CoreResult, SentinelError};
use crate::events::{CoreEvent, EventSink, SamplingConfig, StreamKind};
use crate::model::*;
use crate::platform::{self, PlatformConfig, Providers};
use crate::provider::{PermissionProbe, ProcessControl, ProcessProvider, ResourceProvider};
use crate::service::history::{ProcessHistory, ResourceHistory};
use crate::service::sampling::clamp_config;
use crate::service::{FileFilter, SystemQueries};

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// App data directory (audit database, firewall rules, geo database, settings).
    pub data_dir: std::path::PathBuf,
}

struct ResourceState {
    provider: Box<dyn ResourceProvider>,
    history: ResourceHistory,
    sampled_at: Option<Instant>,
}

struct ProcessState {
    provider: Box<dyn ProcessProvider>,
    last: Option<(Instant, ProcessSnapshot)>,
    history: ProcessHistory,
}

struct Shared {
    sink: Arc<dyn EventSink>,
    config: Mutex<SamplingConfig>,
    wake: Condvar,
    shutdown: AtomicBool,
    system_info: SystemInfo,
    resources: Mutex<ResourceState>,
    processes: Mutex<ProcessState>,
    process_control: Arc<dyn ProcessControl>,
    permissions: Arc<dyn PermissionProbe>,
}

pub struct CoreRuntime {
    shared: Arc<Shared>,
    sampler: Mutex<Option<JoinHandle<()>>>,
}

impl CoreRuntime {
    pub fn start(config: RuntimeConfig, sink: Arc<dyn EventSink>) -> CoreResult<Arc<Self>> {
        std::fs::create_dir_all(&config.data_dir)
            .map_err(|err| SentinelError::io(&err, Some(&config.data_dir)))?;
        let Providers {
            resources,
            processes,
            process_control,
            permissions,
        } = platform::current(&PlatformConfig {
            data_dir: config.data_dir.clone(),
        });
        let system_info = resources.system_info();
        let shared = Arc::new(Shared {
            sink,
            config: Mutex::new(SamplingConfig::default()),
            wake: Condvar::new(),
            shutdown: AtomicBool::new(false),
            system_info,
            resources: Mutex::new(ResourceState {
                provider: resources,
                history: ResourceHistory::default(),
                sampled_at: None,
            }),
            processes: Mutex::new(ProcessState {
                provider: processes,
                last: None,
                history: ProcessHistory::default(),
            }),
            process_control,
            permissions,
        });
        let runtime = Arc::new(Self {
            shared: Arc::clone(&shared),
            sampler: Mutex::new(None),
        });
        let handle = std::thread::Builder::new()
            .name("sentinel-sampler".into())
            .spawn(move || sampler_loop(&shared))
            .map_err(|err| SentinelError::internal(format!("could not start sampler: {err}")))?;
        *runtime.sampler.lock() = Some(handle);
        Ok(runtime)
    }

    pub fn sampling(&self) -> SamplingConfig {
        self.shared.config.lock().clone()
    }

    /// Applies a new subscription set; takes effect on the next tick, which starts immediately.
    pub fn set_sampling(&self, config: SamplingConfig) -> SamplingConfig {
        let clamped = clamp_config(config);
        *self.shared.config.lock() = clamped.clone();
        self.shared.wake.notify_all();
        clamped
    }

    pub fn process_control(&self) -> Arc<dyn ProcessControl> {
        Arc::clone(&self.shared.process_control)
    }
}

impl Drop for CoreRuntime {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::SeqCst);
        self.shared.wake.notify_all();
        if let Some(handle) = self.sampler.lock().take() {
            let _ = handle.join();
        }
    }
}

fn sampler_loop(shared: &Shared) {
    let mut pending_wake = false;
    while !shared.shutdown.load(Ordering::SeqCst) {
        let tick_started = Instant::now();
        let config = shared.config.lock().clone();

        // Resources are always sampled (cheap) so 15 minutes of history exist when a view opens.
        if let Ok(sample) = sample_resources(shared)
            && config.streams.contains(&StreamKind::Resources)
        {
            shared.sink.emit(CoreEvent::Resources(sample));
        }
        if config.streams.contains(&StreamKind::Processes)
            && let Ok(snapshot) = refresh_processes(shared)
        {
            shared.sink.emit(CoreEvent::Processes(snapshot));
        }

        let deadline = tick_started + Duration::from_millis(u64::from(config.interval_ms));
        let mut guard = shared.config.lock();
        while !shared.shutdown.load(Ordering::SeqCst) {
            if *guard != config {
                // A subscription change runs a tick right away, but never faster than the CPU
                // usage counters can resolve.
                pending_wake = true;
                break;
            }
            if shared.wake.wait_until(&mut guard, deadline).timed_out() {
                break;
            }
        }
        drop(guard);
        if pending_wake {
            pending_wake = false;
            let min_gap = Duration::from_millis(250);
            let elapsed = tick_started.elapsed();
            if elapsed < min_gap {
                std::thread::sleep(min_gap - elapsed);
            }
        }
    }
}

fn sample_resources(shared: &Shared) -> CoreResult<ResourceSample> {
    let mut state = shared.resources.lock();
    let sample = state.provider.sample()?;
    state.history.push(sample.clone());
    state.sampled_at = Some(Instant::now());
    Ok(sample)
}

fn refresh_processes(shared: &Shared) -> CoreResult<ProcessSnapshot> {
    let mut state = shared.processes.lock();
    let snapshot = state.provider.refresh()?;
    state.history.record(&snapshot);
    state.last = Some((Instant::now(), snapshot.clone()));
    Ok(snapshot)
}

fn interval(shared: &Shared) -> Duration {
    Duration::from_millis(u64::from(shared.config.lock().interval_ms))
}

fn pending(feature: &str) -> SentinelError {
    SentinelError::Unavailable {
        feature: feature.to_owned(),
        reason: "this part of the core runtime has not been enabled in this build".to_owned(),
    }
}

impl SystemQueries for CoreRuntime {
    fn system_info(&self) -> SystemInfo {
        self.shared.system_info.clone()
    }

    fn permission_status(&self) -> PermissionStatus {
        self.shared.permissions.status()
    }

    fn resource_sample(&self) -> CoreResult<ResourceSample> {
        let fresh_for = interval(&self.shared);
        {
            let state = self.shared.resources.lock();
            if let (Some(at), Some(latest)) = (state.sampled_at, state.history.latest())
                && at.elapsed() <= fresh_for
            {
                return Ok(latest.clone());
            }
        }
        sample_resources(&self.shared)
    }

    fn resource_history(&self, window_secs: u32) -> Vec<ResourceSample> {
        self.shared.resources.lock().history.window(window_secs)
    }

    fn process_snapshot(&self) -> CoreResult<ProcessSnapshot> {
        let fresh_for = interval(&self.shared);
        {
            let state = self.shared.processes.lock();
            if let Some((at, snapshot)) = &state.last
                && at.elapsed() <= fresh_for
            {
                return Ok(snapshot.clone());
            }
        }
        refresh_processes(&self.shared)
    }

    fn process_detail(&self, pid: Pid) -> CoreResult<ProcessDetail> {
        let (info, open_files) = {
            let mut state = self.shared.processes.lock();
            let info = state.provider.lookup(pid)?;
            (info, state.provider.open_files(pid))
        };
        let (open_files, open_files_error) = match open_files {
            Ok(files) => (files, None),
            Err(err) => (Vec::new(), Some(err.into())),
        };
        Ok(ProcessDetail {
            has_window: self.shared.process_control.has_window(pid),
            info,
            open_files,
            open_files_error,
            connections: Vec::new(),
        })
    }

    fn process_history(&self, pid: Pid) -> Vec<ProcessHistoryPoint> {
        self.shared.processes.lock().history.points(pid)
    }

    fn network_snapshot(&self) -> CoreResult<NetworkSnapshot> {
        Err(pending("network monitoring"))
    }

    fn firewall_status(&self) -> FirewallStatus {
        self.shared.permissions.status().firewall
    }

    fn firewall_rules(&self) -> CoreResult<Vec<FirewallRule>> {
        Err(pending("firewall rules"))
    }

    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>> {
        Err(pending("storage volumes"))
    }

    fn start_scan(&self, _request: ScanRequest) -> CoreResult<ScanId> {
        Err(pending("storage scan"))
    }

    fn cancel_scan(&self, _scan_id: &str) -> CoreResult<()> {
        Err(pending("storage scan"))
    }

    fn wait_for_scan(&self, _scan_id: &str, _timeout: Duration) -> CoreResult<ScanSummary> {
        Err(pending("storage scan"))
    }

    fn scan_covering(&self, _path: &str) -> Option<ScanId> {
        None
    }

    fn scan_summary(&self, _scan_id: &str) -> CoreResult<ScanSummary> {
        Err(pending("storage scan"))
    }

    fn scan_tree(&self, _query: &TreeQuery) -> CoreResult<TreeNode> {
        Err(pending("storage scan"))
    }

    fn find_files(&self, _scan_id: &str, _filter: &FileFilter) -> CoreResult<Vec<FileEntry>> {
        Err(pending("storage scan"))
    }

    fn start_duplicate_scan(&self, _scan_id: &str, _min_size_bytes: u64) -> CoreResult<JobId> {
        Err(pending("duplicate detection"))
    }

    fn cancel_duplicate_scan(&self, _job_id: &str) -> CoreResult<()> {
        Err(pending("duplicate detection"))
    }

    fn wait_for_duplicates(
        &self,
        _job_id: &str,
        _timeout: Duration,
    ) -> CoreResult<DuplicateReport> {
        Err(pending("duplicate detection"))
    }
}

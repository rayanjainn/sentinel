//! The core runtime: owns the providers, runs the sampler thread, and answers `SystemQueries`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

use crate::error::{CoreResult, SentinelError};
use crate::events::{CoreEvent, EventSink, SamplingConfig, StreamKind};
use crate::model::*;
use crate::platform::{self, PlatformConfig, ProcessNames, Providers};
use crate::provider::{
    FileOps, PermissionProbe, ProcessControl, ProcessProvider, ResourceProvider,
};
use crate::service::actions::{ActionContext, PathSize};
use crate::service::history::{ProcessHistory, ResourceHistory};
use crate::service::network::NetworkMonitor;
use crate::service::network::dns::ReverseDns;
use crate::service::network::geo::GeoDb;
use crate::service::network::home::HomeLocator;
use crate::service::sampling::clamp_config;
use crate::service::storage::StorageService;
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

struct NetworkState {
    monitor: NetworkMonitor,
    last: Option<(Instant, NetworkSnapshot)>,
}

struct Shared {
    sink: Arc<dyn EventSink>,
    config: Mutex<SamplingConfig>,
    wake: Condvar,
    shutdown: AtomicBool,
    system_info: SystemInfo,
    resources: Mutex<ResourceState>,
    processes: Mutex<ProcessState>,
    network: Mutex<NetworkState>,
    process_names: Mutex<ProcessNames>,
    geo: Arc<GeoDb>,
    home: HomeLocator,
    process_control: Arc<dyn ProcessControl>,
    permissions: Arc<dyn PermissionProbe>,
    file_ops: Arc<dyn FileOps>,
    storage: Arc<StorageService>,
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
            network,
            process_control,
            permissions,
            file_ops,
            storage,
        } = platform::current(&PlatformConfig {
            data_dir: config.data_dir.clone(),
        });
        let system_info = resources.system_info();
        let geo = Arc::new(GeoDb::open(&config.data_dir.join("geo")));
        let dns = Arc::new(ReverseDns::new(Arc::clone(&sink)));
        let storage = StorageService::new(storage, Arc::clone(&sink));
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
            network: Mutex::new(NetworkState {
                monitor: NetworkMonitor::new(network, dns, Arc::clone(&geo)),
                last: None,
            }),
            process_names: Mutex::new(ProcessNames::default()),
            geo,
            home: HomeLocator::new(
                config.data_dir.join("settings").join("home_location.json"),
                Box::new(platform::system_time_zone),
            ),
            process_control,
            permissions,
            file_ops,
            storage,
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

    pub fn file_ops(&self) -> Arc<dyn FileOps> {
        Arc::clone(&self.shared.file_ops)
    }

    pub fn reveal_path(&self, path: &str) -> CoreResult<()> {
        if path.trim().is_empty() {
            return Err(SentinelError::invalid("no path given"));
        }
        self.shared.file_ops.reveal(std::path::Path::new(path))
    }

    pub fn reveal_process_executable(&self, pid: Pid) -> CoreResult<()> {
        let info = self.lookup_process(pid)?;
        let exe = info.exe.ok_or_else(|| SentinelError::Unavailable {
            feature: "reveal executable".to_owned(),
            reason: format!(
                "the executable path of {} (PID {pid}) is not readable without administrator privileges",
                info.name
            ),
        })?;
        self.shared.file_ops.reveal(std::path::Path::new(&exe))
    }

    pub fn storage(&self) -> Arc<StorageService> {
        Arc::clone(&self.shared.storage)
    }

    /// Report of a finished duplicate job; `Unavailable` while still hashing.
    pub fn duplicate_report(&self, job_id: &str) -> CoreResult<DuplicateReport> {
        self.shared
            .storage
            .wait_for_duplicates(job_id, Duration::ZERO)
    }

    pub fn geo_db_status(&self) -> GeoDbStatus {
        self.shared.geo.status()
    }

    /// Starts the geolocation database download in the background; progress arrives as
    /// `sentinel:geo-db` events.
    pub fn download_geo_db(&self) -> CoreResult<()> {
        #[cfg(feature = "native")]
        {
            if self.shared.geo.is_downloading() {
                return Ok(());
            }
            self.shared.geo.set_downloading(0, None);
            let shared = Arc::clone(&self.shared);
            std::thread::Builder::new()
                .name("sentinel-geo-download".into())
                .spawn(move || {
                    let _ = crate::service::network::download::download(
                        &shared.geo,
                        shared.sink.as_ref(),
                    );
                })
                .map_err(|err| {
                    SentinelError::internal(format!("could not start download: {err}"))
                })?;
            Ok(())
        }
        #[cfg(not(feature = "native"))]
        {
            Err(SentinelError::Unavailable {
                feature: "geolocation database download".to_owned(),
                reason: "this build was compiled without network download support".to_owned(),
            })
        }
    }

    pub fn home_location(&self) -> CoreResult<HomeLocation> {
        self.shared.home.get()
    }

    pub fn set_home_location(
        &self,
        location: Option<HomeLocationInput>,
    ) -> CoreResult<HomeLocation> {
        self.shared.home.set(location)
    }

    pub fn open_permission_settings(&self, kind: PermissionKind) -> CoreResult<()> {
        self.shared.permissions.open_settings(kind)
    }
}

impl ActionContext for CoreRuntime {
    fn lookup_process(&self, pid: Pid) -> CoreResult<ProcessInfo> {
        self.shared.processes.lock().provider.lookup(pid)
    }

    fn file_ops(&self) -> CoreResult<Arc<dyn FileOps>> {
        Ok(Arc::clone(&self.shared.file_ops))
    }

    fn path_size(&self, path: &std::path::Path) -> Option<PathSize> {
        let storage = &self.shared.storage;
        if let Some(result) = storage.tree_covering(path)
            && let Some(id) = result.tree.find(path)
            && let Some(node) = result.tree.node(id)
        {
            return Some(PathSize {
                bytes: node.size,
                items: node.items,
                modified: node.modified,
                complete: true,
            });
        }
        Some(storage.measure_path(path, Duration::from_millis(1500)))
    }

    fn volume_usage(&self, path: &std::path::Path) -> CoreResult<(u64, u64)> {
        self.shared.storage.provider().volume_usage(path)
    }

    fn volume_label(&self, path: &std::path::Path) -> Option<String> {
        let volumes = self.shared.storage.volumes().ok()?;
        volumes
            .into_iter()
            .filter(|v| path.starts_with(&v.mount_point))
            .max_by_key(|v| v.mount_point.len())
            .map(|v| v.name)
    }

    fn same_volume(&self, a: &std::path::Path, b: &std::path::Path) -> Option<bool> {
        let provider = self.shared.storage.provider();
        let device_a = provider.metadata(a).ok()?.device;
        let device_b = provider.metadata(b).ok()?.device;
        Some(provider.volume_group(device_a).contains(&device_b))
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
        if config.streams.contains(&StreamKind::Network)
            && let Ok(snapshot) = sample_network(shared)
        {
            shared.sink.emit(CoreEvent::Network(snapshot));
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

fn sample_network(shared: &Shared) -> CoreResult<NetworkSnapshot> {
    let mut state = shared.network.lock();
    let mut names = |pids: &[Pid]| process_names(shared, pids);
    let snapshot = state.monitor.sample(&mut names)?;
    state.last = Some((Instant::now(), snapshot.clone()));
    Ok(snapshot)
}

/// Names from the sampled process table when it is fresh, otherwise from a light name-only scan.
fn process_names(shared: &Shared, pids: &[Pid]) -> std::collections::HashMap<Pid, String> {
    {
        let state = shared.processes.lock();
        if let Some((at, snapshot)) = &state.last
            && at.elapsed() <= Duration::from_secs(3)
        {
            let by_pid: std::collections::HashMap<Pid, &str> = snapshot
                .processes
                .iter()
                .map(|p| (p.pid, p.name.as_str()))
                .collect();
            if pids.iter().all(|pid| by_pid.contains_key(pid)) {
                return pids
                    .iter()
                    .filter_map(|pid| by_pid.get(pid).map(|name| (*pid, (*name).to_owned())))
                    .collect();
            }
        }
    }
    shared.process_names.lock().names(pids.iter().copied())
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
        let connections = self
            .network_snapshot()
            .map(|snapshot| {
                snapshot
                    .sockets
                    .into_iter()
                    .filter(|socket| socket.pid == Some(pid))
                    .collect()
            })
            .unwrap_or_default();
        Ok(ProcessDetail {
            has_window: self.shared.process_control.has_window(pid),
            info,
            open_files,
            open_files_error,
            connections,
        })
    }

    fn process_history(&self, pid: Pid) -> Vec<ProcessHistoryPoint> {
        self.shared.processes.lock().history.points(pid)
    }

    fn network_snapshot(&self) -> CoreResult<NetworkSnapshot> {
        let fresh_for = interval(&self.shared);
        {
            let state = self.shared.network.lock();
            if let Some((at, snapshot)) = &state.last
                && at.elapsed() <= fresh_for
            {
                return Ok(snapshot.clone());
            }
        }
        sample_network(&self.shared)
    }

    fn firewall_status(&self) -> FirewallStatus {
        self.shared.permissions.status().firewall
    }

    fn firewall_rules(&self) -> CoreResult<Vec<FirewallRule>> {
        Err(pending("firewall rules"))
    }

    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>> {
        self.shared.storage.volumes()
    }

    fn start_scan(&self, request: ScanRequest) -> CoreResult<ScanId> {
        self.shared.storage.start_scan(request)
    }

    fn cancel_scan(&self, scan_id: &str) -> CoreResult<()> {
        self.shared.storage.cancel_scan(scan_id)
    }

    fn wait_for_scan(&self, scan_id: &str, timeout: Duration) -> CoreResult<ScanSummary> {
        self.shared.storage.wait_for_scan(scan_id, timeout)
    }

    fn scan_covering(&self, path: &str) -> Option<ScanId> {
        self.shared.storage.scan_covering(path)
    }

    fn scan_summary(&self, scan_id: &str) -> CoreResult<ScanSummary> {
        self.shared.storage.scan_summary(scan_id)
    }

    fn scan_tree(&self, query: &TreeQuery) -> CoreResult<TreeNode> {
        self.shared.storage.scan_tree(query)
    }

    fn find_files(&self, scan_id: &str, filter: &FileFilter) -> CoreResult<Vec<FileEntry>> {
        self.shared.storage.find_files(scan_id, filter)
    }

    fn start_duplicate_scan(&self, scan_id: &str, min_size_bytes: u64) -> CoreResult<JobId> {
        self.shared
            .storage
            .start_duplicate_scan(scan_id, min_size_bytes)
    }

    fn cancel_duplicate_scan(&self, job_id: &str) -> CoreResult<()> {
        self.shared.storage.cancel_duplicate_scan(job_id)
    }

    fn wait_for_duplicates(&self, job_id: &str, timeout: Duration) -> CoreResult<DuplicateReport> {
        self.shared.storage.wait_for_duplicates(job_id, timeout)
    }
}

//! Storage scans, tree queries, cleanup suggestions and duplicate jobs over `StorageProvider`.

pub mod classify;
pub mod duplicates;
pub mod scanner;
pub mod summary;
pub mod tree;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::{Condvar, Mutex};

use self::duplicates::DuplicateCounters;
use self::scanner::{LiveNode, Progress, Walker};
use self::tree::{BuiltDir, ScanTree};
use crate::error::{CoreResult, SentinelError};
use crate::events::{CoreEvent, EventSink};
use crate::model::*;
use crate::provider::StorageProvider;
use crate::service::FileFilter;
use crate::util::{now_ms, now_secs};

const PROGRESS_EVERY: Duration = Duration::from_millis(250);
const PARTIAL_EVERY: Duration = Duration::from_secs(1);
/// Finished scans kept in memory for tree queries and actions.
const KEEP_SCANS: usize = 3;
const KEEP_JOBS: usize = 8;
const WALK_STACK_BYTES: usize = 16 * 1024 * 1024;

pub struct ScanResult {
    pub tree: ScanTree,
    pub summary: ScanSummary,
    pub phase: ScanPhase,
}

enum ScanState {
    Running,
    Finished(Arc<ScanResult>),
    Failed(SentinelError),
}

struct Scan {
    id: ScanId,
    root: PathBuf,
    cancel: AtomicBool,
    state: Mutex<ScanState>,
    done: Condvar,
}

enum JobState {
    Running,
    Finished(DuplicateReport),
    Failed(SentinelError),
}

struct DuplicateJob {
    cancel: AtomicBool,
    state: Mutex<JobState>,
    done: Condvar,
}

pub struct StorageService {
    provider: Arc<dyn StorageProvider>,
    sink: Arc<dyn EventSink>,
    scans: Mutex<Vec<Arc<Scan>>>,
    jobs: Mutex<Vec<(JobId, Arc<DuplicateJob>)>>,
}

fn still_running(feature: &str) -> SentinelError {
    SentinelError::Unavailable {
        feature: feature.to_owned(),
        reason: "still scanning".to_owned(),
    }
}

fn unknown(kind: &str, id: &str) -> SentinelError {
    SentinelError::invalid(format!("no {kind} with id {id}"))
}

impl StorageService {
    pub fn new(provider: Arc<dyn StorageProvider>, sink: Arc<dyn EventSink>) -> Arc<Self> {
        Arc::new(Self {
            provider,
            sink,
            scans: Mutex::new(Vec::new()),
            jobs: Mutex::new(Vec::new()),
        })
    }

    pub fn provider(&self) -> &Arc<dyn StorageProvider> {
        &self.provider
    }

    pub fn volumes(&self) -> CoreResult<Vec<VolumeInfo>> {
        self.provider.volumes()
    }

    fn scan(&self, scan_id: &str) -> CoreResult<Arc<Scan>> {
        self.scans
            .lock()
            .iter()
            .find(|s| s.id == scan_id)
            .cloned()
            .ok_or_else(|| unknown("scan", scan_id))
    }

    fn result(&self, scan_id: &str) -> CoreResult<Arc<ScanResult>> {
        let scan = self.scan(scan_id)?;
        let state = scan.state.lock();
        match &*state {
            ScanState::Running => Err(still_running("storage scan")),
            ScanState::Finished(result) => Ok(Arc::clone(result)),
            ScanState::Failed(err) => Err(err.clone()),
        }
    }

    pub fn start_scan(self: &Arc<Self>, request: ScanRequest) -> CoreResult<ScanId> {
        let requested = PathBuf::from(request.root.trim());
        if requested.as_os_str().is_empty() {
            return Err(SentinelError::invalid("choose a folder or volume to scan"));
        }
        let meta = std::fs::metadata(&requested)
            .map_err(|err| SentinelError::io(&err, Some(&requested)))?;
        if !meta.is_dir() {
            return Err(SentinelError::invalid(format!(
                "{} is not a folder",
                requested.display()
            )));
        }
        let root = std::fs::canonicalize(&requested)
            .map_err(|err| SentinelError::io(&err, Some(&requested)))?;
        let root = strip_verbatim(root);
        let root_meta = self.provider.metadata(&root)?;
        let scan = Arc::new(Scan {
            id: uuid::Uuid::new_v4().simple().to_string(),
            root: root.clone(),
            cancel: AtomicBool::new(false),
            state: Mutex::new(ScanState::Running),
            done: Condvar::new(),
        });
        {
            let mut scans = self.scans.lock();
            scans.push(Arc::clone(&scan));
            // Evict the oldest finished scans beyond the retention limit.
            while scans.len() > KEEP_SCANS + 1 {
                let Some(index) = scans
                    .iter()
                    .position(|s| !matches!(*s.state.lock(), ScanState::Running))
                else {
                    break;
                };
                scans.remove(index);
            }
        }
        let service = Arc::clone(self);
        let thread_scan = Arc::clone(&scan);
        std::thread::Builder::new()
            .name("sentinel-scan".into())
            .spawn(move || {
                service.run_scan(&thread_scan, root_meta.device, request.cross_mounts);
            })
            .map_err(|err| SentinelError::internal(format!("could not start scan: {err}")))?;
        Ok(scan.id.clone())
    }

    fn run_scan(&self, scan: &Scan, root_device: u64, cross_mounts: bool) {
        let started = Instant::now();
        let started_at_ms = now_ms();
        let progress = Progress::default();
        let root_name = scan
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| scan.root.to_string_lossy().into_owned());
        let live_root = LiveNode::new(root_name.clone(), scan.root.clone());
        let walking = AtomicBool::new(true);

        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(2, 8)
            * 2;
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .stack_size(WALK_STACK_BYTES)
            .thread_name(|i| format!("sentinel-walk-{i}"))
            .build();

        let built: Option<BuiltDir> = std::thread::scope(|scope| {
            scope.spawn(|| {
                let mut last_partial = Instant::now();
                while walking.load(Ordering::Relaxed) {
                    std::thread::sleep(PROGRESS_EVERY);
                    if !walking.load(Ordering::Relaxed) {
                        break;
                    }
                    self.sink.emit(CoreEvent::ScanProgress(progress_event(
                        scan,
                        &progress,
                        ScanPhase::Walking,
                        started,
                        None,
                    )));
                    if last_partial.elapsed() >= PARTIAL_EVERY {
                        last_partial = Instant::now();
                        let mut next_id = 0;
                        self.sink.emit(CoreEvent::ScanPartial(ScanPartial {
                            scan_id: scan.id.clone(),
                            root: live_root.snapshot(2, &mut next_id),
                        }));
                    }
                }
            });
            let result = pool.ok().map(|pool| {
                pool.install(|| {
                    let walker = Walker::new(
                        self.provider.as_ref(),
                        root_device,
                        cross_mounts,
                        &scan.cancel,
                        &progress,
                    );
                    walker.walk(
                        &scan.root,
                        root_name.clone(),
                        0,
                        false,
                        &[Arc::clone(&live_root)],
                    )
                })
            });
            walking.store(false, Ordering::Relaxed);
            result
        });

        let Some(built) = built else {
            let err = SentinelError::internal("could not create the scan thread pool");
            self.finish_failed(scan, &progress, started, err);
            return;
        };

        let cancelled = scan.cancel.load(Ordering::Relaxed);
        let vanished = !scan.root.exists();
        self.sink.emit(CoreEvent::ScanProgress(progress_event(
            scan,
            &progress,
            ScanPhase::Summarizing,
            started,
            None,
        )));
        let tree = ScanTree::from_built(scan.root.clone(), built);
        let root_node = tree.node(0);
        let mut unreadable_samples = progress.unreadable_samples.lock().clone();
        unreadable_samples.sort();
        let summary = ScanSummary {
            scan_id: scan.id.clone(),
            root: scan.root.to_string_lossy().into_owned(),
            started_at_ms,
            duration_ms: started.elapsed().as_millis() as u64,
            total_bytes: root_node.map(|n| n.size).unwrap_or(0),
            file_count: progress.files.load(Ordering::Relaxed),
            dir_count: progress.dirs.load(Ordering::Relaxed),
            unreadable_entries: progress.unreadable.load(Ordering::Relaxed),
            unreadable_samples,
            by_extension: summary::by_extension(std::mem::take(&mut *progress.extensions.lock())),
            largest_files: summary::largest_files(&tree, summary::LARGEST_FILES),
            suggestions: summary::suggestions(
                &tree,
                &self.provider.cleanup_locations(),
                now_secs(),
            ),
        };
        let (phase, error) = if vanished {
            (
                ScanPhase::Failed,
                Some(format!(
                    "{} disappeared during the scan (the volume may have been disconnected); the \
                     results are partial.",
                    scan.root.display()
                )),
            )
        } else if cancelled {
            (ScanPhase::Cancelled, None)
        } else {
            (ScanPhase::Complete, None)
        };
        let result = Arc::new(ScanResult {
            tree,
            summary: summary.clone(),
            phase,
        });
        *scan.state.lock() = ScanState::Finished(result);
        scan.done.notify_all();
        let mut final_progress = progress_event(scan, &progress, phase, started, error);
        final_progress.current_path = None;
        self.sink.emit(CoreEvent::ScanProgress(final_progress));
        if phase == ScanPhase::Complete {
            self.sink.emit(CoreEvent::ScanComplete(summary));
        }
    }

    fn finish_failed(
        &self,
        scan: &Scan,
        progress: &Progress,
        started: Instant,
        err: SentinelError,
    ) {
        let message = err.to_string();
        *scan.state.lock() = ScanState::Failed(err);
        scan.done.notify_all();
        self.sink.emit(CoreEvent::ScanProgress(progress_event(
            scan,
            progress,
            ScanPhase::Failed,
            started,
            Some(message),
        )));
    }

    pub fn cancel_scan(&self, scan_id: &str) -> CoreResult<()> {
        let scan = self.scan(scan_id)?;
        scan.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn wait_for_scan(&self, scan_id: &str, timeout: Duration) -> CoreResult<ScanSummary> {
        let scan = self.scan(scan_id)?;
        let deadline = Instant::now() + timeout;
        let mut state = scan.state.lock();
        loop {
            match &*state {
                ScanState::Finished(result) => {
                    return match result.phase {
                        ScanPhase::Cancelled => Err(SentinelError::Cancelled),
                        _ => Ok(result.summary.clone()),
                    };
                }
                ScanState::Failed(err) => return Err(err.clone()),
                ScanState::Running => {
                    if scan.done.wait_until(&mut state, deadline).timed_out() {
                        return Err(still_running("storage scan"));
                    }
                }
            }
        }
    }

    pub fn scan_covering(&self, path: &str) -> Option<ScanId> {
        let path = Path::new(path);
        self.scans
            .lock()
            .iter()
            .rev()
            .find(|scan| {
                path.starts_with(&scan.root)
                    && matches!(&*scan.state.lock(), ScanState::Finished(r) if r.phase == ScanPhase::Complete)
            })
            .map(|scan| scan.id.clone())
    }

    /// Latest finished tree covering `path`, for action previews.
    pub fn tree_covering(&self, path: &Path) -> Option<Arc<ScanResult>> {
        let id = self.scan_covering(&path.to_string_lossy())?;
        self.result(&id).ok()
    }

    /// Size of `path` by a sequential walk bounded by `budget`; `complete` is false when the
    /// budget ran out first.
    pub fn measure_path(&self, path: &Path, budget: Duration) -> crate::service::actions::PathSize {
        let deadline = Instant::now() + budget;
        let mut size = crate::service::actions::PathSize {
            bytes: 0,
            items: 0,
            modified: None,
            complete: true,
        };
        let mut stack = vec![path.to_path_buf()];
        while let Some(current) = stack.pop() {
            if Instant::now() >= deadline {
                size.complete = false;
                break;
            }
            let Ok(meta) = self.provider.metadata(&current) else {
                continue;
            };
            match meta.kind {
                EntryKind::Directory => {
                    if current != path && self.provider.should_skip(&current) {
                        continue;
                    }
                    if let Ok(entries) = std::fs::read_dir(&current) {
                        stack.extend(entries.flatten().map(|e| e.path()));
                    }
                }
                EntryKind::File => {
                    size.bytes += meta.allocated_bytes;
                    size.items += 1;
                    size.modified = tree::newer(size.modified, meta.modified);
                }
                EntryKind::Symlink | EntryKind::Other => {
                    size.bytes += meta.allocated_bytes;
                }
            }
        }
        size
    }

    pub fn scan_summary(&self, scan_id: &str) -> CoreResult<ScanSummary> {
        self.result(scan_id).map(|r| r.summary.clone())
    }

    pub fn scan_tree(&self, query: &TreeQuery) -> CoreResult<TreeNode> {
        let result = self.result(&query.scan_id)?;
        let id = query.node_id.unwrap_or(0);
        result
            .tree
            .slice(id, query.depth.min(8), query.max_children.clamp(1, 5000))
            .ok_or_else(|| unknown("node", &id.to_string()))
    }

    pub fn find_files(&self, scan_id: &str, filter: &FileFilter) -> CoreResult<Vec<FileEntry>> {
        let result = self.result(scan_id)?;
        let tree = &result.tree;
        let start = match &filter.under_path {
            Some(path) => tree.find(Path::new(path)).ok_or_else(|| {
                SentinelError::invalid(format!("{path} is not inside the scanned folder"))
            })?,
            None => 0,
        };
        let cutoff = filter
            .unused_for_days
            .map(|days| now_secs().saturating_sub(u64::from(days) * 86_400));
        let mut matches: Vec<(NodeId, u64)> = tree
            .subtree(start)
            .into_iter()
            .filter_map(|id| {
                let node = tree.node(id)?;
                if node.kind != NodeKind::File || node.size < filter.min_size_bytes {
                    return None;
                }
                if let Some(cutoff) = cutoff {
                    let last_used = node.modified.max(node.accessed)?;
                    if last_used >= cutoff {
                        return None;
                    }
                }
                Some((id, node.size))
            })
            .collect();
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        matches.truncate(filter.limit.clamp(1, 10_000) as usize);
        Ok(matches
            .into_iter()
            .filter_map(|(id, _)| tree.file_entry(id))
            .collect())
    }

    pub fn start_duplicate_scan(
        self: &Arc<Self>,
        scan_id: &str,
        min_size_bytes: u64,
    ) -> CoreResult<JobId> {
        let result = self.result(scan_id)?;
        let job_id = uuid::Uuid::new_v4().simple().to_string();
        let job = Arc::new(DuplicateJob {
            cancel: AtomicBool::new(false),
            state: Mutex::new(JobState::Running),
            done: Condvar::new(),
        });
        {
            let mut jobs = self.jobs.lock();
            jobs.push((job_id.clone(), Arc::clone(&job)));
            while jobs.len() > KEEP_JOBS {
                let Some(index) = jobs
                    .iter()
                    .position(|(_, j)| !matches!(*j.state.lock(), JobState::Running))
                else {
                    break;
                };
                jobs.remove(index);
            }
        }
        let service = Arc::clone(self);
        let thread_job_id = job_id.clone();
        let scan_id = scan_id.to_owned();
        std::thread::Builder::new()
            .name("sentinel-duplicates".into())
            .spawn(move || {
                service.run_duplicates(&thread_job_id, &scan_id, &job, &result, min_size_bytes)
            })
            .map_err(|err| {
                SentinelError::internal(format!("could not start duplicate scan: {err}"))
            })?;
        Ok(job_id)
    }

    fn run_duplicates(
        &self,
        job_id: &str,
        scan_id: &str,
        job: &DuplicateJob,
        result: &ScanResult,
        min_size_bytes: u64,
    ) {
        let started = Instant::now();
        let counters = DuplicateCounters::default();
        let running = AtomicBool::new(true);
        let outcome = std::thread::scope(|scope| {
            scope.spawn(|| {
                while running.load(Ordering::Relaxed) {
                    std::thread::sleep(PROGRESS_EVERY);
                    if !running.load(Ordering::Relaxed) {
                        break;
                    }
                    let phase = counters
                        .phase
                        .lock()
                        .unwrap_or(DuplicatePhase::GroupingBySize);
                    self.sink
                        .emit(CoreEvent::DuplicateProgress(DuplicateProgress {
                            job_id: job_id.to_owned(),
                            phase,
                            candidates: counters.candidates.load(Ordering::Relaxed),
                            hashed: counters.hashed.load(Ordering::Relaxed),
                            bytes_hashed: counters.bytes_hashed.load(Ordering::Relaxed),
                            error: None,
                        }));
                }
            });
            let outcome = duplicates::find(&result.tree, min_size_bytes, &job.cancel, &counters);
            running.store(false, Ordering::Relaxed);
            outcome
        });
        let (phase, error) = match outcome {
            Ok(groups) => {
                let report = DuplicateReport {
                    job_id: job_id.to_owned(),
                    scan_id: scan_id.to_owned(),
                    reclaimable_bytes: groups.iter().map(|g| g.reclaimable_bytes).sum(),
                    groups,
                    duration_ms: started.elapsed().as_millis() as u64,
                };
                *job.state.lock() = JobState::Finished(report.clone());
                job.done.notify_all();
                self.sink.emit(CoreEvent::DuplicateComplete(report));
                (DuplicatePhase::Complete, None)
            }
            Err(SentinelError::Cancelled) => {
                *job.state.lock() = JobState::Failed(SentinelError::Cancelled);
                job.done.notify_all();
                (DuplicatePhase::Cancelled, None)
            }
            Err(err) => {
                let message = err.to_string();
                *job.state.lock() = JobState::Failed(err);
                job.done.notify_all();
                (DuplicatePhase::Failed, Some(message))
            }
        };
        self.sink
            .emit(CoreEvent::DuplicateProgress(DuplicateProgress {
                job_id: job_id.to_owned(),
                phase,
                candidates: counters.candidates.load(Ordering::Relaxed),
                hashed: counters.hashed.load(Ordering::Relaxed),
                bytes_hashed: counters.bytes_hashed.load(Ordering::Relaxed),
                error,
            }));
    }

    fn job(&self, job_id: &str) -> CoreResult<Arc<DuplicateJob>> {
        self.jobs
            .lock()
            .iter()
            .find(|(id, _)| id == job_id)
            .map(|(_, job)| Arc::clone(job))
            .ok_or_else(|| unknown("duplicate scan", job_id))
    }

    pub fn cancel_duplicate_scan(&self, job_id: &str) -> CoreResult<()> {
        self.job(job_id)?.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn wait_for_duplicates(
        &self,
        job_id: &str,
        timeout: Duration,
    ) -> CoreResult<DuplicateReport> {
        let job = self.job(job_id)?;
        let deadline = Instant::now() + timeout;
        let mut state = job.state.lock();
        loop {
            match &*state {
                JobState::Finished(report) => return Ok(report.clone()),
                JobState::Failed(err) => return Err(err.clone()),
                JobState::Running => {
                    if job.done.wait_until(&mut state, deadline).timed_out() {
                        return Err(SentinelError::Unavailable {
                            feature: "duplicate detection".to_owned(),
                            reason: "still hashing files".to_owned(),
                        });
                    }
                }
            }
        }
    }
}

fn progress_event(
    scan: &Scan,
    progress: &Progress,
    phase: ScanPhase,
    started: Instant,
    error: Option<String>,
) -> ScanProgress {
    ScanProgress {
        scan_id: scan.id.clone(),
        phase,
        files_scanned: progress.files.load(Ordering::Relaxed),
        dirs_scanned: progress.dirs.load(Ordering::Relaxed),
        bytes_scanned: progress.bytes.load(Ordering::Relaxed),
        unreadable_entries: progress.unreadable.load(Ordering::Relaxed),
        current_path: progress.current.lock().clone(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        error,
    }
}

/// `canonicalize` on Windows yields `\\?\C:\…`; show and match plain paths.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\")
            && !rest.starts_with("UNC\\")
        {
            return PathBuf::from(rest);
        }
    }
    path
}

#[cfg(test)]
mod tests;

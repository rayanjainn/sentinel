//! Read-only facade over live system state.
//!
//! Implemented once by the core runtime (sampler caches + providers + scanner). Consumed by the
//! Tauri commands and by the agent's read tools, so both see identical data. Methods are blocking;
//! async callers wrap them in `spawn_blocking`.

pub mod actions;
#[cfg(feature = "native")]
pub mod audit_sqlite;
pub mod firewall;
pub mod history;
pub mod network;
pub mod sampling;
pub mod storage;
pub mod tokens;

use std::time::Duration;

use crate::error::CoreResult;
use crate::model::*;

/// Filter for `find_files`, backing the agent's `find_large_files` tool and the storage views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    /// Restrict to files beneath this path (must be inside the scan root).
    pub under_path: Option<String>,
    pub min_size_bytes: u64,
    /// Exclude files whose max(modified, accessed) is newer than this many days.
    pub unused_for_days: Option<u32>,
    pub limit: u32,
}

pub trait SystemQueries: Send + Sync {
    fn system_info(&self) -> SystemInfo;
    fn permission_status(&self) -> PermissionStatus;

    /// Latest sample if fresher than one interval, otherwise samples on demand.
    fn resource_sample(&self) -> CoreResult<ResourceSample>;
    fn resource_history(&self, window_secs: u32) -> Vec<ResourceSample>;

    /// Refreshes on demand when the processes stream is not subscribed.
    fn process_snapshot(&self) -> CoreResult<ProcessSnapshot>;
    fn process_detail(&self, pid: Pid) -> CoreResult<ProcessDetail>;
    fn process_history(&self, pid: Pid) -> Vec<ProcessHistoryPoint>;

    /// Refreshes on demand when the network stream is not subscribed.
    fn network_snapshot(&self) -> CoreResult<NetworkSnapshot>;
    fn firewall_status(&self) -> FirewallStatus;
    fn firewall_rules(&self) -> CoreResult<Vec<FirewallRule>>;

    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>>;
    fn start_scan(&self, request: ScanRequest) -> CoreResult<ScanId>;
    fn cancel_scan(&self, scan_id: &str) -> CoreResult<()>;
    /// Blocks until the scan completes, fails, or `timeout` elapses (`Cancelled` on timeout is not
    /// used; returns `Unavailable` with reason "still scanning").
    fn wait_for_scan(&self, scan_id: &str, timeout: Duration) -> CoreResult<ScanSummary>;
    /// Most recent completed scan whose root contains `path`.
    fn scan_covering(&self, path: &str) -> Option<ScanId>;
    fn scan_summary(&self, scan_id: &str) -> CoreResult<ScanSummary>;
    fn scan_tree(&self, query: &TreeQuery) -> CoreResult<TreeNode>;
    fn find_files(&self, scan_id: &str, filter: &FileFilter) -> CoreResult<Vec<FileEntry>>;

    fn start_duplicate_scan(&self, scan_id: &str, min_size_bytes: u64) -> CoreResult<JobId>;
    fn cancel_duplicate_scan(&self, job_id: &str) -> CoreResult<()>;
    fn wait_for_duplicates(&self, job_id: &str, timeout: Duration) -> CoreResult<DuplicateReport>;
}

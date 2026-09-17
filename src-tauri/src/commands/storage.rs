use sentinel_core::model::{
    DuplicateReport, JobId, ScanId, ScanRequest, ScanSummary, TreeNode, TreeQuery, VolumeInfo,
};

use super::{CmdResult, not_wired};

#[tauri::command]
pub async fn list_volumes() -> CmdResult<Vec<VolumeInfo>> {
    not_wired("list_volumes")
}

#[tauri::command]
pub async fn start_scan(request: ScanRequest) -> CmdResult<ScanId> {
    let _ = request;
    not_wired("start_scan")
}

#[tauri::command]
pub async fn cancel_scan(scan_id: ScanId) -> CmdResult<()> {
    let _ = scan_id;
    not_wired("cancel_scan")
}

#[tauri::command]
pub async fn get_scan_tree(query: TreeQuery) -> CmdResult<TreeNode> {
    let _ = query;
    not_wired("get_scan_tree")
}

#[tauri::command]
pub async fn get_scan_summary(scan_id: ScanId) -> CmdResult<ScanSummary> {
    let _ = scan_id;
    not_wired("get_scan_summary")
}

#[tauri::command]
pub async fn start_duplicate_scan(scan_id: ScanId, min_size_bytes: u64) -> CmdResult<JobId> {
    let _ = (scan_id, min_size_bytes);
    not_wired("start_duplicate_scan")
}

#[tauri::command]
pub async fn cancel_duplicate_scan(job_id: JobId) -> CmdResult<()> {
    let _ = job_id;
    not_wired("cancel_duplicate_scan")
}

#[tauri::command]
pub async fn get_duplicate_report(job_id: JobId) -> CmdResult<DuplicateReport> {
    let _ = job_id;
    not_wired("get_duplicate_report")
}

#[tauri::command]
pub async fn reveal_path(path: String) -> CmdResult<()> {
    let _ = path;
    not_wired("reveal_path")
}

use sentinel_core::model::{
    DuplicateReport, JobId, ScanId, ScanRequest, ScanSummary, TreeNode, TreeQuery, VolumeInfo,
};
use tauri::State;

use super::CmdResult;
use crate::state::{CoreState, blocking};

#[tauri::command]
pub async fn list_volumes(state: State<'_, CoreState>) -> CmdResult<Vec<VolumeInfo>> {
    let queries = state.queries.clone();
    blocking(move || queries.volumes()).await
}

/// Returns immediately; progress arrives as `sentinel:scan-progress` / `sentinel:scan-partial`.
#[tauri::command]
pub async fn start_scan(state: State<'_, CoreState>, request: ScanRequest) -> CmdResult<ScanId> {
    let queries = state.queries.clone();
    blocking(move || queries.start_scan(request)).await
}

#[tauri::command]
pub async fn cancel_scan(state: State<'_, CoreState>, scan_id: ScanId) -> CmdResult<()> {
    let queries = state.queries.clone();
    blocking(move || queries.cancel_scan(&scan_id)).await
}

#[tauri::command]
pub async fn get_scan_tree(state: State<'_, CoreState>, query: TreeQuery) -> CmdResult<TreeNode> {
    let queries = state.queries.clone();
    blocking(move || queries.scan_tree(&query)).await
}

#[tauri::command]
pub async fn get_scan_summary(
    state: State<'_, CoreState>,
    scan_id: ScanId,
) -> CmdResult<ScanSummary> {
    let queries = state.queries.clone();
    blocking(move || queries.scan_summary(&scan_id)).await
}

#[tauri::command]
pub async fn start_duplicate_scan(
    state: State<'_, CoreState>,
    scan_id: ScanId,
    min_size_bytes: u64,
) -> CmdResult<JobId> {
    let queries = state.queries.clone();
    blocking(move || queries.start_duplicate_scan(&scan_id, min_size_bytes)).await
}

#[tauri::command]
pub async fn cancel_duplicate_scan(state: State<'_, CoreState>, job_id: JobId) -> CmdResult<()> {
    let queries = state.queries.clone();
    blocking(move || queries.cancel_duplicate_scan(&job_id)).await
}

#[tauri::command]
pub async fn get_duplicate_report(
    state: State<'_, CoreState>,
    job_id: JobId,
) -> CmdResult<DuplicateReport> {
    let runtime = state.runtime.clone();
    blocking(move || runtime.duplicate_report(&job_id)).await
}

#[tauri::command]
pub async fn reveal_path(state: State<'_, CoreState>, path: String) -> CmdResult<()> {
    let runtime = state.runtime.clone();
    blocking(move || runtime.reveal_path(&path)).await
}

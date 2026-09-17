use sentinel_core::model::{Pid, ProcessDetail, ProcessHistoryPoint, ProcessSnapshot};
use tauri::State;

use super::CmdResult;
use crate::state::{CoreState, blocking};

#[tauri::command]
pub async fn get_process_snapshot(state: State<'_, CoreState>) -> CmdResult<ProcessSnapshot> {
    let queries = state.queries.clone();
    blocking(move || queries.process_snapshot()).await
}

#[tauri::command]
pub async fn get_process_detail(state: State<'_, CoreState>, pid: Pid) -> CmdResult<ProcessDetail> {
    let queries = state.queries.clone();
    blocking(move || queries.process_detail(pid)).await
}

#[tauri::command]
pub async fn get_process_history(
    state: State<'_, CoreState>,
    pid: Pid,
) -> CmdResult<Vec<ProcessHistoryPoint>> {
    let queries = state.queries.clone();
    blocking(move || Ok(queries.process_history(pid))).await
}

#[tauri::command]
pub async fn reveal_process_executable(state: State<'_, CoreState>, pid: Pid) -> CmdResult<()> {
    let runtime = state.runtime.clone();
    blocking(move || runtime.reveal_process_executable(pid)).await
}

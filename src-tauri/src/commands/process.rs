use sentinel_core::model::{Pid, ProcessDetail, ProcessHistoryPoint, ProcessSnapshot};

use super::{CmdResult, not_wired};

#[tauri::command]
pub async fn get_process_snapshot() -> CmdResult<ProcessSnapshot> {
    not_wired("get_process_snapshot")
}

#[tauri::command]
pub async fn get_process_detail(pid: Pid) -> CmdResult<ProcessDetail> {
    let _ = pid;
    not_wired("get_process_detail")
}

#[tauri::command]
pub async fn get_process_history(pid: Pid) -> CmdResult<Vec<ProcessHistoryPoint>> {
    let _ = pid;
    not_wired("get_process_history")
}

#[tauri::command]
pub async fn reveal_process_executable(pid: Pid) -> CmdResult<()> {
    let _ = pid;
    not_wired("reveal_process_executable")
}

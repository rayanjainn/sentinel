use sentinel_core::action::{Action, ActionOutcome, ActionPreview};
use sentinel_core::audit::{AuditEntry, AuditQuery};

use super::{CmdResult, not_wired};

#[tauri::command]
pub async fn prepare_action(action: Action) -> CmdResult<ActionPreview> {
    let _ = action;
    not_wired("prepare_action")
}

#[tauri::command]
pub async fn commit_action(token: String) -> CmdResult<ActionOutcome> {
    let _ = token;
    not_wired("commit_action")
}

#[tauri::command]
pub async fn reject_action(token: String) -> CmdResult<()> {
    let _ = token;
    not_wired("reject_action")
}

#[tauri::command]
pub async fn get_audit_log(query: AuditQuery) -> CmdResult<Vec<AuditEntry>> {
    let _ = query;
    not_wired("get_audit_log")
}

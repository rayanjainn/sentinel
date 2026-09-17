use sentinel_core::action::{Action, ActionOutcome, ActionPreview, Origin};
use sentinel_core::audit::{AuditEntry, AuditQuery};
use tauri::State;

use super::CmdResult;
use crate::state::{CoreState, blocking};

#[tauri::command]
pub async fn prepare_action(
    state: State<'_, CoreState>,
    action: Action,
) -> CmdResult<ActionPreview> {
    let preparer = state.preparer.clone();
    blocking(move || preparer.prepare(action, Origin::User)).await
}

#[tauri::command]
pub async fn commit_action(state: State<'_, CoreState>, token: String) -> CmdResult<ActionOutcome> {
    let committer = state.committer.clone();
    blocking(move || committer.commit(&token)).await
}

#[tauri::command]
pub async fn reject_action(state: State<'_, CoreState>, token: String) -> CmdResult<()> {
    let committer = state.committer.clone();
    blocking(move || committer.reject(&token)).await
}

#[tauri::command]
pub async fn get_audit_log(
    state: State<'_, CoreState>,
    query: AuditQuery,
) -> CmdResult<Vec<AuditEntry>> {
    let audit = state.audit.clone();
    blocking(move || audit.query(&query)).await
}

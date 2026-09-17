use sentinel_core::events::SamplingConfig;
use sentinel_core::model::{PermissionKind, PermissionStatus, ResourceSample, SystemInfo};
use tauri::State;

use super::CmdResult;
use crate::state::{CoreState, blocking};

#[tauri::command]
pub async fn get_system_info(state: State<'_, CoreState>) -> CmdResult<SystemInfo> {
    Ok(state.queries.system_info())
}

#[tauri::command]
pub async fn get_resource_history(
    state: State<'_, CoreState>,
    window_secs: u32,
) -> CmdResult<Vec<ResourceSample>> {
    let queries = state.queries.clone();
    blocking(move || Ok(queries.resource_history(window_secs.min(900)))).await
}

#[tauri::command]
pub async fn set_sampling(
    state: State<'_, CoreState>,
    config: SamplingConfig,
) -> CmdResult<SamplingConfig> {
    Ok(state.runtime.set_sampling(config))
}

#[tauri::command]
pub async fn get_permission_status(state: State<'_, CoreState>) -> CmdResult<PermissionStatus> {
    let queries = state.queries.clone();
    blocking(move || Ok(queries.permission_status())).await
}

#[tauri::command]
pub async fn open_permission_settings(
    state: State<'_, CoreState>,
    kind: PermissionKind,
) -> CmdResult<()> {
    let runtime = state.runtime.clone();
    blocking(move || runtime.open_permission_settings(kind)).await
}

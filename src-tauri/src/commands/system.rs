use sentinel_core::events::SamplingConfig;
use sentinel_core::model::{PermissionKind, PermissionStatus, ResourceSample, SystemInfo};

use super::{CmdResult, not_wired};

#[tauri::command]
pub async fn get_system_info() -> CmdResult<SystemInfo> {
    not_wired("get_system_info")
}

#[tauri::command]
pub async fn get_resource_history(window_secs: u32) -> CmdResult<Vec<ResourceSample>> {
    let _ = window_secs;
    not_wired("get_resource_history")
}

#[tauri::command]
pub async fn set_sampling(config: SamplingConfig) -> CmdResult<SamplingConfig> {
    let _ = config;
    not_wired("set_sampling")
}

#[tauri::command]
pub async fn get_permission_status() -> CmdResult<PermissionStatus> {
    not_wired("get_permission_status")
}

#[tauri::command]
pub async fn open_permission_settings(kind: PermissionKind) -> CmdResult<()> {
    let _ = kind;
    not_wired("open_permission_settings")
}

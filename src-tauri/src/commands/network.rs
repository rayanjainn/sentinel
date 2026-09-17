use sentinel_core::model::{GeoDbStatus, HomeLocation, HomeLocationInput, NetworkSnapshot};

use super::{CmdResult, not_wired};

#[tauri::command]
pub async fn get_network_snapshot() -> CmdResult<NetworkSnapshot> {
    not_wired("get_network_snapshot")
}

#[tauri::command]
pub async fn get_geo_db_status() -> CmdResult<GeoDbStatus> {
    not_wired("get_geo_db_status")
}

#[tauri::command]
pub async fn download_geo_db() -> CmdResult<()> {
    not_wired("download_geo_db")
}

#[tauri::command]
pub async fn get_home_location() -> CmdResult<HomeLocation> {
    not_wired("get_home_location")
}

#[tauri::command]
pub async fn set_home_location(location: Option<HomeLocationInput>) -> CmdResult<HomeLocation> {
    let _ = location;
    not_wired("set_home_location")
}

use sentinel_core::model::{GeoDbStatus, HomeLocation, HomeLocationInput, NetworkSnapshot};
use tauri::State;

use super::CmdResult;
use crate::state::{CoreState, blocking};

#[tauri::command]
pub async fn get_network_snapshot(state: State<'_, CoreState>) -> CmdResult<NetworkSnapshot> {
    let queries = state.queries.clone();
    blocking(move || queries.network_snapshot()).await
}

#[tauri::command]
pub async fn get_geo_db_status(state: State<'_, CoreState>) -> CmdResult<GeoDbStatus> {
    Ok(state.runtime.geo_db_status())
}

/// Returns once the download has started; progress and completion arrive as `sentinel:geo-db`.
#[tauri::command]
pub async fn download_geo_db(state: State<'_, CoreState>) -> CmdResult<()> {
    let runtime = state.runtime.clone();
    blocking(move || runtime.download_geo_db()).await
}

#[tauri::command]
pub async fn get_home_location(state: State<'_, CoreState>) -> CmdResult<HomeLocation> {
    let runtime = state.runtime.clone();
    blocking(move || runtime.home_location()).await
}

#[tauri::command]
pub async fn set_home_location(
    state: State<'_, CoreState>,
    location: Option<HomeLocationInput>,
) -> CmdResult<HomeLocation> {
    let runtime = state.runtime.clone();
    blocking(move || runtime.set_home_location(location)).await
}

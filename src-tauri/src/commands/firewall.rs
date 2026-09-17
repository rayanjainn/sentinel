use sentinel_core::model::{FirewallRule, FirewallStatus};
use tauri::State;

use super::CmdResult;
use crate::state::{CoreState, blocking};

#[tauri::command]
pub async fn get_firewall_status(state: State<'_, CoreState>) -> CmdResult<FirewallStatus> {
    let queries = state.queries.clone();
    blocking(move || Ok(queries.firewall_status())).await
}

/// Sentinel-created rules only, with `active` reconciled against the OS firewall.
#[tauri::command]
pub async fn list_firewall_rules(state: State<'_, CoreState>) -> CmdResult<Vec<FirewallRule>> {
    let queries = state.queries.clone();
    blocking(move || queries.firewall_rules()).await
}

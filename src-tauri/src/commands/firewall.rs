use sentinel_core::model::{FirewallRule, FirewallStatus};

use super::{CmdResult, not_wired};

#[tauri::command]
pub async fn get_firewall_status() -> CmdResult<FirewallStatus> {
    not_wired("get_firewall_status")
}

#[tauri::command]
pub async fn list_firewall_rules() -> CmdResult<Vec<FirewallRule>> {
    not_wired("list_firewall_rules")
}

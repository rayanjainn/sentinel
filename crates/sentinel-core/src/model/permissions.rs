use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{FirewallStatus, Platform};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PermissionState {
    Granted,
    Denied,
    Unknown,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PermissionKind {
    /// macOS TCC Full Disk Access — needed for accurate storage scans of protected folders.
    FullDiskAccess,
    /// Administrator/root/UAC — needed for firewall rules, other users' processes, raising priority.
    Administrator,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PermissionStatus {
    pub platform: Platform,
    pub full_disk_access: PermissionState,
    /// Sentinel itself is running elevated.
    pub running_elevated: bool,
    /// An elevation mechanism exists (osascript admin prompt / pkexec / UAC).
    pub can_request_elevation: bool,
    pub firewall: FirewallStatus,
}

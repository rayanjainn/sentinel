use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{TimestampMs, TransportProtocol};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum FirewallTarget {
    RemoteIp {
        ip: String,
    },
    LocalPort {
        port: u16,
        protocol: TransportProtocol,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TrafficDirection {
    Inbound,
    Outbound,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FirewallBackend {
    /// macOS pf, rules loaded into the `com.apple/250.sentinel` anchor (covered by the default pf.conf).
    Pf,
    /// Linux nftables, dedicated `inet sentinel` table.
    Nftables,
    /// Linux fallback, dedicated `SENTINEL` chains.
    Iptables,
    /// `netsh advfirewall`, rule names prefixed `Sentinel-`.
    WindowsFirewall,
}

/// A rule Sentinel created. Sentinel only lists and removes its own rules, never the user's others.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FirewallRule {
    pub id: String,
    pub target: FirewallTarget,
    pub direction: TrafficDirection,
    pub backend: FirewallBackend,
    pub created_at_ms: TimestampMs,
    /// Audit entry that created it.
    pub audit_id: Option<i64>,
    /// Whether the rule is currently loaded in the OS firewall (pf anchors are cleared on reboot).
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FirewallStatus {
    pub backend: Option<FirewallBackend>,
    pub available: bool,
    /// e.g. pf disabled, Windows Firewall off for the active profile.
    pub firewall_enabled: Option<bool>,
    pub requires_elevation: bool,
    pub note: Option<String>,
}

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{TimestampMs, TimestampSecs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Platform {
    Macos,
    Windows,
    Linux,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SystemInfo {
    pub platform: Platform,
    pub hostname: Option<String>,
    pub os_name: String,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub arch: String,
    pub cpu_brand: String,
    pub physical_cores: Option<u32>,
    pub logical_cores: u32,
    pub total_memory: u64,
    pub total_swap: u64,
    pub boot_time: TimestampSecs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemoryBreakdown {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub free: u64,
    /// File cache / standby list. `None` where the OS does not separate it.
    pub cached: Option<u64>,
    /// macOS compressor, Linux zswap.
    pub compressed: Option<u64>,
    /// macOS wired, Windows non-paged pool.
    pub wired: Option<u64>,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LoadKind {
    /// Classic Unix run-queue load average.
    UnixRunQueue,
    /// Windows: exponentially damped `\System\Processor Queue Length` + running threads,
    /// computed with the same 1/5/15 minute constants.
    WindowsProcessorQueue,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LoadAverage {
    pub one: f64,
    pub five: f64,
    pub fifteen: f64,
    pub kind: LoadKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TemperatureReading {
    pub label: String,
    pub celsius: f32,
    pub critical_celsius: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FanReading {
    pub label: String,
    pub rpm: u32,
    pub max_rpm: Option<u32>,
}

/// Only sensors the OS exposes through public APIs. The whole field is `None` when nothing is
/// readable, and the UI hides the panel rather than showing zeros.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ThermalSample {
    pub temperatures: Vec<TemperatureReading>,
    pub fans: Vec<FanReading>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetThroughput {
    /// Bytes per second (not bits) received, summed across every network interface on this
    /// machine — Wi-Fi, Ethernet, VPN — with loopback excluded.
    pub rx_bps: u64,
    /// Bytes per second (not bits) sent, same scope as `rx_bps`.
    pub tx_bps: u64,
    /// Cumulative bytes received since the interface came up (normally since the machine booted),
    /// same scope as `rx_bps`.
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ResourceSample {
    pub ts_ms: TimestampMs,
    /// 0..=100 across all cores.
    pub cpu_total: f32,
    /// 0..=100 per logical core, index = core number.
    pub per_core: Vec<f32>,
    pub memory: MemoryBreakdown,
    pub load: Option<LoadAverage>,
    pub thermal: Option<ThermalSample>,
    pub network: NetThroughput,
}

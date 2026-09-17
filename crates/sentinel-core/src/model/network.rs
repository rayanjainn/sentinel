use std::net::IpAddr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{NetThroughput, Pid, TimestampMs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TransportProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IpFamily {
    V4,
    V6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TcpState {
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
    Closed,
    DeleteTcb,
    Unknown,
}

/// Address class of the remote end; only `Public` addresses are geolocated and drawn on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AddrScope {
    Loopback,
    Private,
    LinkLocal,
    Multicast,
    Unspecified,
    Public,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GeoLocation {
    pub lat: f64,
    pub lon: f64,
    pub city: Option<String>,
    pub region: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
}

/// Socket row exactly as the OS reports it, before enrichment. Produced by `NetworkProvider`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RawSocket {
    pub protocol: TransportProtocol,
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: Option<IpAddr>,
    pub remote_port: Option<u16>,
    pub state: Option<TcpState>,
    /// A socket can be shared by several processes (fork/inherit); one row per owner.
    pub pid: Option<Pid>,
}

/// Enriched socket row sent to the UI (process name, reverse DNS, geo, traffic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SocketEntry {
    /// Stable across samples: `proto|local:port|remote:port|pid`.
    pub id: String,
    pub protocol: TransportProtocol,
    pub family: IpFamily,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: Option<String>,
    pub remote_port: Option<u16>,
    pub remote_scope: Option<AddrScope>,
    /// `None` for UDP.
    pub state: Option<TcpState>,
    pub pid: Option<Pid>,
    pub process_name: Option<String>,
    /// Reverse-DNS result; resolved asynchronously, so often `None` on first sight.
    pub remote_host: Option<String>,
    pub geo: Option<GeoLocation>,
    /// Cumulative bytes. Kernel counters where exposed, otherwise since Sentinel first saw the socket.
    pub bytes_in: Option<u64>,
    pub bytes_out: Option<u64>,
    pub rx_bps: Option<u64>,
    pub tx_bps: Option<u64>,
    pub first_seen_ms: TimestampMs,
}

/// Granularity of traffic accounting the current OS/privilege level supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TrafficSource {
    PerConnection,
    PerProcess,
    InterfaceOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionTraffic {
    pub protocol: TransportProtocol,
    pub local_addr: IpAddr,
    pub local_port: u16,
    pub remote_addr: Option<IpAddr>,
    pub remote_port: Option<u16>,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessTraffic {
    pub pid: Pid,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrafficReport {
    pub source: TrafficSource,
    pub connections: Vec<ConnectionTraffic>,
    pub processes: Vec<ProcessTraffic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NetworkSnapshot {
    pub ts_ms: TimestampMs,
    pub sockets: Vec<SocketEntry>,
    pub throughput: NetThroughput,
    pub traffic_source: TrafficSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HostResolved {
    pub ip: String,
    pub hostname: Option<String>,
}

/// Local IP-geolocation database (DB-IP City Lite, mmdb format). Downloaded once; lookups never
/// leave the machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum GeoDbStatus {
    Missing,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    Ready {
        build_date: Option<String>,
        attribution: String,
    },
    Failed {
        message: String,
    },
}

/// Approximate position of this machine for the map origin, derived from the system time zone
/// (or a user override) — no external lookup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HomeLocation {
    pub lat: f64,
    pub lon: f64,
    pub label: String,
    pub source: HomeLocationSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HomeLocationSource {
    TimeZone,
    UserSetting,
}

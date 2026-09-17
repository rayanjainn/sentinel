use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{Pid, ProcessExplanation, ProcessSummary, SocketEntry, TimestampMs, TimestampSecs};
use crate::error::ErrorPayload;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ProcessStatus {
    Running,
    Sleeping,
    Idle,
    Waiting,
    Stopped,
    Zombie,
    Dead,
    Unknown,
}

/// A PID alone is not a safe action target: PIDs are recycled. Every process action carries the
/// start time observed at preview so execution can refuse if the PID now belongs to another process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessIdentity {
    pub pid: Pid,
    pub start_time: TimestampSecs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessInfo {
    pub pid: Pid,
    pub ppid: Option<Pid>,
    pub name: String,
    pub cmd: Vec<String>,
    pub exe: Option<String>,
    pub user: Option<String>,
    pub status: ProcessStatus,
    /// Top-style percentage: 100 = one full logical core, so it can exceed 100 on multi-core machines.
    pub cpu_percent: f32,
    /// Exponential moving average of `cpu_percent` (~10 sample time constant).
    pub cpu_percent_avg: f32,
    pub memory_rss: u64,
    pub memory_virtual: u64,
    pub start_time: TimestampSecs,
    pub run_time_secs: u64,
    /// `None` when the OS does not expose it for this process (e.g. insufficient privileges).
    pub thread_count: Option<u32>,
    pub fd_count: Option<u32>,
    /// Unix nice value (-20..=19). On Windows the priority class is mapped onto this scale.
    pub nice: Option<i32>,
    /// Plain-language explanation of what this process is, for the table subtitle and app
    /// grouping. `Default` (empty headline, `Confidence::Unknown`) for a moment before the
    /// runtime's enrichment pass fills it in.
    pub summary: ProcessSummary,
}

impl ProcessInfo {
    pub fn identity(&self) -> ProcessIdentity {
        ProcessIdentity {
            pid: self.pid,
            start_time: self.start_time,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessSnapshot {
    pub ts_ms: TimestampMs,
    pub processes: Vec<ProcessInfo>,
    pub logical_cores: u32,
    pub total_memory: u64,
}

/// Per-process ring buffer point backing the detail panel's 60s mini graph.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessHistoryPoint {
    pub ts_ms: TimestampMs,
    pub cpu_percent: f32,
    pub memory_rss: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OpenFileKind {
    File,
    Directory,
    Socket,
    Pipe,
    Device,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OpenFile {
    pub fd: Option<i64>,
    pub kind: OpenFileKind,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProcessDetail {
    pub info: ProcessInfo,
    pub open_files: Vec<OpenFile>,
    /// Set when handles could not be listed (typically another user's process without elevation).
    pub open_files_error: Option<ErrorPayload>,
    pub connections: Vec<SocketEntry>,
    /// Whether graceful termination will close a window rather than send a signal.
    pub has_window: bool,
    /// Full plain-language explanation, including the "Is it safe to quit?" note and the
    /// evidence it was built from. `info.summary` carries the same headline/role/safety, computed
    /// once here.
    pub explanation: ProcessExplanation,
}

/// How a graceful terminate was delivered, reported back so the UI can say exactly what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TerminateMethod {
    /// SIGTERM (macOS/Linux).
    Signal,
    /// App quit request / WM_CLOSE to the process's windows.
    WindowClose,
    /// Windows process without windows: no SIGTERM equivalent exists, so TerminateProcess is used.
    TerminateProcess,
}

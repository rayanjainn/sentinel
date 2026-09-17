//! Push-based live data. The sampler and background jobs emit [`CoreEvent`]s into an
//! [`EventSink`]; the Tauri layer forwards each to the frontend under the matching name in [`names`].

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum StreamKind {
    Resources,
    Processes,
    Network,
}

/// Only subscribed streams are sampled, so a hidden Network view costs nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SamplingConfig {
    /// 500 ("high refresh"), 1000 (default), or 2000.
    pub interval_ms: u32,
    pub streams: Vec<StreamKind>,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            interval_ms: 1000,
            streams: vec![StreamKind::Resources],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CoreEvent {
    Resources(ResourceSample),
    Processes(ProcessSnapshot),
    Network(NetworkSnapshot),
    HostResolved(HostResolved),
    ScanProgress(ScanProgress),
    ScanPartial(ScanPartial),
    ScanComplete(ScanSummary),
    DuplicateProgress(DuplicateProgress),
    DuplicateComplete(DuplicateReport),
    GeoDb(GeoDbStatus),
}

impl CoreEvent {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Resources(_) => names::RESOURCES,
            Self::Processes(_) => names::PROCESSES,
            Self::Network(_) => names::NETWORK,
            Self::HostResolved(_) => names::HOST_RESOLVED,
            Self::ScanProgress(_) => names::SCAN_PROGRESS,
            Self::ScanPartial(_) => names::SCAN_PARTIAL,
            Self::ScanComplete(_) => names::SCAN_COMPLETE,
            Self::DuplicateProgress(_) => names::DUPLICATE_PROGRESS,
            Self::DuplicateComplete(_) => names::DUPLICATE_COMPLETE,
            Self::GeoDb(_) => names::GEO_DB,
        }
    }
}

pub trait EventSink: Send + Sync + 'static {
    fn emit(&self, event: CoreEvent);
}

pub mod names {
    pub const RESOURCES: &str = "sentinel:resources";
    pub const PROCESSES: &str = "sentinel:processes";
    pub const NETWORK: &str = "sentinel:network";
    pub const HOST_RESOLVED: &str = "sentinel:host-resolved";
    pub const SCAN_PROGRESS: &str = "sentinel:scan-progress";
    pub const SCAN_PARTIAL: &str = "sentinel:scan-partial";
    pub const SCAN_COMPLETE: &str = "sentinel:scan-complete";
    pub const DUPLICATE_PROGRESS: &str = "sentinel:duplicate-progress";
    pub const DUPLICATE_COMPLETE: &str = "sentinel:duplicate-complete";
    pub const GEO_DB: &str = "sentinel:geo-db";
    /// Payload: `sentinel_agent::AgentStreamPayload`.
    pub const AGENT: &str = "sentinel:agent";
}

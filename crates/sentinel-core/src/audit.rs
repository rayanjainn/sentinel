use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::action::{Action, Metric, Origin};
use crate::error::CoreResult;
use crate::model::TimestampMs;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AuditStatus {
    Succeeded,
    PartiallySucceeded,
    Failed,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditEntry {
    pub id: i64,
    pub ts_ms: TimestampMs,
    pub origin: Origin,
    /// The user's natural-language request for agent actions; `None` for direct UI actions.
    pub trigger: Option<String>,
    pub action: Action,
    pub title: String,
    pub status: AuditStatus,
    pub summary: String,
    pub bytes_freed: Option<u64>,
    /// Original locations of anything moved to trash or elsewhere, for recovery.
    pub affected_paths: Vec<String>,
    pub before: Vec<Metric>,
    pub after: Vec<Metric>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OriginFilter {
    Any,
    User,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuditQuery {
    pub limit: u32,
    /// Keyset pagination: entries with id < `before_id`.
    pub before_id: Option<i64>,
    pub origin: OriginFilter,
}

pub trait AuditStore: Send + Sync {
    fn append(&self, entry: AuditEntry) -> CoreResult<i64>;
    fn query(&self, query: &AuditQuery) -> CoreResult<Vec<AuditEntry>>;
}

//! The single action pipeline shared by UI buttons and the agent.
//!
//! Every state-changing operation is an [`Action`]. Execution is two-phase:
//! 1. [`ActionPreparer::prepare`] validates against live state and returns an [`ActionPreview`]
//!    carrying a single-use token. Nothing changes on the system.
//! 2. [`ActionCommitter::commit`] executes a previously previewed token.
//!
//! The agent layer is only ever handed an `ActionPreparer`. The only caller of `commit` is the
//! Tauri `commit_action` / `agent_execute_plan` commands, reached from a human click.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{CoreResult, ErrorPayload};
use crate::model::{FirewallTarget, ProcessIdentity, TimestampMs, TrafficDirection};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Action {
    /// SIGTERM, or a window close / app quit request when the process owns a window.
    TerminateProcess {
        target: ProcessIdentity,
    },
    /// SIGKILL / TerminateProcess.
    ForceKillProcess {
        target: ProcessIdentity,
    },
    /// Unix nice scale -20..=19. Windows maps: <=-11 High, -10..=-1 AboveNormal, 0 Normal,
    /// 1..=10 BelowNormal, >=11 Idle. Realtime is never used.
    SetProcessPriority {
        target: ProcessIdentity,
        nice: i32,
    },
    /// Always the OS trash / recycle bin. There is no permanent-delete action.
    TrashPaths {
        paths: Vec<String>,
    },
    MovePaths {
        paths: Vec<String>,
        destination_dir: String,
    },
    AddFirewallRule {
        target: FirewallTarget,
        direction: TrafficDirection,
    },
    RemoveFirewallRule {
        rule_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Origin {
    User,
    Agent {
        conversation_id: String,
        plan_id: String,
        provider: String,
        model: String,
        /// The user message that produced the plan; recorded as the audit trigger.
        request: String,
    },
}

/// Drives confirm-dialog severity. Firewall changes are `Critical` (modifies OS network policy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ActionRisk {
    Moderate,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Reversibility {
    /// e.g. "Restore from Trash".
    Recoverable {
        how: String,
    },
    /// e.g. "Remove the rule from Firewall rules".
    Undoable {
        how: String,
    },
    Irreversible,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PreviewTarget {
    /// "Google Chrome Helper (PID 4821)", "~/Library/Caches/com.old.app".
    pub label: String,
    pub detail: Option<String>,
    pub size_bytes: Option<u64>,
    /// Target could not be validated (vanished, unreadable); it will be skipped on commit.
    pub problem: Option<String>,
    /// For a process target, the same "Is it safe to quit?" sentence shown in the detail drawer —
    /// computed once so the confirm dialog and the agent's plan card never disagree with it.
    /// `None` for a target that is not a process (a path, a firewall rule).
    pub safety_note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MetricUnit {
    Bytes,
    Percent,
    Count,
    Nice,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Metric {
    pub key: String,
    pub label: String,
    pub value: f64,
    pub unit: MetricUnit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionPreview {
    /// Single-use; expires at `expires_at_ms`.
    pub token: String,
    /// Normalized action (paths canonicalized, identities resolved).
    pub action: Action,
    pub origin: Origin,
    /// "Force kill Google Chrome Helper (PID 4821)".
    pub title: String,
    /// Plain-language statement of exactly what will happen.
    pub description: String,
    pub targets: Vec<PreviewTarget>,
    /// "Frees 2.3 GB", "Last modified 47 days ago".
    pub impact: Vec<Metric>,
    pub estimated_bytes_freed: Option<u64>,
    pub risk: ActionRisk,
    pub reversibility: Reversibility,
    /// e.g. "Owned by root — administrator authorization will be requested", "System process".
    pub warnings: Vec<String>,
    pub requires_elevation: bool,
    pub created_at_ms: TimestampMs,
    pub expires_at_ms: TimestampMs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OutcomeStatus {
    Succeeded,
    PartiallySucceeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemOutcome {
    pub label: String,
    /// The exact path this item acted on, when the action was path-based (trash, move). `None`
    /// for a process or firewall item, whose `label` is not a path. Lets the UI map a batch
    /// result back to the exact row it started from instead of matching display labels.
    pub path: Option<String>,
    pub success: bool,
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionOutcome {
    pub action: Action,
    pub origin: Origin,
    pub status: OutcomeStatus,
    pub items: Vec<ItemOutcome>,
    /// Measured, not estimated: e.g. volume used % before and after a trash batch.
    pub before: Vec<Metric>,
    pub after: Vec<Metric>,
    /// "Moved 14 items (8.7 GB) to Trash. Macintosh HD is now 62% used."
    pub summary: String,
    pub audit_id: i64,
    pub finished_at_ms: TimestampMs,
}

pub trait ActionPreparer: Send + Sync {
    fn prepare(&self, action: Action, origin: Origin) -> CoreResult<ActionPreview>;
}

pub trait ActionCommitter: Send + Sync {
    /// Re-validates (process identity unchanged, paths still exist) before executing.
    /// Fails with `ActionTokenInvalid` for unknown, expired, or already-used tokens.
    fn commit(&self, token: &str) -> CoreResult<ActionOutcome>;

    /// Records that a previewed action was declined, then invalidates the token.
    fn reject(&self, token: &str) -> CoreResult<()>;
}

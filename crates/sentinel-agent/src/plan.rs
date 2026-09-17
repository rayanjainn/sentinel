use serde::{Deserialize, Serialize};
use ts_rs::TS;

use sentinel_core::ErrorPayload;
use sentinel_core::action::{ActionOutcome, ActionPreview};
use sentinel_core::model::TimestampMs;

pub type PlanId = String;
pub type PlanActionId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlanStatus {
    AwaitingReview,
    Executing,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum PlanActionState {
    Pending,
    Rejected,
    Executing,
    Succeeded {
        outcome: ActionOutcome,
    },
    Failed {
        error: ErrorPayload,
    },
    /// Preview token expired before review; must be revised (re-prepared) to execute.
    Expired,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlanAction {
    pub id: PlanActionId,
    pub rationale: String,
    pub preview: ActionPreview,
    pub state: PlanActionState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Plan {
    pub id: PlanId,
    pub conversation_id: String,
    /// The user message that produced this plan.
    pub request: String,
    /// Model's plain-language explanation shown above the cards.
    pub explanation: String,
    pub actions: Vec<PlanAction>,
    pub status: PlanStatus,
    pub created_at_ms: TimestampMs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Decision {
    Approve,
    Reject,
}

/// Actions omitted from a decision list are treated as rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionDecision {
    pub action_id: PlanActionId,
    pub decision: Decision,
}

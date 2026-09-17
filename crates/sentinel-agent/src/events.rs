use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use sentinel_core::ErrorPayload;
use sentinel_core::model::TimestampMs;

use crate::backend::StopReason;
use crate::plan::Plan;
use crate::settings::ProviderId;
use crate::tools::ToolAccess;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AgentEvent {
    TurnStarted {
        turn_id: String,
        provider: ProviderId,
        model: String,
    },
    TextDelta {
        text: String,
    },
    ToolCallStarted {
        call_id: String,
        name: String,
        access: ToolAccess,
        input: Value,
    },
    ToolCallFinished {
        call_id: String,
        ok: bool,
        /// One-line human summary ("412 processes", "Scanned 214 GB").
        summary: String,
    },
    PlanProposed {
        plan: Plan,
    },
    PlanUpdated {
        plan: Plan,
    },
    TurnFinished {
        stop_reason: StopReason,
    },
    Error {
        error: ErrorPayload,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AgentStreamPayload {
    pub conversation_id: String,
    pub event: AgentEvent,
}

pub trait AgentEventSink: Send + Sync + 'static {
    fn emit(&self, payload: AgentStreamPayload);
}

/// Rendered conversation history, for restoring the chat panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum TranscriptItem {
    User {
        text: String,
        ts_ms: TimestampMs,
    },
    Assistant {
        text: String,
        ts_ms: TimestampMs,
    },
    ToolCall {
        call_id: String,
        name: String,
        access: ToolAccess,
        ok: Option<bool>,
        summary: Option<String>,
    },
    Plan {
        plan: Plan,
    },
}

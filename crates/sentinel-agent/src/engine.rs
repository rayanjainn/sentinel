//! The conversation loop.
//!
//! `AgentEngine` is built from [`EngineParts`], which holds a backend, a read executor, a write
//! translator and an action *preparer*. There is deliberately no way to give it anything that can
//! execute an action: every write tool call ends at `prepare`, producing a plan card, and the
//! model is told the action is waiting for the user. Execution lives in `executor::PlanExecutor`.

use std::sync::Arc;

use serde_json::{Value, json};
use tokio::sync::mpsc::unbounded_channel;
use tokio_util::sync::CancellationToken;

use sentinel_core::action::{Action, ActionPreparer, Origin};
use sentinel_core::{CoreResult, ErrorPayload, SentinelError};

use crate::backend::{
    AgentBackend, ChatMessage, ChatRequest, ContentBlock, Role, StopReason, StreamDelta,
};
use crate::conversation::{ConversationStore, new_id, now_ms};
use crate::events::{AgentEvent, AgentEventSink, AgentStreamPayload, TranscriptItem};
use crate::plan::{Plan, PlanAction, PlanActionState, PlanStatus};
use crate::prompt;
use crate::tools::read::summary_of;
use crate::tools::{ReadToolExecutor, ToolAccess, WriteToolTranslator, access_of, specs};

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub model: String,
    pub max_tool_rounds: u32,
    pub max_tokens: u32,
}

/// Everything the engine is allowed to hold. Adding a committer here is the one change that would
/// break the safety model; `tests/safety.rs` fails if this file ever mentions one.
pub struct EngineParts {
    pub backend: Arc<dyn AgentBackend>,
    pub reads: Arc<dyn ReadToolExecutor>,
    pub writes: Arc<dyn WriteToolTranslator>,
    pub preparer: Arc<dyn ActionPreparer>,
    pub store: Arc<ConversationStore>,
    pub sink: Arc<dyn AgentEventSink>,
    pub config: EngineConfig,
}

pub struct AgentEngine {
    parts: EngineParts,
}

/// Result of one tool call as returned to the model.
struct ToolOutcome {
    content: String,
    is_error: bool,
    summary: String,
}

impl ToolOutcome {
    fn error(message: impl Into<String>, summary: impl Into<String>) -> Self {
        Self {
            content: message.into(),
            is_error: true,
            summary: summary.into(),
        }
    }
}

struct TurnState {
    conversation_id: String,
    request: String,
    plan_id: String,
    proposed: Vec<(Action, PlanAction)>,
    explanation: Vec<String>,
}

enum Finish {
    Done(StopReason),
    Cancelled,
    Failed(SentinelError),
}

pub fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

/// UTC timestamp without a date library: days-from-civil inverse (Howard Hinnant).
pub fn iso_utc(ms: u64) -> String {
    let secs = ms / 1000;
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

impl AgentEngine {
    pub fn new(parts: EngineParts) -> Self {
        Self { parts }
    }

    fn emit(&self, conversation_id: &str, event: AgentEvent) {
        self.parts.sink.emit(AgentStreamPayload {
            conversation_id: conversation_id.to_owned(),
            event,
        });
    }

    /// Runs one user turn. Progress, plans and errors stream through the event sink; the returned
    /// error (if any) was already emitted as `AgentEvent::Error`.
    pub async fn send_message(
        &self,
        conversation_id: &str,
        text: &str,
        cancel: CancellationToken,
    ) -> CoreResult<()> {
        let text = text.trim();
        if text.is_empty() {
            return Err(SentinelError::invalid("message is empty"));
        }
        self.parts.store.with(conversation_id, |c| {
            c.transcript.push(TranscriptItem::User {
                text: text.to_owned(),
                ts_ms: now_ms(),
            });
        })?;
        self.run_turn(conversation_id, text, text.to_owned(), cancel)
            .await
    }

    /// Follow-up turn after a plan ran: the model receives the measured outcomes and reports them.
    pub async fn report_execution(&self, plan: &Plan, cancel: CancellationToken) -> CoreResult<()> {
        let outcomes: Vec<Value> = plan
            .actions
            .iter()
            .map(|a| {
                let mut row = json!({"action": a.preview.title, "reason": a.rationale});
                match &a.state {
                    PlanActionState::Succeeded { outcome }
                    | PlanActionState::PartiallySucceeded { outcome } => {
                        row["result"] = json!("executed");
                        row["status"] = json!(outcome.status);
                        row["summary"] = json!(outcome.summary);
                        row["before"] = json!(outcome.before);
                        row["after"] = json!(outcome.after);
                        row["items"] = json!(outcome.items);
                    }
                    PlanActionState::Failed { error } => {
                        row["result"] = json!("failed");
                        row["error"] = json!(error.message);
                    }
                    PlanActionState::Rejected => {
                        row["result"] = json!("rejected by the user; not executed")
                    }
                    PlanActionState::Expired => {
                        row["result"] = json!("preview expired before approval; not executed")
                    }
                    PlanActionState::Pending | PlanActionState::Executing => {
                        row["result"] = json!("not executed")
                    }
                }
                row
            })
            .collect();
        let report =
            prompt::execution_report(&serde_json::to_string_pretty(&outcomes).unwrap_or_default());
        self.run_turn(&plan.conversation_id, &plan.request, report, cancel)
            .await
    }

    async fn run_turn(
        &self,
        conversation_id: &str,
        request: &str,
        user_content: String,
        cancel: CancellationToken,
    ) -> CoreResult<()> {
        let parts = &self.parts;
        let model = parts.config.model.clone();
        let provider = parts.backend.provider();
        self.emit(
            conversation_id,
            AgentEvent::TurnStarted {
                turn_id: new_id(),
                provider,
                model: model.clone(),
            },
        );
        if !parts.backend.supports_tool_calling(&model) {
            let error = SentinelError::Unavailable {
                feature: "tool calling".into(),
                reason: format!(
                    "{model} cannot call tools, so it cannot read live system data or propose actions. Choose a model marked as supporting tools in Settings › AI agent."
                ),
            };
            return self.fail(conversation_id, error);
        }
        parts.store.with(conversation_id, |c| {
            c.messages.push(ChatMessage {
                role: Role::User,
                content: vec![ContentBlock::Text { text: user_content }],
            });
        })?;

        let mut turn = TurnState {
            conversation_id: conversation_id.to_owned(),
            request: request.to_owned(),
            plan_id: new_id(),
            proposed: Vec::new(),
            explanation: Vec::new(),
        };
        let finish = self.tool_loop(&mut turn, &model, cancel).await;

        if !turn.proposed.is_empty() {
            let explanation = turn.explanation.join("\n\n").trim().to_owned();
            let plan = Plan {
                id: turn.plan_id.clone(),
                conversation_id: conversation_id.to_owned(),
                request: turn.request.clone(),
                explanation: if explanation.is_empty() {
                    "Proposed changes awaiting your review.".to_owned()
                } else {
                    explanation
                },
                actions: turn.proposed.into_iter().map(|(_, a)| a).collect(),
                status: PlanStatus::AwaitingReview,
                created_at_ms: now_ms(),
            };
            parts.store.add_plan(plan.clone())?;
            self.emit(conversation_id, AgentEvent::PlanProposed { plan });
        }

        match finish {
            Finish::Done(stop_reason) => {
                self.emit(conversation_id, AgentEvent::TurnFinished { stop_reason });
                Ok(())
            }
            Finish::Cancelled => {
                self.emit(
                    conversation_id,
                    AgentEvent::TurnFinished {
                        stop_reason: StopReason::Other {
                            raw: "cancelled".into(),
                        },
                    },
                );
                Err(SentinelError::Cancelled)
            }
            Finish::Failed(error) => self.fail(conversation_id, error),
        }
    }

    fn fail(&self, conversation_id: &str, error: SentinelError) -> CoreResult<()> {
        self.emit(
            conversation_id,
            AgentEvent::Error {
                error: ErrorPayload::from(error.clone()),
            },
        );
        self.emit(
            conversation_id,
            AgentEvent::TurnFinished {
                stop_reason: StopReason::Other {
                    raw: "error".into(),
                },
            },
        );
        Err(error)
    }

    fn close_history(&self, conversation_id: &str, note: &str) {
        let _ = self.parts.store.with(conversation_id, |c| {
            if c.messages.last().is_some_and(|m| m.role == Role::User) {
                c.messages.push(ChatMessage {
                    role: Role::Assistant,
                    content: vec![ContentBlock::Text {
                        text: note.to_owned(),
                    }],
                });
            }
        });
    }

    async fn tool_loop(
        &self,
        turn: &mut TurnState,
        model: &str,
        cancel: CancellationToken,
    ) -> Finish {
        let parts = &self.parts;
        let conversation_id = turn.conversation_id.clone();
        let system = prompt::system_prompt(
            parts.backend.provider(),
            model,
            platform_name(),
            &iso_utc(now_ms()),
        );
        let tools = specs::definitions();
        // Writes are accepted only after read results reached the model in an earlier round.
        let mut grounded = false;
        let max_rounds = parts.config.max_tool_rounds.max(1);

        for round in 0..max_rounds {
            let messages = match parts.store.messages(&conversation_id) {
                Ok(m) => m,
                Err(e) => return Finish::Failed(e),
            };
            let request = ChatRequest {
                model: model.to_owned(),
                system: system.clone(),
                messages,
                tools: tools.clone(),
                max_tokens: parts.config.max_tokens,
                temperature: None,
            };

            let response = {
                let (tx, mut rx) = unbounded_channel();
                let stream = parts.backend.stream_response(&request, tx);
                tokio::pin!(stream);
                loop {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => {
                            self.close_history(&conversation_id, "(Stopped by the user.)");
                            return Finish::Cancelled;
                        }
                        Some(delta) = rx.recv() => {
                            if let StreamDelta::Text(text) = delta {
                                self.emit(&conversation_id, AgentEvent::TextDelta { text });
                            }
                        }
                        result = &mut stream => {
                            while let Ok(StreamDelta::Text(text)) = rx.try_recv() {
                                self.emit(&conversation_id, AgentEvent::TextDelta { text });
                            }
                            break result;
                        }
                    }
                }
            };
            let response = match response {
                Ok(r) => r,
                Err(error) => {
                    self.close_history(&conversation_id, "(The model request failed.)");
                    return Finish::Failed(error);
                }
            };

            let text: String = response
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            let calls: Vec<(String, String, Value)> = response
                .content
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::ToolUse { id, name, input } => {
                        Some((id.clone(), name.clone(), input.clone()))
                    }
                    _ => None,
                })
                .collect();
            let _ = parts.store.with(&conversation_id, |c| {
                c.messages.push(ChatMessage {
                    role: Role::Assistant,
                    content: if response.content.is_empty() {
                        vec![ContentBlock::Text {
                            text: String::new(),
                        }]
                    } else {
                        response.content.clone()
                    },
                });
                if !text.trim().is_empty() {
                    c.transcript.push(TranscriptItem::Assistant {
                        text: text.clone(),
                        ts_ms: now_ms(),
                    });
                }
            });
            if !text.trim().is_empty() {
                turn.explanation.push(text.trim().to_owned());
            }
            if calls.is_empty() {
                return Finish::Done(response.stop_reason);
            }

            let mut results = Vec::with_capacity(calls.len());
            let mut read_succeeded = false;
            let mut cancelled = false;
            for (call_id, name, input) in calls {
                if cancelled || cancel.is_cancelled() {
                    cancelled = true;
                    results.push(ContentBlock::ToolResult {
                        tool_use_id: call_id,
                        content: "Cancelled by the user before this tool ran.".into(),
                        is_error: true,
                    });
                    continue;
                }
                let access = access_of(&name);
                let shown_access = access.unwrap_or(ToolAccess::Read);
                self.emit(
                    &conversation_id,
                    AgentEvent::ToolCallStarted {
                        call_id: call_id.clone(),
                        name: name.clone(),
                        access: shown_access,
                        input: input.clone(),
                    },
                );
                let _ = parts.store.with(&conversation_id, |c| {
                    c.transcript.push(TranscriptItem::ToolCall {
                        call_id: call_id.clone(),
                        name: name.clone(),
                        access: shown_access,
                        ok: None,
                        summary: None,
                    });
                });

                let outcome = match access {
                    None => ToolOutcome::error(
                        format!(
                            "Unknown tool `{name}`. It was not run. Available tools: {}.",
                            crate::tools::TOOL_ACCESS
                                .iter()
                                .map(|(n, _)| *n)
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                        "Rejected: unknown tool",
                    ),
                    Some(ToolAccess::Read) => {
                        let call = parts.reads.call(&name, input);
                        tokio::select! {
                            biased;
                            _ = cancel.cancelled() => {
                                cancelled = true;
                                ToolOutcome::error("Cancelled by the user.", "Cancelled")
                            }
                            result = call => match result {
                                Ok(value) => {
                                    read_succeeded = true;
                                    ToolOutcome {
                                        summary: summary_of(&value),
                                        content: value.to_string(),
                                        is_error: false,
                                    }
                                }
                                Err(error) => ToolOutcome::error(
                                    format!("Tool failed: {error}"),
                                    error.to_string(),
                                ),
                            }
                        }
                    }
                    Some(ToolAccess::Write) if !grounded => ToolOutcome::error(
                        format!(
                            "Refused: `{name}` was not queued. Write tools are only accepted after you have read live system state with read tools earlier in this turn. Call the relevant read tools first, check the results, then propose the change."
                        ),
                        "Refused: no live data was read first",
                    ),
                    Some(ToolAccess::Write) => self.propose(turn, &name, input, model).await,
                };

                self.emit(
                    &conversation_id,
                    AgentEvent::ToolCallFinished {
                        call_id: call_id.clone(),
                        ok: !outcome.is_error,
                        summary: outcome.summary.clone(),
                    },
                );
                let _ = parts.store.with(&conversation_id, |c| {
                    for item in c.transcript.iter_mut().rev() {
                        if let TranscriptItem::ToolCall {
                            call_id: id,
                            ok,
                            summary,
                            ..
                        } = item
                            && *id == call_id
                        {
                            *ok = Some(!outcome.is_error);
                            *summary = Some(outcome.summary.clone());
                            break;
                        }
                    }
                });
                results.push(ContentBlock::ToolResult {
                    tool_use_id: call_id,
                    content: outcome.content,
                    is_error: outcome.is_error,
                });
            }
            let _ = parts.store.with(&conversation_id, |c| {
                c.messages.push(ChatMessage {
                    role: Role::User,
                    content: results,
                });
            });
            if cancelled {
                self.close_history(&conversation_id, "(Stopped by the user.)");
                return Finish::Cancelled;
            }
            grounded |= read_succeeded;

            if round + 1 == max_rounds {
                self.close_history(
                    &conversation_id,
                    &format!("(Stopped after {max_rounds} tool rounds.)"),
                );
                return Finish::Done(StopReason::Other {
                    raw: "max_tool_rounds".into(),
                });
            }
        }
        Finish::Done(StopReason::EndTurn)
    }

    /// Translate → prepare → queue as a plan card. Never executes.
    async fn propose(
        &self,
        turn: &mut TurnState,
        name: &str,
        input: Value,
        model: &str,
    ) -> ToolOutcome {
        let parts = &self.parts;
        let writes = Arc::clone(&parts.writes);
        let tool = name.to_owned();
        let translated = tokio::task::spawn_blocking(move || writes.translate(&tool, input))
            .await
            .map_err(SentinelError::internal)
            .and_then(|r| r);
        let proposal = match translated {
            Ok(p) => p,
            Err(error) => {
                return ToolOutcome::error(
                    format!("Not queued: {error}. Nothing was changed."),
                    format!("Not queued: {error}"),
                );
            }
        };
        if let Some((_, existing)) = turn
            .proposed
            .iter()
            .find(|(action, _)| *action == proposal.action)
        {
            return ToolOutcome {
                content: json!({
                    "status": "already_queued_for_user_review",
                    "executed": false,
                    "plan_action_id": existing.id,
                    "note": "This exact action is already in the plan and has NOT been performed. Do not propose it again.",
                })
                .to_string(),
                is_error: false,
                summary: format!("Already proposed: {}", existing.preview.title),
            };
        }
        let origin = Origin::Agent {
            conversation_id: turn.conversation_id.clone(),
            plan_id: turn.plan_id.clone(),
            provider: parts.backend.provider().display_name().to_owned(),
            model: model.to_owned(),
            request: turn.request.clone(),
        };
        let preparer = Arc::clone(&parts.preparer);
        let action = proposal.action.clone();
        let prepared = tokio::task::spawn_blocking(move || preparer.prepare(action, origin))
            .await
            .map_err(SentinelError::internal)
            .and_then(|r| r);
        let preview = match prepared {
            Ok(preview) => preview,
            Err(error) => {
                return ToolOutcome::error(
                    format!("Not queued: {error}. Nothing was changed."),
                    format!("Not queued: {error}"),
                );
            }
        };
        let plan_action = PlanAction {
            id: new_id(),
            rationale: proposal.rationale,
            preview,
            state: PlanActionState::Pending,
        };
        let content = json!({
            "status": "queued_for_user_review",
            "executed": false,
            "plan_action_id": plan_action.id,
            "title": plan_action.preview.title,
            "description": plan_action.preview.description,
            "targets": plan_action.preview.targets.iter().map(|t| json!({"label": t.label, "problem": t.problem})).collect::<Vec<_>>(),
            "estimated_bytes_freed": plan_action.preview.estimated_bytes_freed,
            "warnings": plan_action.preview.warnings,
            "requires_elevation": plan_action.preview.requires_elevation,
            "note": "This action has NOT been performed. It is a card in the plan and runs only if the user approves it. Tell the user it is proposed and awaiting their approval; never say it is done.",
        })
        .to_string();
        let summary = format!("Proposed for your review: {}", plan_action.preview.title);
        turn.proposed.push((proposal.action, plan_action));
        ToolOutcome {
            content,
            is_error: false,
            summary,
        }
    }
}

/// Names of read tools, for tests and UI hints.
pub fn read_tool_names() -> Vec<&'static str> {
    crate::tools::TOOL_ACCESS
        .iter()
        .filter(|(_, a)| *a == ToolAccess::Read)
        .map(|(n, _)| *n)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_789_000_000_000), "2026-09-10T00:26:40Z");
        assert!(read_tool_names().contains(&crate::tools::names::LIST_PROCESSES));
    }
}

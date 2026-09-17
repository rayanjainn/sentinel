//! Plan execution: the only place in the agent layer that holds an `ActionCommitter`.
//!
//! `PlanExecutor` runs only from the `agent_execute_plan` / `agent_revise_plan_action` commands,
//! with explicit per-action decisions made by a person. Approved actions are committed one at a
//! time in plan order; rejected or omitted actions are declined through the same pipeline so the
//! audit log records them.

use std::collections::HashMap;
use std::mem::discriminant;
use std::sync::Arc;

use sentinel_core::action::{Action, ActionCommitter, ActionPreparer, OutcomeStatus};
use sentinel_core::{CoreResult, ErrorPayload, SentinelError};

use crate::conversation::{ConversationStore, now_ms};
use crate::events::{AgentEvent, AgentEventSink, AgentStreamPayload};
use crate::plan::{ActionDecision, Decision, Plan, PlanActionState, PlanStatus};

pub struct PlanExecutor {
    committer: Arc<dyn ActionCommitter>,
    preparer: Arc<dyn ActionPreparer>,
    store: Arc<ConversationStore>,
    sink: Arc<dyn AgentEventSink>,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> CoreResult<T> + Send + 'static,
) -> CoreResult<T> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(SentinelError::internal)?
}

/// Maps a committed outcome onto a card state. Only a fully successful outcome is `Succeeded`.
pub fn state_for_outcome(outcome: sentinel_core::action::ActionOutcome) -> PlanActionState {
    match outcome.status {
        OutcomeStatus::Succeeded => PlanActionState::Succeeded { outcome },
        OutcomeStatus::PartiallySucceeded => PlanActionState::PartiallySucceeded { outcome },
        OutcomeStatus::Failed => {
            let error = outcome
                .items
                .iter()
                .find_map(|item| item.error.clone())
                .unwrap_or_else(|| {
                    ErrorPayload::from(SentinelError::Internal {
                        detail: outcome.summary.clone(),
                    })
                });
            PlanActionState::Failed { error }
        }
    }
}

/// An edit may narrow what was proposed or adjust its parameters, never retarget it.
pub fn validate_revision(original: &Action, revised: &Action) -> CoreResult<()> {
    if discriminant(original) != discriminant(revised) {
        return Err(SentinelError::invalid(
            "an edit cannot change the kind of action",
        ));
    }
    let subset = |before: &[String], after: &[String]| -> CoreResult<()> {
        if after.is_empty() {
            return Err(SentinelError::invalid(
                "keep at least one path, or reject the action instead",
            ));
        }
        if let Some(extra) = after.iter().find(|p| !before.contains(p)) {
            return Err(SentinelError::invalid(format!(
                "{extra} was not part of the proposal; edits can only remove paths"
            )));
        }
        Ok(())
    };
    match (original, revised) {
        (Action::TerminateProcess { target: a }, Action::TerminateProcess { target: b })
        | (Action::ForceKillProcess { target: a }, Action::ForceKillProcess { target: b })
            if a.pid != b.pid =>
        {
            Err(SentinelError::invalid(
                "an edit cannot target a different process",
            ))
        }
        (
            Action::SetProcessPriority { target: a, .. },
            Action::SetProcessPriority { target: b, nice },
        ) => {
            if a.pid != b.pid {
                return Err(SentinelError::invalid(
                    "an edit cannot target a different process",
                ));
            }
            if !(-20..=19).contains(nice) {
                return Err(SentinelError::invalid("nice must be between -20 and 19"));
            }
            Ok(())
        }
        (Action::TrashPaths { paths: before }, Action::TrashPaths { paths: after }) => {
            subset(before, after)
        }
        (
            Action::MovePaths { paths: before, .. },
            Action::MovePaths {
                paths: after,
                destination_dir,
            },
        ) => {
            subset(before, after)?;
            if destination_dir.trim().is_empty() {
                return Err(SentinelError::invalid("choose a destination folder"));
            }
            Ok(())
        }
        (Action::AddFirewallRule { target: a, .. }, Action::AddFirewallRule { target: b, .. })
            if a != b =>
        {
            Err(SentinelError::invalid(
                "an edit cannot change the firewall target",
            ))
        }
        (Action::RemoveFirewallRule { rule_id: a }, Action::RemoveFirewallRule { rule_id: b })
            if a != b =>
        {
            Err(SentinelError::invalid(
                "an edit cannot change which rule is removed",
            ))
        }
        _ => Ok(()),
    }
}

impl PlanExecutor {
    pub fn new(
        committer: Arc<dyn ActionCommitter>,
        preparer: Arc<dyn ActionPreparer>,
        store: Arc<ConversationStore>,
        sink: Arc<dyn AgentEventSink>,
    ) -> Self {
        Self {
            committer,
            preparer,
            store,
            sink,
        }
    }

    fn publish(&self, plan: &Plan) -> CoreResult<()> {
        self.store.update_plan(plan)?;
        self.sink.emit(AgentStreamPayload {
            conversation_id: plan.conversation_id.clone(),
            event: AgentEvent::PlanUpdated { plan: plan.clone() },
        });
        Ok(())
    }

    /// Commits approved actions in order and declines everything else. Omitted actions count as
    /// rejected. A plan runs once; running it again fails with `ActionTokenInvalid`.
    pub async fn execute(&self, plan_id: &str, decisions: &[ActionDecision]) -> CoreResult<Plan> {
        let mut plan = self.store.plan(plan_id)?;
        if plan.status != PlanStatus::AwaitingReview {
            return Err(SentinelError::ActionTokenInvalid);
        }
        let mut by_id: HashMap<&str, Decision> = HashMap::new();
        for decision in decisions {
            if !plan.actions.iter().any(|a| a.id == decision.action_id) {
                return Err(SentinelError::invalid(format!(
                    "plan has no action `{}`",
                    decision.action_id
                )));
            }
            if by_id
                .insert(decision.action_id.as_str(), decision.decision)
                .is_some()
            {
                return Err(SentinelError::invalid(format!(
                    "action `{}` has more than one decision",
                    decision.action_id
                )));
            }
        }

        plan.status = PlanStatus::Executing;
        self.publish(&plan)?;

        for index in 0..plan.actions.len() {
            let action = &plan.actions[index];
            let approved = by_id.get(action.id.as_str()) == Some(&Decision::Approve);
            let token = action.preview.token.clone();
            let next = match (&action.state, approved) {
                (PlanActionState::Pending, true) if action.preview.expires_at_ms <= now_ms() => {
                    let committer = Arc::clone(&self.committer);
                    // Releases the stale token; it would fail to commit anyway.
                    let _ = blocking(move || committer.reject(&token)).await;
                    PlanActionState::Expired
                }
                (PlanActionState::Pending, true) => {
                    plan.actions[index].state = PlanActionState::Executing;
                    self.publish(&plan)?;
                    let committer = Arc::clone(&self.committer);
                    match blocking(move || committer.commit(&token)).await {
                        Ok(outcome) => state_for_outcome(outcome),
                        Err(SentinelError::ActionTokenInvalid) => PlanActionState::Expired,
                        Err(error) => PlanActionState::Failed {
                            error: ErrorPayload::from(error),
                        },
                    }
                }
                (PlanActionState::Pending, false) => {
                    let committer = Arc::clone(&self.committer);
                    match blocking(move || committer.reject(&token)).await {
                        Ok(()) | Err(SentinelError::ActionTokenInvalid) => {
                            PlanActionState::Rejected
                        }
                        Err(error) => PlanActionState::Failed {
                            error: ErrorPayload::from(error),
                        },
                    }
                }
                (PlanActionState::Expired, false) => PlanActionState::Rejected,
                (state, _) => state.clone(),
            };
            plan.actions[index].state = next;
            self.publish(&plan)?;
        }

        plan.status = PlanStatus::Completed;
        self.publish(&plan)?;
        Ok(plan)
    }

    /// Replaces one card's action with an edited version, re-prepared through the pipeline.
    pub async fn revise(
        &self,
        plan_id: &str,
        action_id: &str,
        revised: Action,
    ) -> CoreResult<Plan> {
        let mut plan = self.store.plan(plan_id)?;
        if plan.status != PlanStatus::AwaitingReview {
            return Err(SentinelError::invalid(
                "this plan has already run; ask the agent for a new one",
            ));
        }
        let index = plan
            .actions
            .iter()
            .position(|a| a.id == action_id)
            .ok_or_else(|| SentinelError::invalid(format!("plan has no action `{action_id}`")))?;
        let current = &plan.actions[index];
        if !matches!(
            current.state,
            PlanActionState::Pending | PlanActionState::Expired
        ) {
            return Err(SentinelError::invalid(
                "only pending or expired actions can be edited",
            ));
        }
        validate_revision(&current.preview.action, &revised)?;

        let origin = current.preview.origin.clone();
        let preparer = Arc::clone(&self.preparer);
        let preview = blocking(move || preparer.prepare(revised, origin)).await?;
        let old_token = std::mem::replace(&mut plan.actions[index].preview, preview).token;
        let committer = Arc::clone(&self.committer);
        // The old token is superseded; an already-expired one needs no release.
        let _ = blocking(move || committer.reject(&old_token)).await;
        plan.actions[index].state = PlanActionState::Pending;
        self.publish(&plan)?;
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use sentinel_core::model::{FirewallTarget, ProcessIdentity, TrafficDirection};

    use super::*;

    fn identity(pid: u32) -> ProcessIdentity {
        ProcessIdentity {
            pid,
            start_time: 100,
        }
    }

    #[test]
    fn revisions_can_only_narrow() {
        let trash = Action::TrashPaths {
            paths: vec!["/a".into(), "/b".into()],
        };
        assert!(
            validate_revision(
                &trash,
                &Action::TrashPaths {
                    paths: vec!["/a".into()]
                }
            )
            .is_ok()
        );
        assert!(validate_revision(&trash, &Action::TrashPaths { paths: vec![] }).is_err());
        assert!(
            validate_revision(
                &trash,
                &Action::TrashPaths {
                    paths: vec!["/c".into()]
                }
            )
            .is_err()
        );
        assert!(
            validate_revision(
                &trash,
                &Action::MovePaths {
                    paths: vec!["/a".into()],
                    destination_dir: "/x".into()
                }
            )
            .is_err()
        );

        let nice = Action::SetProcessPriority {
            target: identity(5),
            nice: 10,
        };
        assert!(
            validate_revision(
                &nice,
                &Action::SetProcessPriority {
                    target: identity(5),
                    nice: 5
                }
            )
            .is_ok()
        );
        assert!(
            validate_revision(
                &nice,
                &Action::SetProcessPriority {
                    target: identity(6),
                    nice: 5
                }
            )
            .is_err()
        );
        assert!(
            validate_revision(
                &nice,
                &Action::SetProcessPriority {
                    target: identity(5),
                    nice: 40
                }
            )
            .is_err()
        );

        let kill = Action::TerminateProcess {
            target: identity(5),
        };
        assert!(
            validate_revision(
                &kill,
                &Action::TerminateProcess {
                    target: identity(9)
                }
            )
            .is_err()
        );

        let block = Action::AddFirewallRule {
            target: FirewallTarget::RemoteIp {
                ip: "203.0.113.9".into(),
            },
            direction: TrafficDirection::Both,
        };
        assert!(
            validate_revision(
                &block,
                &Action::AddFirewallRule {
                    target: FirewallTarget::RemoteIp {
                        ip: "203.0.113.9".into()
                    },
                    direction: TrafficDirection::Inbound,
                }
            )
            .is_ok()
        );
        assert!(
            validate_revision(
                &block,
                &Action::AddFirewallRule {
                    target: FirewallTarget::RemoteIp {
                        ip: "198.51.100.1".into()
                    },
                    direction: TrafficDirection::Inbound,
                }
            )
            .is_err()
        );
    }
}

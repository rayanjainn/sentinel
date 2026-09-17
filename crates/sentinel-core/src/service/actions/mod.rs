//! `ActionService`: the one implementation of `ActionPreparer` + `ActionCommitter`.
//!
//! `prepare` validates against live state and records a single-use token; `commit` re-validates,
//! executes through the control traits, measures before/after on the live system and writes the
//! audit entry. UI buttons and the agent reach exactly this code.

mod files;
mod firewall;
mod process;

pub use files::PathSize;

use std::path::Path;
use std::sync::Arc;

use crate::action::{
    Action, ActionCommitter, ActionOutcome, ActionPreparer, ActionPreview, ActionRisk, ItemOutcome,
    Metric, MetricUnit, Origin, OutcomeStatus, PreviewTarget, Reversibility,
};
use crate::audit::{AuditEntry, AuditStatus, AuditStore};
use crate::error::{CoreResult, SentinelError};
use crate::model::{Pid, Platform, ProcessInfo};
use crate::provider::{FileOps, ProcessControl};
use crate::service::tokens::{TOKEN_TTL, TokenStore};
use crate::util::now_ms;

/// Live-state lookups the action pipeline needs from the runtime.
pub trait ActionContext: Send + Sync {
    fn lookup_process(&self, pid: Pid) -> CoreResult<ProcessInfo>;

    fn file_ops(&self) -> CoreResult<Arc<dyn FileOps>> {
        Err(SentinelError::Unavailable {
            feature: "file actions".to_owned(),
            reason: "no file operations are available in this context".to_owned(),
        })
    }

    /// Size of a path from the latest covering scan, else a bounded walk.
    fn path_size(&self, _path: &Path) -> Option<PathSize> {
        None
    }

    /// (total, available) bytes of the volume holding `path`.
    fn volume_usage(&self, _path: &Path) -> CoreResult<(u64, u64)> {
        Err(SentinelError::Unavailable {
            feature: "volume usage".to_owned(),
            reason: "no storage provider in this context".to_owned(),
        })
    }

    /// Display name of the volume holding `path` ("Macintosh HD").
    fn volume_label(&self, _path: &Path) -> Option<String> {
        None
    }

    fn firewall(&self) -> CoreResult<Arc<crate::service::firewall::FirewallService>> {
        Err(SentinelError::Unavailable {
            feature: "firewall".to_owned(),
            reason: "no firewall in this context".to_owned(),
        })
    }

    /// "postgres (PID 1555)" labels for processes whose sockets a firewall target would affect.
    fn traffic_users(&self, _target: &crate::model::FirewallTarget) -> Vec<String> {
        Vec::new()
    }

    /// Whether two paths live on the same volume; `None` when unknown.
    fn same_volume(&self, _a: &Path, _b: &Path) -> Option<bool> {
        None
    }
}

pub struct ActionServiceConfig {
    pub platform: Platform,
    pub running_elevated: bool,
}

pub struct ActionService {
    context: Arc<dyn ActionContext>,
    process_control: Arc<dyn ProcessControl>,
    audit: Arc<dyn AuditStore>,
    tokens: TokenStore,
    config: ActionServiceConfig,
}

/// Everything in a preview except the token bookkeeping.
pub(crate) struct Draft {
    pub action: Action,
    pub title: String,
    pub description: String,
    pub targets: Vec<PreviewTarget>,
    pub impact: Vec<Metric>,
    pub estimated_bytes_freed: Option<u64>,
    pub risk: ActionRisk,
    pub reversibility: Reversibility,
    pub warnings: Vec<String>,
    pub requires_elevation: bool,
}

/// Result of executing a previewed action, before it is audited.
pub(crate) struct Execution {
    pub status: OutcomeStatus,
    pub items: Vec<ItemOutcome>,
    pub before: Vec<Metric>,
    pub after: Vec<Metric>,
    pub summary: String,
    pub bytes_freed: Option<u64>,
    pub affected_paths: Vec<String>,
}

impl Execution {
    pub fn failed(label: String, error: SentinelError, before: Vec<Metric>) -> Self {
        Self {
            status: OutcomeStatus::Failed,
            summary: format!("Could not complete: {error}"),
            items: vec![ItemOutcome {
                label,
                success: false,
                error: Some(error.into()),
            }],
            before,
            after: Vec::new(),
            bytes_freed: None,
            affected_paths: Vec::new(),
        }
    }
}

pub(crate) fn metric(key: &str, label: &str, value: f64, unit: MetricUnit) -> Metric {
    Metric {
        key: key.to_owned(),
        label: label.to_owned(),
        value,
        unit,
    }
}

impl ActionService {
    pub fn new(
        context: Arc<dyn ActionContext>,
        process_control: Arc<dyn ProcessControl>,
        audit: Arc<dyn AuditStore>,
        config: ActionServiceConfig,
    ) -> Self {
        Self {
            context,
            process_control,
            audit,
            tokens: TokenStore::default(),
            config,
        }
    }

    fn draft(&self, action: Action) -> CoreResult<Draft> {
        match action {
            Action::TerminateProcess { target } => process::preview_stop(self, target, false),
            Action::ForceKillProcess { target } => process::preview_stop(self, target, true),
            Action::SetProcessPriority { target, nice } => {
                process::preview_priority(self, target, nice)
            }
            Action::TrashPaths { paths } => files::preview_trash(self, paths),
            Action::MovePaths {
                paths,
                destination_dir,
            } => files::preview_move(self, paths, destination_dir),
            Action::AddFirewallRule { target, direction } => {
                firewall::preview_add(self, target, direction)
            }
            Action::RemoveFirewallRule { rule_id } => firewall::preview_remove(self, rule_id),
        }
    }

    fn execute(&self, preview: &ActionPreview) -> Execution {
        match &preview.action {
            Action::TerminateProcess { target } => {
                process::execute_stop(self, preview, target, false)
            }
            Action::ForceKillProcess { target } => {
                process::execute_stop(self, preview, target, true)
            }
            Action::SetProcessPriority { target, nice } => {
                process::execute_priority(self, preview, target, *nice)
            }
            Action::TrashPaths { paths } => files::execute_trash(self, preview, paths),
            Action::MovePaths {
                paths,
                destination_dir,
            } => files::execute_move(self, preview, paths, destination_dir),
            Action::AddFirewallRule { target, direction } => {
                firewall::execute_add(self, preview, target, *direction)
            }
            Action::RemoveFirewallRule { rule_id } => {
                firewall::execute_remove(self, preview, rule_id)
            }
        }
    }

    fn record(
        &self,
        preview: &ActionPreview,
        status: AuditStatus,
        execution: &Execution,
    ) -> CoreResult<i64> {
        let trigger = match &preview.origin {
            Origin::Agent { request, .. } => Some(request.clone()),
            Origin::User => None,
        };
        self.audit.append(AuditEntry {
            id: 0,
            ts_ms: now_ms(),
            origin: preview.origin.clone(),
            trigger,
            action: preview.action.clone(),
            title: preview.title.clone(),
            status,
            summary: execution.summary.clone(),
            bytes_freed: execution.bytes_freed,
            affected_paths: execution.affected_paths.clone(),
            before: execution.before.clone(),
            after: execution.after.clone(),
        })
    }
}

impl ActionPreparer for ActionService {
    fn prepare(&self, action: Action, origin: Origin) -> CoreResult<ActionPreview> {
        let draft = self.draft(action)?;
        let created_at_ms = now_ms();
        let preview = ActionPreview {
            token: TokenStore::new_token(),
            action: draft.action,
            origin,
            title: draft.title,
            description: draft.description,
            targets: draft.targets,
            impact: draft.impact,
            estimated_bytes_freed: draft.estimated_bytes_freed,
            risk: draft.risk,
            reversibility: draft.reversibility,
            warnings: draft.warnings,
            requires_elevation: draft.requires_elevation,
            created_at_ms,
            expires_at_ms: created_at_ms + TOKEN_TTL.as_millis() as u64,
        };
        self.tokens.insert(preview.clone());
        Ok(preview)
    }
}

impl ActionCommitter for ActionService {
    fn commit(&self, token: &str) -> CoreResult<ActionOutcome> {
        let preview = self.tokens.take(token)?;
        let execution = self.execute(&preview);
        let status = match execution.status {
            OutcomeStatus::Succeeded => AuditStatus::Succeeded,
            OutcomeStatus::PartiallySucceeded => AuditStatus::PartiallySucceeded,
            OutcomeStatus::Failed => AuditStatus::Failed,
        };
        let audit_id = self.record(&preview, status, &execution)?;
        if let Action::AddFirewallRule { target, direction } = &preview.action
            && execution.status == OutcomeStatus::Succeeded
            && let Ok(firewall) = self.context.firewall()
            && let Some(rule) = firewall.find(target, *direction)
        {
            firewall.set_audit_id(&rule.id, audit_id)?;
        }
        Ok(ActionOutcome {
            action: preview.action,
            origin: preview.origin,
            status: execution.status,
            items: execution.items,
            before: execution.before,
            after: execution.after,
            summary: execution.summary,
            audit_id,
            finished_at_ms: now_ms(),
        })
    }

    fn reject(&self, token: &str) -> CoreResult<()> {
        let preview = self.tokens.take(token)?;
        let execution = Execution {
            status: OutcomeStatus::Failed,
            items: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            summary: format!("Declined: {}", preview.title),
            bytes_freed: None,
            affected_paths: Vec::new(),
        };
        self.record(&preview, AuditStatus::Rejected, &execution)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

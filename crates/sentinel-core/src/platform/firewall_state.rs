//! Rules Sentinel loaded into a non-persistent firewall (pf anchors, nftables tables) during the
//! current boot. Reading the live ruleset needs root, so without privileges this record — keyed by
//! boot identity so a reboot marks everything inactive — is the source of truth.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CoreResult, SentinelError};
use crate::model::FirewallRule;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Loaded {
    boot: String,
    rules: Vec<FirewallRule>,
}

pub(crate) struct LoadedState {
    path: PathBuf,
}

impl LoadedState {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn rules(&self, boot: &str) -> Vec<FirewallRule> {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str::<Loaded>(&text).ok())
            .filter(|loaded| loaded.boot == boot)
            .map(|loaded| loaded.rules)
            .unwrap_or_default()
    }

    pub fn save(&self, boot: &str, rules: &[FirewallRule]) -> CoreResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| SentinelError::io(&err, Some(parent)))?;
        }
        let json = serde_json::to_vec_pretty(&Loaded {
            boot: boot.to_owned(),
            rules: rules.to_vec(),
        })
        .map_err(SentinelError::internal)?;
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, json).map_err(|err| SentinelError::io(&err, Some(&temp)))?;
        std::fs::rename(&temp, &self.path).map_err(|err| SentinelError::io(&err, Some(&self.path)))
    }
}

/// Replaces or adds `rule` in `rules` by id.
pub(crate) fn with_rule(mut rules: Vec<FirewallRule>, rule: &FirewallRule) -> Vec<FirewallRule> {
    rules.retain(|r| r.id != rule.id);
    rules.push(rule.clone());
    rules
}

pub(crate) fn without_rule(mut rules: Vec<FirewallRule>, rule_id: &str) -> Vec<FirewallRule> {
    rules.retain(|r| r.id != rule_id);
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FirewallBackend, FirewallTarget, TrafficDirection};

    fn rule(id: &str) -> FirewallRule {
        FirewallRule {
            id: id.into(),
            target: FirewallTarget::RemoteIp {
                ip: "203.0.113.1".into(),
            },
            direction: TrafficDirection::Both,
            backend: FirewallBackend::Pf,
            created_at_ms: 1,
            audit_id: None,
            active: true,
        }
    }

    #[test]
    fn loaded_rules_expire_with_the_boot() {
        let dir = tempfile::tempdir().unwrap();
        let state = LoadedState::new(dir.path().join("fw/loaded.json"));
        assert!(state.rules("boot-1").is_empty());
        let rules = with_rule(with_rule(Vec::new(), &rule("a")), &rule("b"));
        state.save("boot-1", &rules).unwrap();
        assert_eq!(state.rules("boot-1").len(), 2);
        assert!(
            state.rules("boot-2").is_empty(),
            "a reboot clears loaded rules"
        );
        assert_eq!(without_rule(rules, "a").len(), 1);
    }
}

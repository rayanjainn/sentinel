//! Sentinel-owned firewall rules: persisted in the app data dir, reconciled against what the OS
//! firewall actually has loaded, and changed only through `FirewallProvider`.

pub mod render;

use std::cmp::Reverse;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::error::{CoreResult, SentinelError};
use crate::model::{FirewallRule, FirewallStatus, FirewallTarget, TrafficDirection};
use crate::provider::FirewallProvider;
use crate::util::now_ms;

const INSTALLED_CACHE: Duration = Duration::from_secs(5);

pub struct FirewallService {
    provider: Arc<dyn FirewallProvider>,
    path: PathBuf,
    rules: Mutex<Vec<FirewallRule>>,
    installed: Mutex<Option<(Instant, Vec<String>)>>,
}

fn same_target(a: &FirewallTarget, b: &FirewallTarget) -> bool {
    match (a, b) {
        (FirewallTarget::RemoteIp { ip: x }, FirewallTarget::RemoteIp { ip: y }) => {
            match (x.parse::<std::net::IpAddr>(), y.parse::<std::net::IpAddr>()) {
                (Ok(x), Ok(y)) => x == y,
                _ => x == y,
            }
        }
        (a, b) => a == b,
    }
}

impl FirewallService {
    pub fn new(provider: Arc<dyn FirewallProvider>, path: PathBuf) -> Self {
        let rules = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<FirewallRule>>(&text).ok())
            .unwrap_or_default();
        Self {
            provider,
            path,
            rules: Mutex::new(rules),
            installed: Mutex::new(None),
        }
    }

    pub fn status(&self) -> FirewallStatus {
        self.provider.status()
    }

    fn persist(&self, rules: &[FirewallRule]) -> CoreResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| SentinelError::io(&err, Some(parent)))?;
        }
        let json = serde_json::to_vec_pretty(rules).map_err(SentinelError::internal)?;
        let temp = self.path.with_extension("json.tmp");
        std::fs::write(&temp, json).map_err(|err| SentinelError::io(&err, Some(&temp)))?;
        std::fs::rename(&temp, &self.path).map_err(|err| SentinelError::io(&err, Some(&self.path)))
    }

    fn installed_ids(&self, fresh: bool) -> CoreResult<Vec<String>> {
        if !fresh
            && let Some((at, ids)) = &*self.installed.lock()
            && at.elapsed() < INSTALLED_CACHE
        {
            return Ok(ids.clone());
        }
        let ids = self.provider.installed_rule_ids()?;
        *self.installed.lock() = Some((Instant::now(), ids.clone()));
        Ok(ids)
    }

    /// Sentinel's rules with `active` reconciled against the OS firewall.
    pub fn list(&self) -> CoreResult<Vec<FirewallRule>> {
        let installed = self.installed_ids(false)?;
        let mut rules = self.rules.lock().clone();
        for rule in &mut rules {
            rule.active = installed.contains(&rule.id);
        }
        rules.sort_by_key(|r| Reverse(r.created_at_ms));
        Ok(rules)
    }

    pub fn get(&self, rule_id: &str) -> Option<FirewallRule> {
        self.rules.lock().iter().find(|r| r.id == rule_id).cloned()
    }

    pub fn find(
        &self,
        target: &FirewallTarget,
        direction: TrafficDirection,
    ) -> Option<FirewallRule> {
        self.rules
            .lock()
            .iter()
            .find(|r| r.direction == direction && same_target(&r.target, target))
            .cloned()
    }

    pub fn is_active(&self, rule_id: &str) -> bool {
        self.installed_ids(true)
            .map(|ids| ids.iter().any(|id| id == rule_id))
            .unwrap_or(false)
    }

    /// Installs a rule, re-applying the stored one when the same block already exists (e.g. after
    /// a reboot cleared it).
    pub fn add(
        &self,
        target: FirewallTarget,
        direction: TrafficDirection,
    ) -> CoreResult<FirewallRule> {
        render::validate_target(&target)?;
        let status = self.status();
        let backend = status.backend.filter(|_| status.available).ok_or_else(|| {
            SentinelError::Unavailable {
                feature: "firewall".to_owned(),
                reason: status
                    .note
                    .clone()
                    .unwrap_or_else(|| "no supported firewall was found".to_owned()),
            }
        })?;
        let mut rule = self
            .find(&target, direction)
            .unwrap_or_else(|| FirewallRule {
                id: uuid::Uuid::new_v4().simple().to_string()[..12].to_owned(),
                target,
                direction,
                backend,
                created_at_ms: now_ms(),
                audit_id: None,
                active: false,
            });
        self.provider.install(&rule)?;
        *self.installed.lock() = None;
        rule.active = true;
        let mut rules = self.rules.lock();
        rules.retain(|r| r.id != rule.id);
        rules.push(rule.clone());
        self.persist(&rules)?;
        Ok(rule)
    }

    pub fn remove(&self, rule_id: &str) -> CoreResult<FirewallRule> {
        let rule = self.get(rule_id).ok_or_else(|| {
            SentinelError::invalid(format!("Sentinel has no firewall rule {rule_id}"))
        })?;
        self.provider.remove(&rule)?;
        *self.installed.lock() = None;
        let mut rules = self.rules.lock();
        rules.retain(|r| r.id != rule_id);
        self.persist(&rules)?;
        Ok(rule)
    }

    pub fn set_audit_id(&self, rule_id: &str, audit_id: i64) -> CoreResult<()> {
        let mut rules = self.rules.lock();
        if let Some(rule) = rules.iter_mut().find(|r| r.id == rule_id) {
            rule.audit_id = Some(audit_id);
            self.persist(&rules)?;
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.rules.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{FirewallBackend, TransportProtocol};

    #[derive(Default)]
    struct FakeFirewall {
        loaded: Mutex<Vec<String>>,
        decline: bool,
    }

    impl FirewallProvider for FakeFirewall {
        fn status(&self) -> FirewallStatus {
            FirewallStatus {
                backend: Some(FirewallBackend::Nftables),
                available: true,
                firewall_enabled: None,
                requires_elevation: true,
                note: None,
            }
        }
        fn install(&self, rule: &FirewallRule) -> CoreResult<()> {
            if self.decline {
                return Err(SentinelError::ElevationDeclined {
                    operation: "change firewall rules".into(),
                });
            }
            self.loaded.lock().push(rule.id.clone());
            Ok(())
        }
        fn remove(&self, rule: &FirewallRule) -> CoreResult<()> {
            self.loaded.lock().retain(|id| id != &rule.id);
            Ok(())
        }
        fn installed_rule_ids(&self) -> CoreResult<Vec<String>> {
            Ok(self.loaded.lock().clone())
        }
    }

    #[test]
    fn adds_reconciles_reapplies_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("firewall/rules.json");
        let provider = Arc::new(FakeFirewall::default());
        let service = FirewallService::new(provider.clone(), path.clone());
        let target = FirewallTarget::LocalPort {
            port: 5432,
            protocol: TransportProtocol::Tcp,
        };
        let rule = service
            .add(target.clone(), TrafficDirection::Inbound)
            .unwrap();
        assert!(rule.active);
        assert_eq!(rule.backend, FirewallBackend::Nftables);
        service.set_audit_id(&rule.id, 7).unwrap();

        // Simulate a reboot clearing the kernel ruleset.
        provider.loaded.lock().clear();
        let reopened = FirewallService::new(provider.clone(), path.clone());
        let listed = reopened.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].active);
        assert_eq!(listed[0].audit_id, Some(7));

        let again = reopened.add(target, TrafficDirection::Inbound).unwrap();
        assert_eq!(again.id, rule.id, "re-applying reuses the stored rule");
        assert!(reopened.is_active(&rule.id));
        assert_eq!(reopened.len(), 1);

        reopened.remove(&rule.id).unwrap();
        assert!(reopened.list().unwrap().is_empty());
        assert!(reopened.remove(&rule.id).is_err());
    }

    #[test]
    fn declined_elevation_stores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let service = FirewallService::new(
            Arc::new(FakeFirewall {
                decline: true,
                ..Default::default()
            }),
            dir.path().join("rules.json"),
        );
        let err = service
            .add(
                FirewallTarget::RemoteIp {
                    ip: "203.0.113.5".into(),
                },
                TrafficDirection::Both,
            )
            .unwrap_err();
        assert!(matches!(err, SentinelError::ElevationDeclined { .. }));
        assert!(service.is_empty());
        assert!(
            service
                .add(
                    FirewallTarget::RemoteIp { ip: "::1".into() },
                    TrafficDirection::Both
                )
                .is_err()
        );
    }
}

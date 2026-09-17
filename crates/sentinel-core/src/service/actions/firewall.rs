//! Firewall block actions. Always `Critical`: they change OS network policy.

use std::net::IpAddr;

use super::{ActionService, Draft, Execution, metric};
use crate::action::{
    Action, ActionPreview, ActionRisk, ItemOutcome, MetricUnit, OutcomeStatus, PreviewTarget,
    Reversibility,
};
use crate::error::{CoreResult, SentinelError};
use crate::model::{
    AddrScope, FirewallBackend, FirewallRule, FirewallTarget, TrafficDirection, TransportProtocol,
};
use crate::service::firewall::render::validate_target;
use crate::service::network::scope::classify;

fn protocol_name(protocol: TransportProtocol) -> &'static str {
    match protocol {
        TransportProtocol::Tcp => "TCP",
        TransportProtocol::Udp => "UDP",
    }
}

pub(crate) fn describe(target: &FirewallTarget, direction: TrafficDirection) -> String {
    match (target, direction) {
        (FirewallTarget::RemoteIp { ip }, TrafficDirection::Inbound) => {
            format!("Block incoming connections from {ip}")
        }
        (FirewallTarget::RemoteIp { ip }, TrafficDirection::Outbound) => {
            format!("Block outgoing connections to {ip}")
        }
        (FirewallTarget::RemoteIp { ip }, TrafficDirection::Both) => {
            format!("Block all traffic to and from {ip}")
        }
        (FirewallTarget::LocalPort { port, protocol }, TrafficDirection::Inbound) => {
            format!(
                "Block incoming {} connections to port {port}",
                protocol_name(*protocol)
            )
        }
        (FirewallTarget::LocalPort { port, protocol }, TrafficDirection::Outbound) => {
            format!(
                "Block outgoing {} traffic from local port {port}",
                protocol_name(*protocol)
            )
        }
        (FirewallTarget::LocalPort { port, protocol }, TrafficDirection::Both) => {
            format!(
                "Block all {} traffic on local port {port}",
                protocol_name(*protocol)
            )
        }
    }
}

fn backend_description(backend: FirewallBackend) -> &'static str {
    match backend {
        FirewallBackend::Pf => "the macOS packet filter (pf), in Sentinel's own anchor",
        FirewallBackend::Nftables => "nftables, in Sentinel's own inet sentinel table",
        FirewallBackend::Iptables => "iptables, in Sentinel's own SENTINEL chains",
        FirewallBackend::WindowsFirewall => "Windows Defender Firewall, as a rule named Sentinel-…",
    }
}

fn backend_short(backend: FirewallBackend) -> &'static str {
    match backend {
        FirewallBackend::Pf => "pf",
        FirewallBackend::Nftables => "nftables",
        FirewallBackend::Iptables => "iptables",
        FirewallBackend::WindowsFirewall => "Windows Defender Firewall",
    }
}

pub(super) fn preview_add(
    service: &ActionService,
    target: FirewallTarget,
    direction: TrafficDirection,
) -> CoreResult<Draft> {
    validate_target(&target)?;
    let target = match target {
        FirewallTarget::RemoteIp { ip } => FirewallTarget::RemoteIp {
            ip: ip
                .trim()
                .parse::<IpAddr>()
                .map(|addr| addr.to_string())
                .map_err(|_| SentinelError::invalid(format!("{ip} is not an IP address")))?,
        },
        other => other,
    };
    let firewall = service.context.firewall()?;
    let status = firewall.status();
    let Some(backend) = status.backend.filter(|_| status.available) else {
        return Err(SentinelError::Unavailable {
            feature: "firewall".to_owned(),
            reason: status
                .note
                .unwrap_or_else(|| "no supported firewall was found".to_owned()),
        });
    };
    let mut warnings = Vec::new();
    if let Some(existing) = firewall.find(&target, direction) {
        if firewall.is_active(&existing.id) {
            return Err(SentinelError::invalid(format!(
                "Sentinel rule {} already does this",
                existing.id
            )));
        }
        warnings.push(format!(
            "Re-applies saved Sentinel rule {}, which is not currently loaded.",
            existing.id
        ));
    }
    let users = service.context.traffic_users(&target);
    match &target {
        FirewallTarget::RemoteIp { ip } => {
            if let Ok(addr) = ip.parse::<IpAddr>()
                && classify(&addr) != AddrScope::Public
            {
                warnings.push(format!(
                    "{ip} is a local-network address; blocking it can cut off devices such as your router or printer."
                ));
            }
            if !users.is_empty() {
                warnings.push(format!(
                    "Open connections from {} will be cut off.",
                    users.join(", ")
                ));
            }
        }
        FirewallTarget::LocalPort { port, .. } => {
            if !users.is_empty() {
                warnings.push(format!(
                    "Port {port} is in use by {}; its connections will stop working.",
                    users.join(", ")
                ));
            }
        }
    }
    if backend == FirewallBackend::Pf {
        warnings.push(
            "pf clears rules when the Mac restarts; Sentinel keeps the rule and shows it as inactive until you apply it again."
                .to_owned(),
        );
    }
    if status.firewall_enabled == Some(false) {
        warnings.push("The system firewall is currently turned off, so the rule will not take effect until it is turned on.".to_owned());
    }
    let title = describe(&target, direction);
    let elevation_note = if status.requires_elevation {
        " Your administrator password will be requested."
    } else {
        ""
    };
    Ok(Draft {
        title: title.clone(),
        description: format!(
            "{title}. Adds a rule to {}, kept separate from your other firewall rules.{elevation_note} Matching connections stop immediately.",
            backend_description(backend)
        ),
        targets: vec![PreviewTarget {
            label: match &target {
                FirewallTarget::RemoteIp { ip } => ip.clone(),
                FirewallTarget::LocalPort { port, protocol } => {
                    format!("{} port {port}", protocol_name(*protocol))
                }
            },
            detail: (!users.is_empty()).then(|| format!("In use by {}", users.join(", "))),
            size_bytes: None,
            problem: None,
            safety_note: None,
        }],
        impact: vec![metric(
            "sentinelRules",
            "Sentinel firewall rules",
            firewall.len() as f64,
            MetricUnit::Count,
        )],
        action: Action::AddFirewallRule { target, direction },
        estimated_bytes_freed: None,
        risk: ActionRisk::Critical,
        reversibility: Reversibility::Undoable {
            how: "Remove the rule from Sentinel's firewall rules".to_owned(),
        },
        warnings,
        requires_elevation: status.requires_elevation,
    })
}

pub(super) fn preview_remove(service: &ActionService, rule_id: String) -> CoreResult<Draft> {
    let firewall = service.context.firewall()?;
    let rule = firewall.get(&rule_id).ok_or_else(|| {
        SentinelError::invalid(format!("Sentinel has no firewall rule {rule_id}"))
    })?;
    let status = firewall.status();
    let active = firewall.is_active(&rule.id);
    let what = describe(&rule.target, rule.direction);
    Ok(Draft {
        title: format!("Remove firewall rule: {what}"),
        description: format!(
            "Removes Sentinel rule {} from {}; traffic it blocked is allowed again.{}",
            rule.id,
            backend_description(rule.backend),
            if active && status.requires_elevation {
                " Your administrator password will be requested."
            } else if !active {
                " The rule is not currently loaded, so only Sentinel's saved copy is removed."
            } else {
                ""
            }
        ),
        targets: vec![PreviewTarget {
            label: format!("Rule {}", rule.id),
            detail: Some(what.clone()),
            size_bytes: None,
            problem: None,
            safety_note: None,
        }],
        impact: vec![metric(
            "ruleActive",
            "Rule active",
            if active { 1.0 } else { 0.0 },
            MetricUnit::Count,
        )],
        action: Action::RemoveFirewallRule { rule_id },
        estimated_bytes_freed: None,
        risk: ActionRisk::Critical,
        reversibility: Reversibility::Undoable {
            how: format!("{what} again from the network view"),
        },
        warnings: Vec::new(),
        requires_elevation: active && status.requires_elevation,
    })
}

fn rule_metrics(
    service: &ActionService,
    rule: Option<&FirewallRule>,
) -> Vec<crate::action::Metric> {
    let Ok(firewall) = service.context.firewall() else {
        return Vec::new();
    };
    let mut metrics = vec![metric(
        "sentinelRules",
        "Sentinel firewall rules",
        firewall.len() as f64,
        MetricUnit::Count,
    )];
    if let Some(rule) = rule {
        metrics.push(metric(
            "ruleActive",
            "Rule active",
            if firewall.is_active(&rule.id) {
                1.0
            } else {
                0.0
            },
            MetricUnit::Count,
        ));
    }
    metrics
}

pub(super) fn execute_add(
    service: &ActionService,
    preview: &ActionPreview,
    target: &FirewallTarget,
    direction: TrafficDirection,
) -> Execution {
    let before = rule_metrics(service, None);
    let firewall = match service.context.firewall() {
        Ok(firewall) => firewall,
        Err(err) => return Execution::failed(preview.title.clone(), err, before),
    };
    let rule = match firewall.add(target.clone(), direction) {
        Ok(rule) => rule,
        Err(err) => return Execution::failed(preview.title.clone(), err, before),
    };
    let after = rule_metrics(service, Some(&rule));
    let active = after.iter().any(|m| m.key == "ruleActive" && m.value > 0.0);
    let what = describe(target, direction);
    Execution {
        status: OutcomeStatus::Succeeded,
        items: vec![ItemOutcome {
            label: what.clone(),
            path: None,
            success: true,
            error: None,
        }],
        before,
        after,
        summary: if active {
            format!(
                "{what}: Sentinel rule {} is active in {}.",
                rule.id,
                backend_short(rule.backend)
            )
        } else {
            format!(
                "{what}: Sentinel rule {} was applied but {} does not report it as loaded.",
                rule.id,
                backend_short(rule.backend)
            )
        },
        bytes_freed: None,
        affected_paths: Vec::new(),
    }
}

pub(super) fn execute_remove(
    service: &ActionService,
    preview: &ActionPreview,
    rule_id: &str,
) -> Execution {
    let before = rule_metrics(service, None);
    let firewall = match service.context.firewall() {
        Ok(firewall) => firewall,
        Err(err) => return Execution::failed(preview.title.clone(), err, before),
    };
    match firewall.remove(rule_id) {
        Ok(rule) => {
            let what = describe(&rule.target, rule.direction);
            Execution {
                status: OutcomeStatus::Succeeded,
                items: vec![ItemOutcome {
                    label: format!("Rule {rule_id}"),
                    path: None,
                    success: true,
                    error: None,
                }],
                before,
                after: rule_metrics(service, None),
                summary: format!(
                    "Removed Sentinel rule {rule_id}; \"{what}\" is no longer in effect."
                ),
                bytes_freed: None,
                affected_paths: Vec::new(),
            }
        }
        Err(err) => Execution::failed(preview.title.clone(), err, before),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn describes_blocks_plainly() {
        let ip = FirewallTarget::RemoteIp {
            ip: "203.0.113.9".into(),
        };
        assert_eq!(
            describe(&ip, TrafficDirection::Both),
            "Block all traffic to and from 203.0.113.9"
        );
        let port = FirewallTarget::LocalPort {
            port: 5432,
            protocol: TransportProtocol::Tcp,
        };
        assert_eq!(
            describe(&port, TrafficDirection::Inbound),
            "Block incoming TCP connections to port 5432"
        );
    }
}

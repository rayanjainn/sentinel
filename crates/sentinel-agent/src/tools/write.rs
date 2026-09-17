//! `WriteToolTranslator`: turns a write tool call into a normalized core `Action`.
//!
//! It resolves live identities (pid → start time, so a recycled PID can never be hit), normalizes
//! and validates paths and firewall targets, and refuses obviously unsafe targets. It has no way
//! to perform anything: its only output is an `Action` value for `ActionPreparer::prepare`.

use std::collections::HashSet;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

use sentinel_core::action::Action;
use sentinel_core::model::{
    FirewallTarget, Pid, ProcessIdentity, TrafficDirection, TransportProtocol,
};
use sentinel_core::service::SystemQueries;
use sentinel_core::{CoreResult, SentinelError};

use super::read::Args;
use super::{ProposedWrite, ToolAccess, WriteToolTranslator, access_of, names, paths};

const MAX_PATHS: usize = 500;

pub struct SystemWriteTranslator {
    queries: Arc<dyn SystemQueries>,
    own_pid: Pid,
}

impl SystemWriteTranslator {
    pub fn new(queries: Arc<dyn SystemQueries>) -> Self {
        Self {
            queries,
            own_pid: std::process::id(),
        }
    }

    fn identity(&self, args: &Args) -> CoreResult<ProcessIdentity> {
        let pid = args
            .int("pid")?
            .ok_or_else(|| SentinelError::invalid("`pid` is required"))?;
        let pid =
            Pid::try_from(pid).map_err(|_| SentinelError::invalid("`pid` is not a valid PID"))?;
        if pid <= 1 {
            return Err(SentinelError::invalid(format!(
                "PID {pid} is the kernel or init process and cannot be targeted"
            )));
        }
        if pid == self.own_pid {
            return Err(SentinelError::invalid(
                "that PID is Sentinel itself; it will not stop itself",
            ));
        }
        Ok(self.queries.process_detail(pid)?.info.identity())
    }

    fn protected_roots(&self) -> Vec<PathBuf> {
        let mut roots: Vec<PathBuf> = vec![PathBuf::from("/")];
        if let Some(home) = paths::home_dir() {
            roots.push(home);
        }
        if let Ok(volumes) = self.queries.volumes() {
            roots.extend(volumes.into_iter().map(|v| PathBuf::from(v.mount_point)));
        }
        roots
    }

    fn source_paths(&self, args: &Args) -> CoreResult<Vec<String>> {
        let raw = args
            .strings("paths")?
            .ok_or_else(|| SentinelError::invalid("`paths` is required"))?;
        if raw.is_empty() {
            return Err(SentinelError::invalid(
                "`paths` must list at least one path",
            ));
        }
        if raw.len() > MAX_PATHS {
            return Err(SentinelError::invalid(format!(
                "at most {MAX_PATHS} paths per action; split the proposal"
            )));
        }
        let protected = self.protected_roots();
        let mut seen = HashSet::new();
        let mut normalized: Vec<PathBuf> = Vec::new();
        for entry in &raw {
            let path = paths::normalize(entry)?;
            if path.parent().is_none() || protected.contains(&path) {
                return Err(SentinelError::invalid(format!(
                    "{} is a volume root or the home folder and cannot be moved",
                    paths::display(&path)
                )));
            }
            if std::fs::symlink_metadata(&path).is_err() {
                return Err(SentinelError::PathNotFound {
                    path: paths::display(&path),
                });
            }
            if seen.insert(path.clone()) {
                normalized.push(path);
            }
        }
        // A path inside another listed path moves with its parent.
        let kept: Vec<String> = normalized
            .iter()
            .filter(|p| {
                !normalized
                    .iter()
                    .any(|other| other != *p && paths::is_within(p, other))
            })
            .map(|p| paths::display(p))
            .collect();
        Ok(kept)
    }

    fn destination(&self, args: &Args, sources: &[String]) -> CoreResult<String> {
        let raw = args
            .string("destination_dir")?
            .ok_or_else(|| SentinelError::invalid("`destination_dir` is required"))?;
        let dest = paths::normalize(&raw)?;
        match std::fs::metadata(&dest) {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => {
                return Err(SentinelError::invalid(format!(
                    "{} is not a folder",
                    paths::display(&dest)
                )));
            }
            Err(_) => {
                return Err(SentinelError::PathNotFound {
                    path: paths::display(&dest),
                });
            }
        }
        for source in sources {
            let source = Path::new(source);
            if paths::is_within(&dest, source) {
                return Err(SentinelError::invalid(format!(
                    "cannot move {} into itself",
                    source.display()
                )));
            }
            if source.parent() == Some(dest.as_path()) {
                return Err(SentinelError::invalid(format!(
                    "{} is already in {}",
                    source.display(),
                    dest.display()
                )));
            }
        }
        Ok(paths::display(&dest))
    }
}

fn direction(args: &Args, default: TrafficDirection) -> CoreResult<TrafficDirection> {
    Ok(match args.string("direction")?.as_deref() {
        None => default,
        Some("inbound") => TrafficDirection::Inbound,
        Some("outbound") => TrafficDirection::Outbound,
        Some("both") => TrafficDirection::Both,
        Some(other) => {
            return Err(SentinelError::invalid(format!(
                "`direction` must be inbound, outbound or both, not `{other}`"
            )));
        }
    })
}

impl WriteToolTranslator for SystemWriteTranslator {
    fn translate(&self, name: &str, input: Value) -> CoreResult<ProposedWrite> {
        if access_of(name) != Some(ToolAccess::Write) {
            return Err(SentinelError::invalid(format!(
                "`{name}` is not a write tool"
            )));
        }
        let args = Args::parse(name, input)?;
        let rationale = args
            .string("reason")?
            .map(|r| r.trim().to_owned())
            .filter(|r| !r.is_empty())
            .ok_or_else(|| {
                SentinelError::invalid("`reason` must explain why this change is proposed")
            })?;
        let action = match name {
            names::TERMINATE_PROCESS => Action::TerminateProcess {
                target: self.identity(&args)?,
            },
            names::FORCE_KILL_PROCESS => Action::ForceKillProcess {
                target: self.identity(&args)?,
            },
            names::SET_PROCESS_PRIORITY => {
                let nice = args
                    .int("nice")?
                    .ok_or_else(|| SentinelError::invalid("`nice` is required"))?;
                if !(-20..=19).contains(&nice) {
                    return Err(SentinelError::invalid("`nice` must be between -20 and 19"));
                }
                Action::SetProcessPriority {
                    target: self.identity(&args)?,
                    nice: nice as i32,
                }
            }
            names::MOVE_TO_TRASH => Action::TrashPaths {
                paths: self.source_paths(&args)?,
            },
            names::MOVE_PATHS => {
                let paths = self.source_paths(&args)?;
                let destination_dir = self.destination(&args, &paths)?;
                Action::MovePaths {
                    paths,
                    destination_dir,
                }
            }
            names::BLOCK_REMOTE_IP => {
                let raw = args
                    .string("ip")?
                    .ok_or_else(|| SentinelError::invalid("`ip` is required"))?;
                let ip: IpAddr = raw
                    .trim()
                    .parse()
                    .map_err(|_| SentinelError::invalid(format!("`{raw}` is not an IP address")))?;
                if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
                    return Err(SentinelError::invalid(format!(
                        "{ip} is a loopback, unspecified or multicast address; blocking it would break local networking"
                    )));
                }
                Action::AddFirewallRule {
                    target: FirewallTarget::RemoteIp { ip: ip.to_string() },
                    direction: direction(&args, TrafficDirection::Both)?,
                }
            }
            names::BLOCK_LOCAL_PORT => {
                let port = args
                    .int("port")?
                    .and_then(|p| u16::try_from(p).ok())
                    .filter(|p| *p > 0)
                    .ok_or_else(|| SentinelError::invalid("`port` must be between 1 and 65535"))?;
                let protocol = match args.string("protocol")?.as_deref() {
                    Some("tcp") => TransportProtocol::Tcp,
                    Some("udp") => TransportProtocol::Udp,
                    _ => return Err(SentinelError::invalid("`protocol` must be tcp or udp")),
                };
                Action::AddFirewallRule {
                    target: FirewallTarget::LocalPort { port, protocol },
                    direction: direction(&args, TrafficDirection::Inbound)?,
                }
            }
            names::REMOVE_FIREWALL_RULE => {
                let rule_id = args
                    .string("rule_id")?
                    .map(|r| r.trim().to_owned())
                    .filter(|r| !r.is_empty())
                    .ok_or_else(|| SentinelError::invalid("`rule_id` is required"))?;
                if !self
                    .queries
                    .firewall_rules()?
                    .iter()
                    .any(|r| r.id == rule_id)
                {
                    return Err(SentinelError::invalid(format!(
                        "no Sentinel firewall rule has id `{rule_id}`; call list_firewall_rules"
                    )));
                }
                Action::RemoveFirewallRule { rule_id }
            }
            other => {
                return Err(SentinelError::invalid(format!(
                    "unknown write tool `{other}`"
                )));
            }
        };
        Ok(ProposedWrite { action, rationale })
    }
}

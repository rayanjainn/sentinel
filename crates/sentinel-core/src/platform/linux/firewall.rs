//! nftables table `inet sentinel` (iptables `SENTINEL_*` chains as a fallback), applied as root
//! through polkit (`pkexec`).

use std::process::{Command, Stdio};

use crate::error::{CoreResult, SentinelError};
use crate::model::{FirewallBackend, FirewallRule, FirewallStatus};
use crate::platform::PlatformConfig;
use crate::platform::command::which;
use crate::platform::firewall_state::{LoadedState, with_rule, without_rule};
use crate::platform::unix::is_root;
use crate::provider::FirewallProvider;
use crate::service::firewall::render::{
    IPTABLES_IN, IPTABLES_OUT, NFT_TABLE, iptables_rule_ids, iptables_script, nft_rule_ids,
    nft_ruleset, shell_quote,
};

const MARKER: &str = "--- loaded ---";

pub(crate) struct LinuxFirewall {
    state: LoadedState,
}

impl LinuxFirewall {
    pub fn new(config: &PlatformConfig) -> Self {
        Self {
            state: LoadedState::new(config.data_dir.join("firewall").join("linux-loaded.json")),
        }
    }

    fn backend(&self) -> CoreResult<FirewallBackend> {
        super::permissions::firewall_backend().ok_or_else(|| SentinelError::Unavailable {
            feature: "firewall".to_owned(),
            reason: "neither nft nor iptables is installed".to_owned(),
        })
    }

    fn listing_script(backend: FirewallBackend) -> String {
        match backend {
            FirewallBackend::Iptables => format!(
                "iptables -S {IPTABLES_IN}; iptables -S {IPTABLES_OUT}; ip6tables -S {IPTABLES_IN}; ip6tables -S {IPTABLES_OUT}"
            ),
            _ => format!("nft list table inet {NFT_TABLE}"),
        }
    }

    fn run_privileged(&self, script: &str) -> CoreResult<std::process::Output> {
        let mut command = if is_root() {
            let mut c = Command::new("/bin/sh");
            c.args(["-c", script]);
            c
        } else {
            let pkexec = which("pkexec").ok_or_else(|| SentinelError::Unavailable {
                feature: "firewall".to_owned(),
                reason: "pkexec (polkit) is required to request administrator access".to_owned(),
            })?;
            let mut c = Command::new(pkexec);
            c.args(["/bin/sh", "-c", script]);
            c
        };
        let output =
            command
                .stdin(Stdio::null())
                .output()
                .map_err(|err| SentinelError::Unavailable {
                    feature: "firewall".to_owned(),
                    reason: format!("could not run the administrator helper: {err}"),
                })?;
        // pkexec: 126 = the authentication dialog was dismissed, 127 = not authorized.
        match output.status.code() {
            Some(126) if !is_root() => Err(SentinelError::ElevationDeclined {
                operation: "change firewall rules".to_owned(),
            }),
            Some(127) if !is_root() => Err(SentinelError::PermissionDenied {
                operation: "change firewall rules".to_owned(),
                target: None,
                hint: Some("Your account is not allowed to administer this system.".to_owned()),
            }),
            _ => Ok(output),
        }
    }

    fn apply(&self, rules: &[FirewallRule]) -> CoreResult<()> {
        let backend = self.backend()?;
        let apply = match backend {
            FirewallBackend::Iptables => iptables_script(rules)?,
            _ => format!(
                "printf '%s' {} | nft -f -",
                shell_quote(&nft_ruleset(rules)?)
            ),
        };
        let script = format!(
            "{apply} && echo '{MARKER}' && {{ {} ; }} 2>/dev/null; true",
            Self::listing_script(backend)
        );
        let output = self.run_privileged(&script)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let Some(listing) = stdout.split(MARKER).nth(1) else {
            return Err(SentinelError::Io {
                detail: format!("the firewall rejected the rules: {}", stderr.trim()),
                path: None,
            });
        };
        let active = match backend {
            FirewallBackend::Iptables => iptables_rule_ids(listing),
            _ => nft_rule_ids(listing),
        };
        if let Some(missing) = rules.iter().find(|r| !active.contains(&r.id)) {
            return Err(SentinelError::Io {
                detail: format!("rule {} was not loaded: {}", missing.id, stderr.trim()),
                path: None,
            });
        }
        self.state.save(&boot_id(), rules)
    }
}

fn boot_id() -> String {
    std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
        .map(|id| id.trim().to_owned())
        .unwrap_or_else(|_| sysinfo::System::boot_time().to_string())
}

impl FirewallProvider for LinuxFirewall {
    fn status(&self) -> FirewallStatus {
        super::permissions::firewall_status()
    }

    fn install(&self, rule: &FirewallRule) -> CoreResult<()> {
        let rules = with_rule(self.state.rules(&boot_id()), rule);
        self.apply(&rules)
    }

    fn remove(&self, rule: &FirewallRule) -> CoreResult<()> {
        let current = self.state.rules(&boot_id());
        if !current.iter().any(|r| r.id == rule.id) {
            return Ok(());
        }
        self.apply(&without_rule(current, &rule.id))
    }

    fn installed_rule_ids(&self) -> CoreResult<Vec<String>> {
        if is_root() {
            let backend = self.backend()?;
            let output = Command::new("/bin/sh")
                .args(["-c", &Self::listing_script(backend)])
                .stdin(Stdio::null())
                .output()
                .map_err(|err| SentinelError::io(&err, None))?;
            let text = String::from_utf8_lossy(&output.stdout);
            return Ok(match backend {
                FirewallBackend::Iptables => iptables_rule_ids(&text),
                _ => nft_rule_ids(&text),
            });
        }
        Ok(self
            .state
            .rules(&boot_id())
            .into_iter()
            .map(|r| r.id)
            .collect())
    }
}

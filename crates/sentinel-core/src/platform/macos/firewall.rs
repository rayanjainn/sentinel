//! pf rules in the `com.apple/250.sentinel` anchor, applied through one administrator prompt per
//! change (`osascript … with administrator privileges`).

use std::process::{Command, Stdio};

use crate::error::{CoreResult, SentinelError};
use crate::model::{FirewallRule, FirewallStatus};
use crate::platform::PlatformConfig;
use crate::platform::firewall_state::{LoadedState, with_rule, without_rule};
use crate::platform::unix::is_root;
use crate::provider::FirewallProvider;
use crate::service::firewall::render::{
    PF_ANCHOR, applescript_quote, pf_rule_ids, pf_ruleset, shell_quote,
};

pub(crate) struct PfFirewall {
    state: LoadedState,
}

impl PfFirewall {
    pub fn new(config: &PlatformConfig) -> Self {
        Self {
            state: LoadedState::new(config.data_dir.join("firewall").join("pf-loaded.json")),
        }
    }

    fn apply(&self, rules: &[FirewallRule]) -> CoreResult<()> {
        let ruleset = pf_ruleset(rules)?;
        let script = format!(
            "printf '%s' {rules} | /sbin/pfctl -a {PF_ANCHOR} -f - 2>&1 && \
             {{ /sbin/pfctl -s info 2>/dev/null | grep -q 'Status: Enabled' || /sbin/pfctl -E >/dev/null 2>&1; }} && \
             echo '--- loaded ---' && /sbin/pfctl -a {PF_ANCHOR} -s rules 2>/dev/null",
            rules = shell_quote(&ruleset)
        );
        let output = if is_root() {
            Command::new("/bin/sh")
                .args(["-c", &script])
                .stdin(Stdio::null())
                .output()
        } else {
            let apple_script = format!(
                "do shell script {} with administrator privileges",
                applescript_quote(&script)
            );
            Command::new("/usr/bin/osascript")
                .args(["-e", &apple_script])
                .stdin(Stdio::null())
                .output()
        }
        .map_err(|err| SentinelError::Unavailable {
            feature: "firewall".to_owned(),
            reason: format!("could not run the administrator helper: {err}"),
        })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            if stderr.contains("User canceled") || stderr.contains("(-128)") {
                return Err(SentinelError::ElevationDeclined {
                    operation: "change packet filter rules".to_owned(),
                });
            }
            return Err(SentinelError::Io {
                detail: format!(
                    "pfctl rejected the rules: {}{}",
                    stdout.trim(),
                    stderr.trim()
                ),
                path: None,
            });
        }
        let loaded = stdout.split("--- loaded ---").nth(1).unwrap_or_default();
        let active = pf_rule_ids(loaded);
        if let Some(missing) = rules.iter().find(|r| !active.contains(&r.id)) {
            return Err(SentinelError::Io {
                detail: format!("pf did not load rule {} ({})", missing.id, stdout.trim()),
                path: None,
            });
        }
        self.state.save(&boot_id(), rules)
    }
}

/// `kern.boottime` seconds: stable for the life of a boot.
fn boot_id() -> String {
    sysinfo::System::boot_time().to_string()
}

impl FirewallProvider for PfFirewall {
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
            let output = Command::new("/sbin/pfctl")
                .args(["-a", PF_ANCHOR, "-s", "rules"])
                .stdin(Stdio::null())
                .output()
                .map_err(|err| SentinelError::io(&err, None))?;
            return Ok(pf_rule_ids(&String::from_utf8_lossy(&output.stdout)));
        }
        Ok(self
            .state
            .rules(&boot_id())
            .into_iter()
            .map(|r| r.id)
            .collect())
    }
}

//! Windows Defender Firewall rules named `Sentinel-<id>`, managed with `netsh advfirewall` run
//! elevated through a UAC prompt. Rules persist across reboots, so the live list is authoritative.

use std::os::windows::process::CommandExt;
use std::process::{Command, Output, Stdio};

use crate::error::{CoreResult, SentinelError};
use crate::model::{FirewallRule, FirewallStatus};
use crate::provider::FirewallProvider;
use crate::service::firewall::render::{
    netsh_add_commands, netsh_delete_command, netsh_rule_ids, powershell_quote,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// ERROR_CANCELLED: the user declined the UAC prompt.
const DECLINED_EXIT: i32 = 1223;

pub(crate) struct WindowsFirewall;

fn netsh(args: &[String]) -> CoreResult<Output> {
    Command::new("netsh")
        .args(args)
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|err| SentinelError::Unavailable {
            feature: "firewall".to_owned(),
            reason: format!("could not run netsh: {err}"),
        })
}

fn check(output: Output) -> CoreResult<()> {
    if output.status.success() {
        return Ok(());
    }
    Err(SentinelError::Io {
        detail: format!(
            "netsh failed: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        path: None,
    })
}

/// A netsh invocation as one cmd.exe command; arguments contain spaces only inside `key=value`.
fn netsh_line(args: &[String]) -> String {
    let mut line = String::from("netsh");
    for arg in args {
        line.push(' ');
        if arg.contains(' ') {
            line.push('"');
            line.push_str(arg);
            line.push('"');
        } else {
            line.push_str(arg);
        }
    }
    line
}

/// Runs a cmd.exe command line elevated through UAC and returns once it exits.
fn run_elevated(line: &str, operation: &str) -> CoreResult<()> {
    let script = format!(
        "try {{ $p = Start-Process -FilePath 'cmd.exe' -ArgumentList {} -Verb RunAs -WindowStyle Hidden -Wait -PassThru -ErrorAction Stop; exit $p.ExitCode }} catch {{ exit {DECLINED_EXIT} }}",
        powershell_quote(&format!("/s /c \"{line}\""))
    );
    let output = Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|err| SentinelError::Unavailable {
            feature: "firewall".to_owned(),
            reason: format!("could not start PowerShell to request elevation: {err}"),
        })?;
    match output.status.code() {
        Some(0) => Ok(()),
        Some(DECLINED_EXIT) => Err(SentinelError::ElevationDeclined {
            operation: operation.to_owned(),
        }),
        code => Err(SentinelError::Io {
            detail: format!(
                "netsh exited with {} while trying to {operation}",
                code.map(|c| c.to_string())
                    .unwrap_or_else(|| "an unknown status".into())
            ),
            path: None,
        }),
    }
}

impl FirewallProvider for WindowsFirewall {
    fn status(&self) -> FirewallStatus {
        super::permissions::firewall_status()
    }

    fn install(&self, rule: &FirewallRule) -> CoreResult<()> {
        let delete = netsh_delete_command(&rule.id)?;
        let adds = netsh_add_commands(rule)?;
        if super::permissions::is_elevated() {
            // Re-applying replaces any rule with the same name; a missing rule is fine.
            let _ = netsh(&delete)?;
            for add in &adds {
                check(netsh(add)?)?;
            }
            return Ok(());
        }
        let mut line = format!("({} >nul 2>&1 || ver >nul)", netsh_line(&delete));
        for add in &adds {
            line.push_str(" && ");
            line.push_str(&netsh_line(add));
        }
        run_elevated(&line, "add a firewall rule")
    }

    fn remove(&self, rule: &FirewallRule) -> CoreResult<()> {
        if !self.installed_rule_ids()?.contains(&rule.id) {
            return Ok(());
        }
        let delete = netsh_delete_command(&rule.id)?;
        if super::permissions::is_elevated() {
            return check(netsh(&delete)?);
        }
        run_elevated(&netsh_line(&delete), "remove a firewall rule")
    }

    fn installed_rule_ids(&self) -> CoreResult<Vec<String>> {
        let output = netsh(&[
            "advfirewall".into(),
            "firewall".into(),
            "show".into(),
            "rule".into(),
            "name=all".into(),
        ])?;
        Ok(netsh_rule_ids(&String::from_utf8_lossy(&output.stdout)))
    }
}

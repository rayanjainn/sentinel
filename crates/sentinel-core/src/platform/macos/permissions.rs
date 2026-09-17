use std::io::ErrorKind;
use std::path::Path;

use crate::error::CoreResult;
use crate::model::{
    FirewallBackend, FirewallStatus, PermissionKind, PermissionState, PermissionStatus, Platform,
};
use crate::platform::command::run_status;
use crate::platform::unix::is_root;
use crate::provider::PermissionProbe;
use crate::util::home_dir;

pub(crate) struct MacPermissions;

impl PermissionProbe for MacPermissions {
    fn status(&self) -> PermissionStatus {
        PermissionStatus {
            platform: Platform::Macos,
            full_disk_access: full_disk_access(),
            running_elevated: is_root(),
            can_request_elevation: Path::new("/usr/bin/osascript").exists(),
            firewall: firewall_status(),
        }
    }

    fn open_settings(&self, kind: PermissionKind) -> CoreResult<()> {
        let url = match kind {
            PermissionKind::FullDiskAccess => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles"
            }
            PermissionKind::Administrator => {
                "x-apple.systempreferences:com.apple.Users-Groups-Settings.extension"
            }
        };
        run_status("/usr/bin/open", &[url], "open System Settings")
    }
}

/// TCC gives no query API for Full Disk Access; reading a protected location is the documented
/// way to detect it. EPERM means the grant is missing.
fn full_disk_access() -> PermissionState {
    let mut candidates = Vec::new();
    if let Some(home) = home_dir() {
        candidates.push(home.join("Library/Safari"));
        candidates.push(home.join("Library/Mail"));
        candidates.push(home.join("Library/Messages"));
    }
    candidates.push("/Library/Application Support/com.apple.TCC".into());
    for path in candidates {
        match std::fs::read_dir(&path) {
            Ok(_) => return PermissionState::Granted,
            Err(err) if err.kind() == ErrorKind::PermissionDenied => {
                return PermissionState::Denied;
            }
            Err(_) => continue,
        }
    }
    PermissionState::Unknown
}

pub(crate) fn firewall_status() -> FirewallStatus {
    let available = Path::new("/sbin/pfctl").exists();
    let root = is_root();
    let firewall_enabled = if available && root {
        std::process::Command::new("/sbin/pfctl")
            .args(["-s", "info"])
            .stdin(std::process::Stdio::null())
            .output()
            .ok()
            .and_then(|out| parse_pf_enabled(&String::from_utf8_lossy(&out.stdout)))
    } else {
        None
    };
    FirewallStatus {
        backend: available.then_some(FirewallBackend::Pf),
        available,
        firewall_enabled,
        requires_elevation: !root,
        note: if !available {
            Some("pfctl was not found on this system.".to_owned())
        } else if root {
            None
        } else {
            Some(
                "Packet filter state is only readable with administrator privileges; adding or \
                 removing a rule asks for your password once."
                    .to_owned(),
            )
        },
    }
}

fn parse_pf_enabled(info: &str) -> Option<bool> {
    info.lines().find_map(|line| {
        let rest = line.trim().strip_prefix("Status:")?;
        let word = rest.split_whitespace().next()?;
        Some(word.eq_ignore_ascii_case("enabled"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pfctl_info() {
        let text = "No ALTQ support in kernel\nStatus: Enabled for 0 days 00:12:01           Debug: Urgent\n";
        assert_eq!(parse_pf_enabled(text), Some(true));
        assert_eq!(parse_pf_enabled("Status: Disabled"), Some(false));
        assert_eq!(parse_pf_enabled("garbage"), None);
    }
}

use crate::error::{CoreResult, SentinelError};
use crate::model::{
    FirewallBackend, FirewallStatus, PermissionKind, PermissionState, PermissionStatus, Platform,
};
use crate::platform::command::which;
use crate::platform::unix::is_root;
use crate::provider::PermissionProbe;

pub(crate) struct LinuxPermissions;

impl PermissionProbe for LinuxPermissions {
    fn status(&self) -> PermissionStatus {
        let root = is_root();
        PermissionStatus {
            platform: Platform::Linux,
            full_disk_access: PermissionState::NotApplicable,
            running_elevated: root,
            can_request_elevation: root || which("pkexec").is_some(),
            firewall: firewall_status(),
        }
    }

    fn open_settings(&self, kind: PermissionKind) -> CoreResult<()> {
        Err(SentinelError::Unavailable {
            feature: "open permission settings".to_owned(),
            reason: match kind {
                PermissionKind::FullDiskAccess => {
                    "Linux has no Full Disk Access setting; file access follows normal user permissions."
                }
                PermissionKind::Administrator => {
                    "Administrator access is requested through polkit (pkexec) when an action needs it."
                }
            }
            .to_owned(),
        })
    }
}

pub(crate) fn firewall_backend() -> Option<FirewallBackend> {
    if which("nft").is_some() {
        Some(FirewallBackend::Nftables)
    } else if which("iptables").is_some() {
        Some(FirewallBackend::Iptables)
    } else {
        None
    }
}

pub(crate) fn firewall_status() -> FirewallStatus {
    let backend = firewall_backend();
    let root = is_root();
    let can_elevate = root || which("pkexec").is_some();
    let available = backend.is_some() && can_elevate;
    FirewallStatus {
        backend,
        available,
        firewall_enabled: None,
        requires_elevation: !root,
        note: match (backend, can_elevate) {
            (None, _) => Some("Neither nft nor iptables was found on this system.".to_owned()),
            (Some(_), false) => Some(
                "pkexec (polkit) is not installed, so Sentinel cannot request administrator access \
                 to change firewall rules."
                    .to_owned(),
            ),
            (Some(_), true) if !root => Some(
                "Sentinel keeps its rules in a dedicated table; changes ask for administrator \
                 authorization through polkit."
                    .to_owned(),
            ),
            _ => None,
        },
    }
}

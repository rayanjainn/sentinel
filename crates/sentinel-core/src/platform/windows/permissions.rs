use windows::Win32::Foundation::HANDLE;
use windows::Win32::Security::{GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::handle::OwnedHandle;
use crate::error::{CoreResult, SentinelError};
use crate::model::{
    FirewallBackend, FirewallStatus, PermissionKind, PermissionState, PermissionStatus, Platform,
};
use crate::parse::netsh;
use crate::platform::command::{run_output, spawn_detached};
use crate::provider::PermissionProbe;

pub(crate) struct WindowsPermissions;

impl PermissionProbe for WindowsPermissions {
    fn status(&self) -> PermissionStatus {
        PermissionStatus {
            platform: Platform::Windows,
            full_disk_access: PermissionState::NotApplicable,
            running_elevated: is_elevated(),
            can_request_elevation: true,
            firewall: firewall_status(),
        }
    }

    fn open_settings(&self, kind: PermissionKind) -> CoreResult<()> {
        match kind {
            PermissionKind::FullDiskAccess => Err(SentinelError::Unavailable {
                feature: "open permission settings".to_owned(),
                reason: "Windows has no Full Disk Access setting; file access follows NTFS \
                         permissions."
                    .to_owned(),
            }),
            PermissionKind::Administrator => spawn_detached(
                "UserAccountControlSettings.exe",
                &[],
                "open User Account Control settings",
            ),
        }
    }
}

pub(crate) fn is_elevated() -> bool {
    let mut token = HANDLE::default();
    // SAFETY: querying our own process token; closed by OwnedHandle.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) }.is_err() {
        return false;
    }
    let token = OwnedHandle(token);
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0u32;
    // SAFETY: buffer is a TOKEN_ELEVATION of the stated size.
    let ok = unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            Some((&mut elevation as *mut TOKEN_ELEVATION).cast()),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    }
    .is_ok();
    ok && elevation.TokenIsElevated != 0
}

pub(crate) fn firewall_status() -> FirewallStatus {
    let elevated = is_elevated();
    let enabled = run_output(
        "netsh",
        &["advfirewall", "show", "currentprofile", "state"],
        "read firewall state",
    )
    .ok()
    .filter(|out| out.status.success())
    .and_then(|out| netsh::profile_state(&String::from_utf8_lossy(&out.stdout)));
    FirewallStatus {
        backend: Some(FirewallBackend::WindowsFirewall),
        available: true,
        firewall_enabled: enabled,
        requires_elevation: !elevated,
        note: match enabled {
            Some(false) => Some(
                "Windows Defender Firewall is off for the active profile; Sentinel rules will not \
                 take effect until it is turned on."
                    .to_owned(),
            ),
            _ if !elevated => {
                Some("Adding or removing a rule shows a User Account Control prompt.".to_owned())
            }
            _ => None,
        },
    }
}

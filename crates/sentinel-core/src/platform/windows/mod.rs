mod handle;
mod handles;
mod network;
mod permissions;
mod process;
mod resources;
mod storage;

use std::sync::Arc;

use crate::platform::sys_resources::SysResources;
use crate::platform::{PlatformConfig, Providers, proc_table::ProcTable};
use crate::provider::ProcessProvider;

pub(crate) fn providers(_config: &PlatformConfig) -> Providers {
    Providers {
        resources: Box::new(SysResources::new(resources::WindowsResources::new())),
        processes: process_provider(),
        network: Box::new(network::WindowsNetwork::new()),
        process_control: Arc::new(process::WindowsProcessControl),
        permissions: Arc::new(permissions::WindowsPermissions),
        file_ops: Arc::new(crate::platform::fileops::PlatformFileOps),
        storage: Arc::new(storage::WindowsStorage::default()),
    }
}

pub(crate) fn process_provider() -> Box<dyn ProcessProvider> {
    Box::new(ProcTable::new(process::WindowsProcessExtras::default()))
}

pub(crate) fn is_elevated() -> bool {
    permissions::is_elevated()
}

pub(crate) fn system_time_zone() -> Option<String> {
    use windows::Win32::System::Time::{
        DYNAMIC_TIME_ZONE_INFORMATION, GetDynamicTimeZoneInformation,
    };

    use crate::parse::zonetab::{WINDOWS_ZONES, windows_to_iana};

    let mut info = DYNAMIC_TIME_ZONE_INFORMATION::default();
    // SAFETY: out pointer to a caller-owned structure.
    let status = unsafe { GetDynamicTimeZoneInformation(&mut info) };
    if status == u32::MAX {
        return None;
    }
    let len = info
        .TimeZoneKeyName
        .iter()
        .position(|c| *c == 0)
        .unwrap_or(info.TimeZoneKeyName.len());
    let key = String::from_utf16_lossy(&info.TimeZoneKeyName[..len]);
    windows_to_iana(WINDOWS_ZONES, &key)
}

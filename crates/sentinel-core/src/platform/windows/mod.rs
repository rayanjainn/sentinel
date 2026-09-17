mod handle;
mod handles;
mod permissions;
mod process;
mod resources;

use std::sync::Arc;

use crate::platform::sys_resources::SysResources;
use crate::platform::{PlatformConfig, Providers, proc_table::ProcTable};
use crate::provider::ProcessProvider;

pub(crate) fn providers(_config: &PlatformConfig) -> Providers {
    Providers {
        resources: Box::new(SysResources::new(resources::WindowsResources::new())),
        processes: process_provider(),
        process_control: Arc::new(process::WindowsProcessControl),
        permissions: Arc::new(permissions::WindowsPermissions),
    }
}

pub(crate) fn process_provider() -> Box<dyn ProcessProvider> {
    Box::new(ProcTable::new(process::WindowsProcessExtras::default()))
}

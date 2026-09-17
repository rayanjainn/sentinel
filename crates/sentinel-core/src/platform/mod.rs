//! OS implementations of the provider traits, selected with `cfg(target_os)`.
//!
//! Shared building blocks (`proc_table`, `sys_resources`, `identity`) sit on top of `sysinfo`;
//! each OS module supplies the pieces `sysinfo` does not expose.

use std::path::PathBuf;
use std::sync::Arc;

use crate::provider::*;

mod command;
mod fileops;
mod identity;
mod proc_names;
mod proc_table;
mod sys_resources;

#[cfg(unix)]
mod unix;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as os;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as os;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as os;

pub use identity::current_identity;
pub use proc_names::ProcessNames;

/// Where platform components may keep small state files (firewall bookkeeping, etc.).
#[derive(Debug, Clone)]
pub struct PlatformConfig {
    pub data_dir: PathBuf,
}

/// Every provider for the running OS. Read providers are boxed for exclusive use by one owner
/// (the sampler); control providers are shared.
pub struct Providers {
    pub resources: Box<dyn ResourceProvider>,
    pub processes: Box<dyn ProcessProvider>,
    pub network: Box<dyn NetworkProvider>,
    pub process_control: Arc<dyn ProcessControl>,
    pub permissions: Arc<dyn PermissionProbe>,
    pub file_ops: Arc<dyn FileOps>,
}

pub fn current(config: &PlatformConfig) -> Providers {
    os::providers(config)
}

/// Fresh, independent read providers (e.g. for a second consumer that must not contend with the
/// sampler).
pub fn new_process_provider() -> Box<dyn ProcessProvider> {
    os::process_provider()
}

/// IANA name of the system time zone (e.g. "Europe/Paris").
pub fn system_time_zone() -> Option<String> {
    os::system_time_zone()
}

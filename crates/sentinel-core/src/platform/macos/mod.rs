mod ffi;
mod memory;
mod permissions;
mod process;

use std::sync::Arc;

use crate::error::CoreResult;
use crate::model::{FanReading, LoadAverage, MemoryBreakdown, Platform};
use crate::platform::sys_resources::{ResourceExtras, SysResources};
use crate::platform::{PlatformConfig, Providers, proc_table::ProcTable};
use crate::provider::ProcessProvider;

pub(crate) fn providers(_config: &PlatformConfig) -> Providers {
    Providers {
        resources: Box::new(SysResources::new(MacResources::new())),
        processes: process_provider(),
        process_control: Arc::new(process::MacProcessControl),
        permissions: Arc::new(permissions::MacPermissions),
    }
}

pub(crate) fn process_provider() -> Box<dyn ProcessProvider> {
    Box::new(ProcTable::new(process::MacProcessExtras::default()))
}

struct MacResources {
    interfaces: InterfaceCounters,
}

impl MacResources {
    fn new() -> Self {
        Self {
            interfaces: InterfaceCounters::new(),
        }
    }
}

impl ResourceExtras for MacResources {
    fn platform(&self) -> Platform {
        Platform::Macos
    }

    fn memory(&mut self, system: &sysinfo::System) -> MemoryBreakdown {
        memory::breakdown(system)
    }

    fn load(&mut self, _cpu_total: f32, _logical_cores: u32) -> Option<LoadAverage> {
        crate::platform::unix::load_average()
    }

    fn fans(&mut self) -> Vec<FanReading> {
        // Fan speeds are only reachable through undocumented SMC keys on macOS.
        Vec::new()
    }

    fn interface_counters(&mut self) -> CoreResult<(u64, u64)> {
        Ok(self.interfaces.totals())
    }
}

/// Cumulative bytes over physical interfaces (64-bit `if_data64` counters via `sysinfo`).
pub(crate) struct InterfaceCounters {
    networks: sysinfo::Networks,
}

impl InterfaceCounters {
    pub fn new() -> Self {
        Self {
            networks: sysinfo::Networks::new_with_refreshed_list(),
        }
    }

    pub fn totals(&mut self) -> (u64, u64) {
        self.networks.refresh(true);
        self.networks
            .list()
            .iter()
            .filter(|(name, _)| is_physical_interface(name))
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (
                    rx.saturating_add(data.total_received()),
                    tx.saturating_add(data.total_transmitted()),
                )
            })
    }
}

/// Hardware-backed interface families. Tunnels (utun, ipsec, gif, stf), bridges and loopback carry
/// traffic that is also counted on the underlying physical interface.
fn is_physical_interface(name: &str) -> bool {
    const PHYSICAL: [&str; 5] = ["en", "awdl", "llw", "pdp_ip", "ap"];
    PHYSICAL.iter().any(|prefix| {
        name.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_interfaces() {
        for name in ["en0", "en10", "awdl0", "llw0", "pdp_ip0", "ap1"] {
            assert!(is_physical_interface(name), "{name}");
        }
        for name in [
            "lo0", "utun4", "bridge0", "gif0", "stf0", "anpi0", "en", "enx",
        ] {
            assert!(!is_physical_interface(name), "{name}");
        }
    }
}

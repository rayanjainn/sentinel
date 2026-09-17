mod permissions;
mod process;

use std::path::Path;
use std::sync::Arc;

use crate::error::CoreResult;
use crate::model::{FanReading, LoadAverage, MemoryBreakdown, Platform};
use crate::parse::procfs;
use crate::platform::sys_resources::{ResourceExtras, SysResources, basic_memory};
use crate::platform::{PlatformConfig, Providers, proc_table::ProcTable};
use crate::provider::ProcessProvider;

pub(crate) fn providers(_config: &PlatformConfig) -> Providers {
    Providers {
        resources: Box::new(SysResources::new(LinuxResources::new())),
        processes: process_provider(),
        process_control: Arc::new(process::LinuxProcessControl),
        permissions: Arc::new(permissions::LinuxPermissions),
        file_ops: Arc::new(crate::platform::fileops::PlatformFileOps),
    }
}

pub(crate) fn process_provider() -> Box<dyn ProcessProvider> {
    Box::new(ProcTable::new(process::LinuxProcessExtras))
}

struct LinuxResources {
    interfaces: InterfaceCounters,
}

impl LinuxResources {
    fn new() -> Self {
        Self {
            interfaces: InterfaceCounters::new(),
        }
    }
}

impl ResourceExtras for LinuxResources {
    fn platform(&self) -> Platform {
        Platform::Linux
    }

    fn memory(&mut self, system: &sysinfo::System) -> MemoryBreakdown {
        match std::fs::read_to_string("/proc/meminfo") {
            Ok(text) => {
                memory_from_meminfo(&procfs::meminfo(&text)).unwrap_or_else(|| basic_memory(system))
            }
            Err(_) => basic_memory(system),
        }
    }

    fn load(&mut self, _cpu_total: f32, _logical_cores: u32) -> Option<LoadAverage> {
        crate::platform::unix::load_average()
    }

    fn fans(&mut self) -> Vec<FanReading> {
        hwmon_fans(Path::new("/sys/class/hwmon"))
    }

    fn interface_counters(&mut self) -> CoreResult<(u64, u64)> {
        Ok(self.interfaces.totals())
    }
}

pub(crate) fn memory_from_meminfo(
    info: &std::collections::HashMap<String, u64>,
) -> Option<MemoryBreakdown> {
    let get = |key: &str| info.get(key).copied();
    let total = get("MemTotal")?;
    let free = get("MemFree")?;
    let available = get("MemAvailable").unwrap_or(free);
    let cached =
        get("Cached").unwrap_or(0) + get("Buffers").unwrap_or(0) + get("SReclaimable").unwrap_or(0);
    let cached = cached.saturating_sub(get("Shmem").unwrap_or(0));
    let swap_total = get("SwapTotal").unwrap_or(0);
    let swap_free = get("SwapFree").unwrap_or(0);
    Some(MemoryBreakdown {
        total,
        used: total.saturating_sub(available),
        available,
        free,
        cached: Some(cached),
        compressed: get("Zswap"),
        wired: None,
        swap_total,
        swap_used: swap_total.saturating_sub(swap_free),
    })
}

fn hwmon_fans(root: &Path) -> Vec<FanReading> {
    let Ok(chips) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut fans = Vec::new();
    for chip in chips.flatten() {
        let dir = chip.path();
        let chip_name = read_trimmed(&dir.join("name")).unwrap_or_else(|| "hwmon".to_owned());
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut inputs: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|name| name.starts_with("fan") && name.ends_with("_input"))
            .collect();
        inputs.sort();
        for input in inputs {
            let prefix = input.trim_end_matches("_input");
            let Some(rpm) = read_trimmed(&dir.join(&input)).and_then(|v| v.parse::<u32>().ok())
            else {
                continue;
            };
            let label = read_trimmed(&dir.join(format!("{prefix}_label")))
                .unwrap_or_else(|| format!("{chip_name} {prefix}"));
            let max_rpm = read_trimmed(&dir.join(format!("{prefix}_max")))
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|max| *max > 0);
            fans.push(FanReading {
                label,
                rpm,
                max_rpm,
            });
        }
    }
    fans
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Cumulative bytes over interfaces backed by a device (virtual bridges, veth, tunnels and
/// loopback have no `device` link in sysfs and would double count).
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
            .filter(|(name, _)| {
                Path::new("/sys/class/net")
                    .join(name)
                    .join("device")
                    .exists()
            })
            .fold((0u64, 0u64), |(rx, tx), (_, data)| {
                (
                    rx.saturating_add(data.total_received()),
                    tx.saturating_add(data.total_transmitted()),
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_breakdown_from_meminfo() {
        let info = procfs::meminfo(
            "MemTotal: 1000 kB\nMemFree: 100 kB\nMemAvailable: 600 kB\nBuffers: 50 kB\nCached: 300 kB\nSReclaimable: 20 kB\nShmem: 10 kB\nSwapTotal: 200 kB\nSwapFree: 150 kB\n",
        );
        let memory = memory_from_meminfo(&info).unwrap();
        assert_eq!(memory.used, 400 * 1024);
        assert_eq!(memory.cached, Some(360 * 1024));
        assert_eq!(memory.swap_used, 50 * 1024);
        assert_eq!(memory.compressed, None);
    }
}

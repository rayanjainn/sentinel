//! `sysinfo`-backed resource sampler shared by every OS, with per-OS memory, load and fan details.

use std::time::{Duration, Instant};

use sysinfo::{Components, CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use crate::error::CoreResult;
use crate::model::{
    FanReading, LoadAverage, MemoryBreakdown, Platform, ResourceSample, SystemInfo,
    TemperatureReading, ThermalSample,
};
use crate::provider::ResourceProvider;
use crate::util::{RateCounter, now_ms};

pub(crate) trait ResourceExtras: Send {
    fn platform(&self) -> Platform;
    /// `system` has fresh RAM and swap values.
    fn memory(&mut self, system: &System) -> MemoryBreakdown;
    fn load(&mut self, cpu_total: f32, logical_cores: u32) -> Option<LoadAverage>;
    fn fans(&mut self) -> Vec<FanReading> {
        Vec::new()
    }
    /// Cumulative (rx, tx) bytes over physical interfaces.
    fn interface_counters(&mut self) -> CoreResult<(u64, u64)>;
}

/// Sensor enumeration is comparatively expensive (IOKit / WMI); temperatures move slowly.
const THERMAL_REFRESH: Duration = Duration::from_secs(5);

pub(crate) struct SysResources<E: ResourceExtras> {
    system: System,
    components: Option<Components>,
    thermal_cache: Option<(Instant, Option<ThermalSample>)>,
    extras: E,
    net_rate: RateCounter,
    logical_cores: u32,
}

impl<E: ResourceExtras> SysResources<E> {
    pub fn new(extras: E) -> Self {
        let mut system = System::new_with_specifics(
            RefreshKind::nothing()
                .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
                .with_memory(MemoryRefreshKind::everything()),
        );
        system.refresh_cpu_usage();
        let logical_cores = system.cpus().len().max(1) as u32;
        Self {
            system,
            components: None,
            thermal_cache: None,
            extras,
            net_rate: RateCounter::default(),
            logical_cores,
        }
    }

    fn thermal(&mut self) -> Option<ThermalSample> {
        if let Some((at, cached)) = &self.thermal_cache
            && at.elapsed() < THERMAL_REFRESH
        {
            return cached.clone();
        }
        let components = self
            .components
            .get_or_insert_with(Components::new_with_refreshed_list);
        components.refresh(true);
        let temperatures: Vec<TemperatureReading> = components
            .list()
            .iter()
            .filter_map(|component| {
                let celsius = component.temperature()?;
                // Sensors that are present but not reporting read as 0 or NaN; never publish those.
                if !celsius.is_finite() || celsius <= 0.0 || celsius > 150.0 {
                    return None;
                }
                Some(TemperatureReading {
                    label: component.label().to_owned(),
                    celsius,
                    critical_celsius: component.critical().filter(|c| c.is_finite() && *c > 0.0),
                })
            })
            .collect();
        let fans = self.extras.fans();
        let sample = if temperatures.is_empty() && fans.is_empty() {
            None
        } else {
            Some(ThermalSample { temperatures, fans })
        };
        self.thermal_cache = Some((Instant::now(), sample.clone()));
        sample
    }
}

impl<E: ResourceExtras> ResourceProvider for SysResources<E> {
    fn system_info(&self) -> SystemInfo {
        SystemInfo {
            platform: self.extras.platform(),
            hostname: System::host_name(),
            os_name: System::name().unwrap_or_else(|| std::env::consts::OS.to_owned()),
            os_version: System::long_os_version().or_else(System::os_version),
            kernel_version: System::kernel_version(),
            arch: System::cpu_arch(),
            cpu_brand: self
                .system
                .cpus()
                .first()
                .map(|cpu| cpu.brand().trim().to_owned())
                .filter(|brand| !brand.is_empty())
                .unwrap_or_else(|| "Unknown CPU".to_owned()),
            physical_cores: System::physical_core_count().map(|n| n as u32),
            logical_cores: self.logical_cores,
            total_memory: self.system.total_memory(),
            total_swap: self.system.total_swap(),
            boot_time: System::boot_time(),
        }
    }

    fn sample(&mut self) -> CoreResult<ResourceSample> {
        self.system.refresh_cpu_usage();
        self.system
            .refresh_memory_specifics(MemoryRefreshKind::everything());
        let per_core: Vec<f32> = self
            .system
            .cpus()
            .iter()
            .map(|cpu| clamp_percent(cpu.cpu_usage()))
            .collect();
        let cpu_total = clamp_percent(self.system.global_cpu_usage());
        let memory = self.extras.memory(&self.system);
        let load = self.extras.load(cpu_total, self.logical_cores);
        let thermal = self.thermal();
        let network = match self.extras.interface_counters() {
            Ok((rx, tx)) => self.net_rate.update(rx, tx),
            Err(_) => self.net_rate.last(),
        };
        Ok(ResourceSample {
            ts_ms: now_ms(),
            cpu_total,
            per_core,
            memory,
            load,
            thermal,
            network,
        })
    }
}

fn clamp_percent(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

/// Generic breakdown from `sysinfo` when the OS has no finer source.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub(crate) fn basic_memory(system: &System) -> MemoryBreakdown {
    MemoryBreakdown {
        total: system.total_memory(),
        used: system.used_memory(),
        available: system.available_memory(),
        free: system.free_memory(),
        cached: None,
        compressed: None,
        wired: None,
        swap_total: system.total_swap(),
        swap_used: system.used_swap(),
    }
}

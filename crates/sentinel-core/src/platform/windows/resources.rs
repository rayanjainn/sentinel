use std::time::Instant;

use windows::Win32::Foundation::NO_ERROR;
use windows::Win32::NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2, MIB_IF_TABLE2};
use windows::Win32::System::Performance::{
    PDH_FMT_COUNTERVALUE, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY, PdhAddEnglishCounterW,
    PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterValue, PdhOpenQueryW,
};
use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::core::{PCWSTR, w};

use crate::error::{CoreResult, SentinelError};
use crate::model::{LoadAverage, LoadKind, MemoryBreakdown, Platform};
use crate::platform::sys_resources::{ResourceExtras, basic_memory};
use crate::util::DampedLoad;

pub(crate) struct WindowsResources {
    queue: Option<QueueLengthCounter>,
    load: DampedLoad,
    interfaces: InterfaceCounters,
}

impl WindowsResources {
    pub fn new() -> Self {
        Self {
            queue: QueueLengthCounter::open(),
            load: DampedLoad::default(),
            interfaces: InterfaceCounters,
        }
    }
}

impl ResourceExtras for WindowsResources {
    fn platform(&self) -> Platform {
        Platform::Windows
    }

    fn memory(&mut self, system: &sysinfo::System) -> MemoryBreakdown {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        // SAFETY: dwLength is set as required.
        if unsafe { GlobalMemoryStatusEx(&mut status) }.is_err() {
            return basic_memory(system);
        }
        let mut perf = PERFORMANCE_INFORMATION {
            cb: std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32,
            ..Default::default()
        };
        // SAFETY: cb matches the structure size.
        let perf_ok = unsafe { GetPerformanceInfo(&mut perf, perf.cb) }.is_ok();
        let page = perf.PageSize as u64;
        let total = status.ullTotalPhys;
        let available = status.ullAvailPhys;
        MemoryBreakdown {
            total,
            used: total.saturating_sub(available),
            available,
            // Windows folds zeroed and free pages into "available" alongside standby cache.
            free: available.saturating_sub(if perf_ok {
                (perf.SystemCache as u64 * page).min(available)
            } else {
                0
            }),
            cached: perf_ok.then(|| perf.SystemCache as u64 * page),
            compressed: None,
            wired: perf_ok.then(|| perf.KernelNonpaged as u64 * page),
            swap_total: system.total_swap(),
            swap_used: system.used_swap(),
        }
    }

    fn load(&mut self, cpu_total: f32, logical_cores: u32) -> Option<LoadAverage> {
        let queue = self.queue.as_mut()?.sample()?;
        let running = f64::from(cpu_total) / 100.0 * f64::from(logical_cores);
        let (one, five, fifteen) = self.load.update(Instant::now(), queue + running);
        Some(LoadAverage {
            one,
            five,
            fifteen,
            kind: LoadKind::WindowsProcessorQueue,
        })
    }

    fn interface_counters(&mut self) -> CoreResult<(u64, u64)> {
        self.interfaces.totals()
    }
}

struct QueueLengthCounter {
    query: PDH_HQUERY,
    counter: PDH_HCOUNTER,
}

// SAFETY: PDH query handles are not tied to the creating thread; access is serialised by &mut.
unsafe impl Send for QueueLengthCounter {}

impl QueueLengthCounter {
    fn open() -> Option<Self> {
        let mut query = PDH_HQUERY::default();
        // SAFETY: out pointer is valid; null data source means live data.
        if unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) } != 0 {
            return None;
        }
        let mut counter = PDH_HCOUNTER::default();
        // SAFETY: valid query and out pointer.
        let status = unsafe {
            PdhAddEnglishCounterW(
                query,
                w!("\\System\\Processor Queue Length"),
                0,
                &mut counter,
            )
        };
        if status != 0 {
            // SAFETY: query was opened above.
            unsafe { PdhCloseQuery(query) };
            return None;
        }
        Some(Self { query, counter })
    }

    fn sample(&mut self) -> Option<f64> {
        // SAFETY: valid query handle.
        if unsafe { PdhCollectQueryData(self.query) } != 0 {
            return None;
        }
        let mut value = PDH_FMT_COUNTERVALUE::default();
        // SAFETY: valid counter and out pointer.
        let status =
            unsafe { PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, None, &mut value) };
        if status != 0 {
            return None;
        }
        // SAFETY: PDH_FMT_DOUBLE fills the doubleValue member.
        let queue = unsafe { value.Anonymous.doubleValue };
        queue.is_finite().then_some(queue.max(0.0))
    }
}

impl Drop for QueueLengthCounter {
    fn drop(&mut self) {
        // SAFETY: query was opened by us.
        unsafe { PdhCloseQuery(self.query) };
    }
}

/// Octet counters over hardware interfaces, excluding NDIS filter layers that mirror them.
pub(crate) struct InterfaceCounters;

impl InterfaceCounters {
    pub fn totals(&self) -> CoreResult<(u64, u64)> {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        // SAFETY: out pointer is valid; the table is freed with FreeMibTable below.
        let status = unsafe { GetIfTable2(&mut table) };
        if status != NO_ERROR || table.is_null() {
            return Err(SentinelError::internal(format!(
                "GetIfTable2 failed with error {}",
                status.0
            )));
        }
        // SAFETY: GetIfTable2 returned a table with NumEntries rows.
        let rows = unsafe {
            std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
        };
        let mut totals = (0u64, 0u64);
        for row in rows {
            let flags = row.InterfaceAndOperStatusFlags._bitfield;
            let hardware = flags & 0x1 != 0;
            let filter = flags & 0x2 != 0;
            if hardware && !filter {
                totals.0 = totals.0.saturating_add(row.InOctets);
                totals.1 = totals.1.saturating_add(row.OutOctets);
            }
        }
        // SAFETY: table was allocated by GetIfTable2.
        unsafe { FreeMibTable(table.cast()) };
        Ok(totals)
    }
}

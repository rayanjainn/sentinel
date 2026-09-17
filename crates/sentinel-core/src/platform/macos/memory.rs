use crate::model::MemoryBreakdown;

/// Activity Monitor–style breakdown from the Mach VM statistics.
pub(super) fn breakdown(system: &sysinfo::System) -> MemoryBreakdown {
    let total = system.total_memory();
    let swap_total = system.total_swap();
    let swap_used = system.used_swap();
    let Some(vm) = vm_statistics() else {
        return MemoryBreakdown {
            total,
            used: system.used_memory(),
            available: system.available_memory(),
            free: system.free_memory(),
            cached: None,
            compressed: None,
            wired: None,
            swap_total,
            swap_used,
        };
    };
    // SAFETY: sysconf has no preconditions.
    let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) }.max(4096) as u64;
    let pages = |count: u32| u64::from(count) * page;

    let wired = pages(vm.wire_count);
    let compressed = pages(vm.compressor_page_count);
    let app = pages(vm.internal_page_count.saturating_sub(vm.purgeable_count));
    let cached = pages(vm.external_page_count) + pages(vm.purgeable_count);
    let free = pages(vm.free_count.saturating_sub(vm.speculative_count));
    let used = (app + wired + compressed).min(total);
    MemoryBreakdown {
        total,
        used,
        available: total.saturating_sub(used),
        free,
        cached: Some(cached),
        compressed: Some(compressed),
        wired: Some(wired),
        swap_total,
        swap_used,
    }
}

fn vm_statistics() -> Option<libc::vm_statistics64> {
    // SAFETY: vm_statistics64 is plain old data; zeroed is a valid initial value.
    let mut stats: libc::vm_statistics64 = unsafe { std::mem::zeroed() };
    let mut count = libc::HOST_VM_INFO64_COUNT;
    // SAFETY: the buffer is a vm_statistics64 and `count` states its size in integer_t units.
    #[allow(deprecated)]
    let rc = unsafe {
        libc::host_statistics64(
            libc::mach_host_self(),
            libc::HOST_VM_INFO64,
            (&mut stats as *mut libc::vm_statistics64).cast(),
            &mut count,
        )
    };
    (rc == libc::KERN_SUCCESS).then_some(stats)
}

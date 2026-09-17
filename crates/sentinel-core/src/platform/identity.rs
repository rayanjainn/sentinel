use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

use crate::error::{CoreResult, SentinelError};
use crate::model::{Pid, ProcessIdentity};

/// Identity as reported by the same source the process table uses, so start times always compare
/// equal for an unchanged process.
pub fn current_identity(pid: Pid) -> CoreResult<ProcessIdentity> {
    let mut system = System::new();
    let spid = sysinfo::Pid::from_u32(pid);
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[spid]),
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    match system.process(spid) {
        Some(process) if process.exists() => Ok(ProcessIdentity {
            pid,
            start_time: process.start_time(),
        }),
        _ => Err(SentinelError::ProcessNotFound { pid }),
    }
}

/// Re-reads the live identity immediately before acting.
pub(crate) fn verify_identity(target: &ProcessIdentity) -> CoreResult<()> {
    crate::util::guard_process_target(target.pid)?;
    let live = current_identity(target.pid)?;
    if live.start_time != target.start_time {
        return Err(SentinelError::ProcessChanged { pid: target.pid });
    }
    Ok(())
}

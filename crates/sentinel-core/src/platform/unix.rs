//! POSIX pieces shared by macOS and Linux.

use std::io;

use crate::error::{CoreResult, SentinelError};
use crate::model::{LoadAverage, LoadKind, Pid, ProcessIdentity};
use crate::platform::identity::verify_identity;

pub(crate) fn load_average() -> Option<LoadAverage> {
    let mut values = [0f64; 3];
    // SAFETY: getloadavg writes at most `nelem` doubles into the provided buffer.
    let n = unsafe { libc::getloadavg(values.as_mut_ptr(), 3) };
    if n < 3 {
        return None;
    }
    Some(LoadAverage {
        one: values[0],
        five: values[1],
        fifteen: values[2],
        kind: LoadKind::UnixRunQueue,
    })
}

pub(crate) fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions.
    unsafe { libc::geteuid() == 0 }
}

pub(crate) fn signal_error(pid: Pid, operation: &str, err: io::Error) -> SentinelError {
    match err.raw_os_error() {
        Some(libc::ESRCH) => SentinelError::ProcessNotFound { pid },
        Some(libc::EPERM) | Some(libc::EACCES) => SentinelError::PermissionDenied {
            operation: operation.to_owned(),
            target: Some(format!("PID {pid}")),
            hint: Some(
                "The process belongs to another user or the system; Sentinel would need to run \
                 with administrator privileges to act on it."
                    .to_owned(),
            ),
        },
        _ => SentinelError::Io {
            detail: err.to_string(),
            path: None,
        },
    }
}

pub(crate) fn send_signal(target: &ProcessIdentity, signal: libc::c_int) -> CoreResult<()> {
    verify_identity(target)?;
    let operation = if signal == libc::SIGKILL {
        "force kill process"
    } else {
        "terminate process"
    };
    // SAFETY: kill(2) with a positive pid only signals that process.
    let rc = unsafe { libc::kill(target.pid as libc::pid_t, signal) };
    if rc != 0 {
        return Err(signal_error(
            target.pid,
            operation,
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

pub(crate) fn current_nice(pid: Pid) -> Option<i32> {
    // getpriority can legitimately return -1, so errno must be cleared and checked.
    clear_errno();
    // SAFETY: getpriority reads scheduling priority; no memory is passed.
    let value = unsafe { libc::getpriority(libc::PRIO_PROCESS, pid as libc::id_t) };
    if value == -1 && io::Error::last_os_error().raw_os_error().unwrap_or(0) != 0 {
        return None;
    }
    Some(value)
}

pub(crate) fn set_nice(target: &ProcessIdentity, nice: i32) -> CoreResult<()> {
    if !(-20..=19).contains(&nice) {
        return Err(SentinelError::invalid(format!(
            "nice value {nice} is outside -20..=19"
        )));
    }
    verify_identity(target)?;
    let previous = current_nice(target.pid);
    // SAFETY: setpriority adjusts scheduling priority; no memory is passed.
    let rc = unsafe { libc::setpriority(libc::PRIO_PROCESS, target.pid as libc::id_t, nice) };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::EPERM) | Some(libc::EACCES) => {
            let raising = previous.is_some_and(|p| nice < p);
            let hint = if raising {
                "Raising a process's priority (lowering its nice value) requires administrator \
                 privileges. Lowering priority is allowed for your own processes."
            } else {
                "The process belongs to another user or the system; changing its priority \
                 requires administrator privileges."
            };
            Err(SentinelError::PermissionDenied {
                operation: "change process priority".to_owned(),
                target: Some(format!("PID {}", target.pid)),
                hint: Some(hint.to_owned()),
            })
        }
        _ => Err(signal_error(target.pid, "change process priority", err)),
    }
}

#[cfg(target_os = "macos")]
fn clear_errno() {
    // SAFETY: __error returns a valid thread-local errno pointer.
    unsafe { *libc::__error() = 0 }
}

#[cfg(target_os = "linux")]
fn clear_errno() {
    // SAFETY: __errno_location returns a valid thread-local errno pointer.
    unsafe { *libc::__errno_location() = 0 }
}

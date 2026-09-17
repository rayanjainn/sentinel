use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::PathBuf;

use crate::error::{CoreResult, SentinelError};
use crate::model::{OpenFile, OpenFileKind, Pid, ProcessIdentity, TerminateMethod};
use crate::parse::procfs::{self, FdTarget};
use crate::platform::identity::{current_identity, verify_identity};
use crate::platform::proc_table::{ProcExtra, ProcessExtras};
use crate::platform::unix::{self, set_nice, signal_error};
use crate::provider::ProcessControl;

pub(crate) struct LinuxProcessExtras;

impl ProcessExtras for LinuxProcessExtras {
    fn extra(&mut self, pid: Pid) -> ProcExtra {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|text| procfs::pid_stat(&text));
        let fd_count = std::fs::read_dir(format!("/proc/{pid}/fd"))
            .ok()
            .map(|entries| entries.count() as u32);
        ProcExtra {
            thread_count: stat.map(|s| s.num_threads),
            fd_count,
            nice: stat.map(|s| s.nice),
        }
    }

    fn open_files(&self, pid: Pid) -> CoreResult<Vec<OpenFile>> {
        let dir = PathBuf::from(format!("/proc/{pid}/fd"));
        let entries = std::fs::read_dir(&dir).map_err(|err| match err.kind() {
            io::ErrorKind::NotFound => SentinelError::ProcessNotFound { pid },
            _ => signal_error(pid, "list open files", err),
        })?;
        let mut files: Vec<OpenFile> = entries
            .flatten()
            .filter_map(|entry| {
                let fd: i64 = entry.file_name().to_str()?.parse().ok()?;
                let link = std::fs::read_link(entry.path()).ok()?;
                let (kind, path) = match procfs::fd_target(&link.to_string_lossy()) {
                    FdTarget::Socket(_) => (OpenFileKind::Socket, None),
                    FdTarget::Pipe => (OpenFileKind::Pipe, None),
                    FdTarget::Anon(_) => (OpenFileKind::Other, None),
                    FdTarget::Path(path) => {
                        let kind = if path.starts_with("/dev/") {
                            OpenFileKind::Device
                        } else if std::fs::metadata(entry.path()).is_ok_and(|m| m.is_dir()) {
                            OpenFileKind::Directory
                        } else {
                            OpenFileKind::File
                        };
                        (kind, Some(path))
                    }
                };
                Some(OpenFile {
                    fd: Some(fd),
                    kind,
                    path,
                })
            })
            .collect();
        files.sort_by_key(|f| f.fd);
        Ok(files)
    }
}

pub(crate) struct LinuxProcessControl;

/// Signals through a pidfd opened *before* the identity check, so a PID recycled after the check
/// cannot receive the signal.
fn signal(target: &ProcessIdentity, sig: libc::c_int) -> CoreResult<()> {
    crate::util::guard_process_target(target.pid)?;
    // SAFETY: pidfd_open takes a pid and flags and returns a new descriptor or -1.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, target.pid as libc::pid_t, 0) };
    if raw < 0 {
        let err = io::Error::last_os_error();
        return match err.raw_os_error() {
            Some(libc::ESRCH) => Err(SentinelError::ProcessNotFound { pid: target.pid }),
            // Kernels before 5.3 lack pidfds; fall back to kill(2) right after verification.
            _ => unix::send_signal(target, sig),
        };
    }
    // SAFETY: `raw` is a freshly opened descriptor we own.
    let pidfd = unsafe { OwnedFd::from_raw_fd(raw as libc::c_int) };
    verify_identity(target)?;
    use std::os::fd::AsRawFd;
    // SAFETY: pidfd is valid; a null siginfo sends a plain signal.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            sig,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    if rc != 0 {
        let operation = if sig == libc::SIGKILL {
            "force kill process"
        } else {
            "terminate process"
        };
        return Err(signal_error(
            target.pid,
            operation,
            io::Error::last_os_error(),
        ));
    }
    Ok(())
}

impl ProcessControl for LinuxProcessControl {
    fn identity(&self, pid: Pid) -> CoreResult<ProcessIdentity> {
        current_identity(pid)
    }

    fn has_window(&self, _pid: Pid) -> bool {
        // X11/Wayland expose no reliable process → window mapping without a compositor-specific
        // protocol, so graceful termination always uses SIGTERM.
        false
    }

    fn terminate(&self, target: &ProcessIdentity) -> CoreResult<TerminateMethod> {
        signal(target, libc::SIGTERM)?;
        Ok(TerminateMethod::Signal)
    }

    fn force_kill(&self, target: &ProcessIdentity) -> CoreResult<()> {
        signal(target, libc::SIGKILL)
    }

    fn set_priority(&self, target: &ProcessIdentity, nice: i32) -> CoreResult<()> {
        set_nice(target, nice)
    }
}

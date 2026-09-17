use std::ffi::CStr;
use std::io;
use std::mem::size_of;

use objc2::rc::{Retained, autoreleasepool};
use objc2_app_kit::{NSApplicationActivationPolicy, NSRunningApplication};

use super::ffi;
use crate::error::{CoreResult, SentinelError};
use crate::model::{OpenFile, OpenFileKind, Pid, ProcessIdentity, TerminateMethod};
use crate::platform::identity::{current_identity, verify_identity};
use crate::platform::proc_table::{ProcExtra, ProcessExtras};
use crate::platform::unix::{send_signal, set_nice, signal_error};
use crate::provider::ProcessControl;

#[derive(Default)]
pub(crate) struct MacProcessExtras {
    fd_buf: Vec<libc::proc_fdinfo>,
}

impl ProcessExtras for MacProcessExtras {
    fn extra(&mut self, pid: Pid) -> ProcExtra {
        let (thread_count, nice) = match task_all_info(pid) {
            Some(info) => (
                u32::try_from(info.ptinfo.pti_threadnum).ok(),
                Some(info.pbsd.pbi_nice),
            ),
            None => (None, None),
        };
        let fd_count = list_fds(pid, &mut self.fd_buf)
            .ok()
            .map(|fds| fds.len() as u32);
        ProcExtra {
            thread_count,
            fd_count,
            nice,
        }
    }

    fn open_files(&self, pid: Pid) -> CoreResult<Vec<OpenFile>> {
        let mut buf = Vec::new();
        let fds =
            list_fds(pid, &mut buf).map_err(|err| signal_error(pid, "list open files", err))?;
        Ok(fds
            .iter()
            .map(|fd| {
                let kind = match fd.proc_fdtype as libc::c_int {
                    libc::PROX_FDTYPE_VNODE => OpenFileKind::File,
                    libc::PROX_FDTYPE_SOCKET => OpenFileKind::Socket,
                    libc::PROX_FDTYPE_PIPE => OpenFileKind::Pipe,
                    _ => OpenFileKind::Other,
                };
                let (kind, path) = if kind == OpenFileKind::File {
                    match vnode_path(pid, fd.proc_fd) {
                        Some((path, vtype)) => (vnode_kind(vtype), Some(path)),
                        None => (OpenFileKind::File, None),
                    }
                } else {
                    (kind, None)
                };
                OpenFile {
                    fd: Some(i64::from(fd.proc_fd)),
                    kind,
                    path,
                }
            })
            .collect())
    }
}

fn task_all_info(pid: Pid) -> Option<libc::proc_taskallinfo> {
    // SAFETY: proc_taskallinfo is plain old data.
    let mut info: libc::proc_taskallinfo = unsafe { std::mem::zeroed() };
    let size = size_of::<libc::proc_taskallinfo>() as libc::c_int;
    // SAFETY: buffer and size describe `info`.
    let written = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTASKALLINFO,
            0,
            (&mut info as *mut libc::proc_taskallinfo).cast(),
            size,
        )
    };
    (written == size).then_some(info)
}

fn list_fds(pid: Pid, buf: &mut Vec<libc::proc_fdinfo>) -> io::Result<&[libc::proc_fdinfo]> {
    let entry = size_of::<libc::proc_fdinfo>();
    // SAFETY: a null buffer asks for the required size.
    let needed = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDLISTFDS,
            0,
            std::ptr::null_mut(),
            0,
        )
    };
    if needed <= 0 {
        return Err(last_error_or_denied());
    }
    // Leave headroom for descriptors opened between the two calls.
    let capacity = needed as usize / entry + 16;
    buf.clear();
    buf.resize(
        capacity,
        libc::proc_fdinfo {
            proc_fd: 0,
            proc_fdtype: 0,
        },
    );
    // SAFETY: buffer holds `capacity` proc_fdinfo entries.
    let written = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDLISTFDS,
            0,
            buf.as_mut_ptr().cast(),
            (capacity * entry) as libc::c_int,
        )
    };
    if written <= 0 {
        return Err(last_error_or_denied());
    }
    Ok(&buf[..written as usize / entry])
}

fn last_error_or_denied() -> io::Error {
    let err = io::Error::last_os_error();
    if err.raw_os_error().unwrap_or(0) == 0 {
        io::Error::from_raw_os_error(libc::EPERM)
    } else {
        err
    }
}

fn vnode_path(pid: Pid, fd: i32) -> Option<(String, libc::c_int)> {
    // SAFETY: vnode_fdinfowithpath is plain old data.
    let mut info: ffi::vnode_fdinfowithpath = unsafe { std::mem::zeroed() };
    let size = size_of::<ffi::vnode_fdinfowithpath>() as libc::c_int;
    // SAFETY: buffer and size describe `info`.
    let written = unsafe {
        libc::proc_pidfdinfo(
            pid as libc::c_int,
            fd,
            ffi::PROC_PIDFDVNODEPATHINFO,
            (&mut info as *mut ffi::vnode_fdinfowithpath).cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    // SAFETY: the kernel NUL-terminates vip_path within MAXPATHLEN.
    let path = unsafe { CStr::from_ptr(info.pvip.vip_path.as_ptr()) };
    let path = path.to_string_lossy().into_owned();
    (!path.is_empty()).then_some((path, info.pvip.vip_vi.vi_type))
}

/// `enum vtype` from `<sys/vnode.h>`.
fn vnode_kind(vtype: libc::c_int) -> OpenFileKind {
    match vtype {
        1 => OpenFileKind::File,
        2 => OpenFileKind::Directory,
        3 | 4 => OpenFileKind::Device,
        6 => OpenFileKind::Socket,
        7 => OpenFileKind::Pipe,
        _ => OpenFileKind::Other,
    }
}

pub(crate) struct MacProcessControl;

/// A regular (Dock) application that can be asked to quit like ⌘Q.
fn gui_application(pid: Pid) -> Option<Retained<NSRunningApplication>> {
    let pid = libc::pid_t::try_from(pid).ok()?;
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    (app.activationPolicy() == NSApplicationActivationPolicy::Regular && !app.isTerminated())
        .then_some(app)
}

impl ProcessControl for MacProcessControl {
    fn identity(&self, pid: Pid) -> CoreResult<ProcessIdentity> {
        current_identity(pid)
    }

    fn has_window(&self, pid: Pid) -> bool {
        autoreleasepool(|_| gui_application(pid).is_some())
    }

    fn terminate(&self, target: &ProcessIdentity) -> CoreResult<TerminateMethod> {
        verify_identity(target)?;
        let asked_app =
            autoreleasepool(|_| gui_application(target.pid).is_some_and(|app| app.terminate()));
        if asked_app {
            return Ok(TerminateMethod::WindowClose);
        }
        send_signal(target, libc::SIGTERM)?;
        Ok(TerminateMethod::Signal)
    }

    fn force_kill(&self, target: &ProcessIdentity) -> CoreResult<()> {
        send_signal(target, libc::SIGKILL)
    }

    fn set_priority(&self, target: &ProcessIdentity, nice: i32) -> CoreResult<()> {
        set_nice(target, nice)
    }
}

#[allow(dead_code)]
fn not_found(pid: Pid) -> SentinelError {
    SentinelError::ProcessNotFound { pid }
}

use std::collections::HashMap;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{
    ABOVE_NORMAL_PRIORITY_CLASS, BELOW_NORMAL_PRIORITY_CLASS, GetPriorityClass,
    GetProcessHandleCount, HIGH_PRIORITY_CLASS, IDLE_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS,
    PROCESS_CREATION_FLAGS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
    PROCESS_TERMINATE, REALTIME_PRIORITY_CLASS, SetPriorityClass, TerminateProcess,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
    WM_CLOSE,
};
use windows::core::BOOL;

use super::handle::{OwnedHandle, map_error, open_process};
use crate::error::{CoreResult, SentinelError};
use crate::model::{OpenFile, Pid, ProcessIdentity, TerminateMethod};
use crate::platform::identity::{current_identity, verify_identity};
use crate::platform::proc_table::{ProcExtra, ProcessExtras};
use crate::provider::ProcessControl;

#[derive(Default)]
pub(crate) struct WindowsProcessExtras {
    threads: HashMap<Pid, u32>,
}

impl ProcessExtras for WindowsProcessExtras {
    fn begin_refresh(&mut self) {
        self.threads = thread_counts();
    }

    fn extra(&mut self, pid: Pid) -> ProcExtra {
        let thread_count = self.threads.get(&pid).copied();
        let (fd_count, nice) = match open_process(pid, PROCESS_QUERY_LIMITED_INFORMATION, "query") {
            Ok(handle) => {
                let mut count = 0u32;
                // SAFETY: valid process handle and out pointer.
                let handles = unsafe { GetProcessHandleCount(handle.0, &mut count) }
                    .ok()
                    .map(|_| count);
                // SAFETY: valid process handle.
                let class = unsafe { GetPriorityClass(handle.0) };
                (handles, priority_class_to_nice(class))
            }
            Err(_) => (None, None),
        };
        ProcExtra {
            thread_count,
            fd_count,
            nice,
        }
    }

    fn open_files(&self, pid: Pid) -> CoreResult<Vec<OpenFile>> {
        super::handles::open_files(pid)
    }
}

fn thread_counts() -> HashMap<Pid, u32> {
    let mut counts = HashMap::new();
    // SAFETY: snapshot of all processes; closed by OwnedHandle.
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return counts;
    };
    let snapshot = OwnedHandle(snapshot);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    // SAFETY: entry.dwSize is initialised as the API requires.
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) }.is_ok();
    while ok {
        counts.insert(entry.th32ProcessID, entry.cntThreads);
        // SAFETY: as above.
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) }.is_ok();
    }
    counts
}

/// Windows priority classes projected onto the Unix nice scale (see `Action::SetProcessPriority`).
pub(crate) fn priority_class_to_nice(class: u32) -> Option<i32> {
    match PROCESS_CREATION_FLAGS(class) {
        REALTIME_PRIORITY_CLASS => Some(-20),
        HIGH_PRIORITY_CLASS => Some(-15),
        ABOVE_NORMAL_PRIORITY_CLASS => Some(-5),
        NORMAL_PRIORITY_CLASS => Some(0),
        BELOW_NORMAL_PRIORITY_CLASS => Some(5),
        IDLE_PRIORITY_CLASS => Some(15),
        _ => None,
    }
}

pub(crate) fn nice_to_priority_class(nice: i32) -> PROCESS_CREATION_FLAGS {
    match nice {
        i32::MIN..=-11 => HIGH_PRIORITY_CLASS,
        -10..=-1 => ABOVE_NORMAL_PRIORITY_CLASS,
        0 => NORMAL_PRIORITY_CLASS,
        1..=10 => BELOW_NORMAL_PRIORITY_CLASS,
        _ => IDLE_PRIORITY_CLASS,
    }
}

struct WindowSearch {
    pid: Pid,
    windows: Vec<HWND>,
}

unsafe extern "system" fn collect_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam carries the &mut WindowSearch passed to EnumWindows for this call only.
    let search = unsafe { &mut *(lparam.0 as *mut WindowSearch) };
    let mut owner_pid = 0u32;
    // SAFETY: valid window handle from EnumWindows.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut owner_pid)) };
    // SAFETY: valid window handle.
    let visible = unsafe { IsWindowVisible(hwnd) }.as_bool();
    // SAFETY: valid window handle; an error means the window has no owner.
    let owned = unsafe { GetWindow(hwnd, GW_OWNER) }.is_ok_and(|owner| !owner.is_invalid());
    if owner_pid == search.pid && visible && !owned {
        search.windows.push(hwnd);
    }
    true.into()
}

/// Visible, unowned top-level windows belonging to `pid`.
fn top_level_windows(pid: Pid) -> Vec<HWND> {
    let mut search = WindowSearch {
        pid,
        windows: Vec::new(),
    };
    // SAFETY: the callback only dereferences `search`, which outlives the call.
    let _ = unsafe {
        EnumWindows(
            Some(collect_windows),
            LPARAM(&mut search as *mut WindowSearch as isize),
        )
    };
    search.windows
}

pub(crate) struct WindowsProcessControl;

impl ProcessControl for WindowsProcessControl {
    fn identity(&self, pid: Pid) -> CoreResult<ProcessIdentity> {
        current_identity(pid)
    }

    fn has_window(&self, pid: Pid) -> bool {
        !top_level_windows(pid).is_empty()
    }

    fn terminate(&self, target: &ProcessIdentity) -> CoreResult<TerminateMethod> {
        verify_identity(target)?;
        let windows = top_level_windows(target.pid);
        if !windows.is_empty() {
            let mut delivered = false;
            for hwnd in windows {
                // SAFETY: posting WM_CLOSE has no memory preconditions.
                delivered |=
                    unsafe { PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) }.is_ok();
            }
            if delivered {
                return Ok(TerminateMethod::WindowClose);
            }
        }
        kill(target)?;
        Ok(TerminateMethod::TerminateProcess)
    }

    fn force_kill(&self, target: &ProcessIdentity) -> CoreResult<()> {
        verify_identity(target)?;
        kill(target)
    }

    fn set_priority(&self, target: &ProcessIdentity, nice: i32) -> CoreResult<()> {
        if !(-20..=19).contains(&nice) {
            return Err(SentinelError::invalid(format!(
                "nice value {nice} is outside -20..=19"
            )));
        }
        let handle = open_process(
            target.pid,
            PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
            "change process priority",
        )?;
        verify_identity(target)?;
        // SAFETY: valid handle with PROCESS_SET_INFORMATION.
        unsafe { SetPriorityClass(handle.0, nice_to_priority_class(nice)) }
            .map_err(|err| map_error(target.pid, "change process priority", &err))
    }
}

fn kill(target: &ProcessIdentity) -> CoreResult<()> {
    let handle = open_process(
        target.pid,
        PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
        "terminate process",
    )?;
    // The handle pins the process object, so re-checking identity now closes the PID reuse window.
    verify_identity(target)?;
    // SAFETY: valid handle with PROCESS_TERMINATE.
    unsafe { TerminateProcess(handle.0, 1) }
        .map_err(|err| map_error(target.pid, "terminate process", &err))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_mapping_round_trips() {
        for nice in [-15, -5, 0, 5, 15] {
            let class = nice_to_priority_class(nice);
            assert_eq!(priority_class_to_nice(class.0), Some(nice));
        }
        assert_eq!(nice_to_priority_class(-20), HIGH_PRIORITY_CLASS);
    }
}

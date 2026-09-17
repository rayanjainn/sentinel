use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, HANDLE,
};
use windows::Win32::System::Threading::{OpenProcess, PROCESS_ACCESS_RIGHTS};

use crate::error::SentinelError;
use crate::model::Pid;

/// Closes the wrapped handle on drop.
pub(crate) struct OwnedHandle(pub HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: the handle was opened by us and is closed exactly once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

pub(crate) fn open_process(
    pid: Pid,
    access: PROCESS_ACCESS_RIGHTS,
    operation: &str,
) -> Result<OwnedHandle, SentinelError> {
    // SAFETY: OpenProcess has no memory preconditions.
    unsafe { OpenProcess(access, false, pid) }
        .map(OwnedHandle)
        .map_err(|err| map_error(pid, operation, &err))
}

pub(crate) fn map_error(pid: Pid, operation: &str, err: &windows::core::Error) -> SentinelError {
    let code = err.code();
    if code == ERROR_ACCESS_DENIED.to_hresult() {
        SentinelError::PermissionDenied {
            operation: operation.to_owned(),
            target: Some(format!("PID {pid}")),
            hint: Some(
                "The process runs as another user or with higher integrity; run Sentinel as \
                 administrator to act on it."
                    .to_owned(),
            ),
        }
    } else if code == ERROR_INVALID_PARAMETER.to_hresult() {
        SentinelError::ProcessNotFound { pid }
    } else {
        SentinelError::Io {
            detail: format!("{operation}: {}", err.message()),
            path: None,
        }
    }
}

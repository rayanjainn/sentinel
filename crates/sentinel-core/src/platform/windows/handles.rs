//! Open file handles of another process: system handle table → duplicate → classify.
//!
//! Some handle kinds (synchronous named pipes with a pending read) block file queries indefinitely,
//! so known hanging access masks are skipped and the inspection runs on a worker with a deadline.

use std::time::{Duration, Instant};

use windows::Wdk::System::SystemInformation::{NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS};
use windows::Win32::Foundation::{DUPLICATE_SAME_ACCESS, DuplicateHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    FILE_NAME_NORMALIZED, FILE_TYPE_CHAR, FILE_TYPE_DISK, FILE_TYPE_PIPE, GetFileType,
    GetFinalPathNameByHandleW,
};
use windows::Win32::System::Threading::{GetCurrentProcess, PROCESS_DUP_HANDLE};

use super::handle::{OwnedHandle, open_process};
use crate::error::{CoreResult, SentinelError};
use crate::model::{OpenFile, OpenFileKind, Pid};

const SYSTEM_EXTENDED_HANDLE_INFORMATION: SYSTEM_INFORMATION_CLASS = SYSTEM_INFORMATION_CLASS(64);
const STATUS_INFO_LENGTH_MISMATCH: i32 = 0xC000_0004_u32 as i32;
const INSPECTION_DEADLINE: Duration = Duration::from_secs(2);
/// Granted-access masks of synchronous pipe handles known to block `GetFileType` and path queries.
const HANGING_ACCESS_MASKS: [u32; 4] = [0x0012_019F, 0x001A_019F, 0x0012_0189, 0x0010_0000];

#[repr(C)]
#[derive(Clone, Copy)]
struct SystemHandleEntry {
    object: usize,
    unique_process_id: usize,
    handle_value: usize,
    granted_access: u32,
    creator_back_trace_index: u16,
    object_type_index: u16,
    handle_attributes: u32,
    reserved: u32,
}

struct SendHandle(OwnedHandle);
// SAFETY: kernel handles are process-wide and valid on any thread.
unsafe impl Send for SendHandle {}

pub(crate) fn open_files(pid: Pid) -> CoreResult<Vec<OpenFile>> {
    let process = SendHandle(open_process(pid, PROCESS_DUP_HANDLE, "list open handles")?);
    let handles = process_handles(pid)?;
    let (tx, rx) = crossbeam_channel::unbounded();
    std::thread::Builder::new()
        .name("sentinel-handles".into())
        .spawn(move || {
            let process = process;
            for (value, access) in handles {
                if HANGING_ACCESS_MASKS.contains(&access) {
                    continue;
                }
                if let Some(file) = inspect(&process.0, value) {
                    if tx.send(file).is_err() {
                        return;
                    }
                }
            }
        })
        .map_err(|err| SentinelError::internal(format!("could not inspect handles: {err}")))?;

    let deadline = Instant::now() + INSPECTION_DEADLINE;
    let mut files = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(file) => files.push(file),
            Err(_) => break,
        }
    }
    files.sort_by_key(|f| f.fd);
    Ok(files)
}

fn process_handles(pid: Pid) -> CoreResult<Vec<(usize, u32)>> {
    let mut buffer: Vec<u8> = vec![0; 1 << 20];
    loop {
        let mut needed = 0u32;
        // SAFETY: buffer pointer/length describe a writable allocation.
        let status = unsafe {
            NtQuerySystemInformation(
                SYSTEM_EXTENDED_HANDLE_INFORMATION,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut needed,
            )
        };
        if status.0 == STATUS_INFO_LENGTH_MISMATCH {
            let next = (needed as usize).max(buffer.len() * 2);
            if next > 512 << 20 {
                return Err(SentinelError::internal("system handle table is too large"));
            }
            buffer.resize(next + (1 << 16), 0);
            continue;
        }
        if status.is_err() {
            return Err(SentinelError::internal(format!(
                "NtQuerySystemInformation failed with status {:#x}",
                status.0
            )));
        }
        break;
    }
    let header = std::mem::size_of::<usize>() * 2;
    if buffer.len() < header {
        return Ok(Vec::new());
    }
    // SAFETY: the buffer starts with NumberOfHandles (ULONG_PTR).
    let count = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<usize>()) };
    let entry_size = std::mem::size_of::<SystemHandleEntry>();
    let available = (buffer.len() - header) / entry_size;
    let mut handles = Vec::new();
    for index in 0..count.min(available) {
        // SAFETY: index is within the populated region of the buffer.
        let entry: SystemHandleEntry = unsafe {
            std::ptr::read_unaligned(buffer.as_ptr().add(header + index * entry_size).cast())
        };
        if entry.unique_process_id == pid as usize {
            handles.push((entry.handle_value, entry.granted_access));
        }
    }
    Ok(handles)
}

fn inspect(process: &OwnedHandle, value: usize) -> Option<OpenFile> {
    let mut duplicate = HANDLE::default();
    // SAFETY: duplicating into our own process; the result is closed by OwnedHandle.
    unsafe {
        DuplicateHandle(
            process.0,
            HANDLE(value as *mut core::ffi::c_void),
            GetCurrentProcess(),
            &mut duplicate,
            0,
            false,
            DUPLICATE_SAME_ACCESS,
        )
    }
    .ok()?;
    let duplicate = OwnedHandle(duplicate);
    // SAFETY: valid handle.
    let file_type = unsafe { GetFileType(duplicate.0) };
    let (kind, path) = if file_type == FILE_TYPE_DISK {
        let path = final_path(&duplicate)?;
        let kind = if std::fs::metadata(&path).is_ok_and(|m| m.is_dir()) {
            OpenFileKind::Directory
        } else {
            OpenFileKind::File
        };
        (kind, Some(path))
    } else if file_type == FILE_TYPE_PIPE {
        (OpenFileKind::Pipe, None)
    } else if file_type == FILE_TYPE_CHAR {
        (OpenFileKind::Device, None)
    } else {
        return None;
    };
    Some(OpenFile {
        fd: Some(value as i64),
        kind,
        path,
    })
}

fn final_path(handle: &OwnedHandle) -> Option<String> {
    let mut buffer = vec![0u16; 1024];
    loop {
        // SAFETY: buffer is writable for its length. FILE_NAME_NORMALIZED with VOLUME_NAME_DOS (0)
        // yields a drive-letter path.
        let len = unsafe { GetFinalPathNameByHandleW(handle.0, &mut buffer, FILE_NAME_NORMALIZED) }
            as usize;
        if len == 0 {
            return None;
        }
        if len >= buffer.len() {
            buffer.resize(len + 1, 0);
            continue;
        }
        let path = String::from_utf16_lossy(&buffer[..len]);
        return Some(path.strip_prefix(r"\\?\").unwrap_or(&path).to_owned());
    }
}

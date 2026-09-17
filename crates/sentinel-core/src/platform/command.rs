//! Running helper executables (open, pfctl, netsh, …) with specific error mapping.

use std::process::{Command, Output, Stdio};

use crate::error::{CoreResult, SentinelError};

pub(crate) fn run_output(program: &str, args: &[&str], operation: &str) -> CoreResult<Output> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => SentinelError::Unavailable {
                feature: operation.to_owned(),
                reason: format!("{program} was not found"),
            },
            _ => SentinelError::Io {
                detail: format!("could not run {program}: {err}"),
                path: None,
            },
        })
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
/// Runs to completion and fails with the tool's stderr when it exits non-zero.
pub(crate) fn run_status(program: &str, args: &[&str], operation: &str) -> CoreResult<()> {
    let output = run_output(program, args, operation)?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Err(SentinelError::Io {
        detail: if stderr.is_empty() {
            format!("{operation} failed ({})", output.status)
        } else {
            format!("{operation} failed: {stderr}")
        },
        path: None,
    })
}

/// Locates an executable on PATH, also checking the sbin directories GUI apps often lack.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn which(program: &str) -> Option<std::path::PathBuf> {
    #[cfg_attr(windows, allow(unused_mut))]
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    #[cfg(unix)]
    for extra in [
        "/usr/local/sbin",
        "/usr/local/bin",
        "/usr/sbin",
        "/usr/bin",
        "/sbin",
        "/bin",
    ] {
        dirs.push(extra.into());
    }
    #[cfg(windows)]
    let names = [format!("{program}.exe"), program.to_owned()];
    #[cfg(not(windows))]
    let names = [program.to_owned()];
    dirs.into_iter().find_map(|dir| {
        names
            .iter()
            .map(|name| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// Detached launch for GUI helpers whose exit status is irrelevant (file managers).
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub(crate) fn spawn_detached(program: &str, args: &[&str], operation: &str) -> CoreResult<()> {
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| SentinelError::Unavailable {
            feature: operation.to_owned(),
            reason: format!("could not launch {program}: {err}"),
        })?;
    reap(child);
    Ok(())
}

/// Waits for a detached child on a background thread so it never lingers as a zombie.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub(crate) fn reap(mut child: std::process::Child) {
    let _ = std::thread::Builder::new()
        .name("sentinel-reaper".into())
        .spawn(move || {
            let _ = child.wait();
        });
}

//! `FileOps` for every OS: the OS trash via the `trash` crate, same-volume rename or
//! copy-then-trash across volumes, and reveal in the platform file manager.
//!
//! Nothing here unlinks user data; `clippy.toml` forbids the std removal functions.

use std::io;
use std::path::{Path, PathBuf};

use crate::error::{CoreResult, SentinelError};
use crate::provider::FileOps;
use crate::util::path_string;

pub(crate) struct PlatformFileOps;

fn trash_context() -> trash::TrashContext {
    #[allow(unused_mut)]
    let mut context = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // NSFileManager trashItem: no Finder automation permission prompt.
        context.set_delete_method(DeleteMethod::NsFileManager);
    }
    context
}

fn trash_error(path: &Path, err: trash::Error) -> SentinelError {
    match err {
        trash::Error::TargetedRoot => SentinelError::invalid(format!(
            "{} is a filesystem root and cannot be trashed",
            path_string(path)
        )),
        #[cfg(target_os = "linux")]
        trash::Error::FileSystem { source, .. } => SentinelError::io(&source, Some(path)),
        trash::Error::CouldNotAccess { .. } => SentinelError::PermissionDenied {
            operation: "move to Trash".to_owned(),
            target: Some(path_string(path)),
            hint: None,
        },
        other => SentinelError::Io {
            detail: format!("could not move to Trash: {other}"),
            path: Some(path_string(path)),
        },
    }
}

impl FileOps for PlatformFileOps {
    fn trash(&self, path: &Path) -> CoreResult<()> {
        std::fs::symlink_metadata(path).map_err(|err| SentinelError::io(&err, Some(path)))?;
        trash_context()
            .delete(path)
            .map_err(|err| trash_error(path, err))
    }

    fn move_into(&self, path: &Path, destination_dir: &Path) -> CoreResult<PathBuf> {
        let source_meta =
            std::fs::symlink_metadata(path).map_err(|err| SentinelError::io(&err, Some(path)))?;
        let dest_meta = std::fs::metadata(destination_dir)
            .map_err(|err| SentinelError::io(&err, Some(destination_dir)))?;
        if !dest_meta.is_dir() {
            return Err(SentinelError::invalid(format!(
                "{} is not a folder",
                path_string(destination_dir)
            )));
        }
        let name = path.file_name().ok_or_else(|| {
            SentinelError::invalid(format!("{} has no file name", path_string(path)))
        })?;
        let target = destination_dir.join(name);
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(SentinelError::invalid(format!(
                "{} already exists in the destination",
                path_string(&target)
            )));
        }
        if source_meta.is_dir() && destination_dir.starts_with(path) {
            return Err(SentinelError::invalid(
                "a folder cannot be moved into itself",
            ));
        }
        match std::fs::rename(path, &target) {
            Ok(()) => Ok(target),
            Err(err) if err.kind() == io::ErrorKind::CrossesDevices => {
                if let Err(copy_err) = copy_tree(path, &target) {
                    // Leave the source untouched; a partial copy goes to the trash, never unlinked.
                    if std::fs::symlink_metadata(&target).is_ok() {
                        let _ = self.trash(&target);
                    }
                    return Err(SentinelError::io(&copy_err, Some(path)));
                }
                self.trash(path)?;
                Ok(target)
            }
            Err(err) => Err(SentinelError::io(&err, Some(path))),
        }
    }

    fn reveal(&self, path: &Path) -> CoreResult<()> {
        std::fs::symlink_metadata(path).map_err(|err| SentinelError::io(&err, Some(path)))?;
        reveal(path)
    }
}

fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    let meta = std::fs::symlink_metadata(source)?;
    if meta.file_type().is_symlink() {
        let link = std::fs::read_link(source)?;
        #[cfg(unix)]
        return std::os::unix::fs::symlink(link, target);
        #[cfg(windows)]
        {
            return if std::fs::metadata(source).is_ok_and(|m| m.is_dir()) {
                std::os::windows::fs::symlink_dir(link, target)
            } else {
                std::os::windows::fs::symlink_file(link, target)
            };
        }
    }
    if meta.is_dir() {
        std::fs::create_dir(target)?;
        for entry in std::fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
        std::fs::set_permissions(target, meta.permissions())?;
        return Ok(());
    }
    std::fs::copy(source, target)?;
    let mut times = std::fs::FileTimes::new();
    if let Ok(modified) = meta.modified() {
        times = times.set_modified(modified);
    }
    if let Ok(accessed) = meta.accessed() {
        times = times.set_accessed(accessed);
    }
    std::fs::File::options()
        .write(true)
        .open(target)?
        .set_times(times)
}

#[cfg(target_os = "macos")]
fn reveal(path: &Path) -> CoreResult<()> {
    let path = path_string(path);
    crate::platform::command::run_status("/usr/bin/open", &["-R", &path], "reveal in Finder")
}

#[cfg(windows)]
fn reveal(path: &Path) -> CoreResult<()> {
    use std::os::windows::process::CommandExt;
    // explorer.exe needs the path quoted inside the /select, argument and exits non-zero even on
    // success, so it is launched detached with a raw argument.
    let child = std::process::Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn()
        .map_err(|err| SentinelError::Unavailable {
            feature: "reveal in Explorer".to_owned(),
            reason: err.to_string(),
        })?;
    crate::platform::command::reap(child);
    Ok(())
}

#[cfg(target_os = "linux")]
fn reveal(path: &Path) -> CoreResult<()> {
    use crate::platform::command::{run_output, spawn_detached, which};
    let absolute =
        std::fs::canonicalize(path).map_err(|err| SentinelError::io(&err, Some(path)))?;
    let uri = crate::parse::uri::file_uri(&path_string(&absolute));
    if which("dbus-send").is_some() {
        let items = format!("array:string:{uri}");
        let shown = run_output(
            "dbus-send",
            &[
                "--session",
                "--print-reply",
                "--dest=org.freedesktop.FileManager1",
                "--type=method_call",
                "/org/freedesktop/FileManager1",
                "org.freedesktop.FileManager1.ShowItems",
                &items,
                "string:",
            ],
            "reveal in file manager",
        )
        .is_ok_and(|out| out.status.success());
        if shown {
            return Ok(());
        }
    }
    let parent = absolute.parent().unwrap_or(&absolute);
    spawn_detached("xdg-open", &[&path_string(parent)], "open file manager")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_tree_preserves_structure_and_times() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("nested")).unwrap();
        std::fs::write(src.join("nested/a.txt"), b"hello").unwrap();
        let old = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
        std::fs::File::options()
            .write(true)
            .open(src.join("nested/a.txt"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let dst = dir.path().join("dst");
        copy_tree(&src, &dst).unwrap();
        assert_eq!(std::fs::read(dst.join("nested/a.txt")).unwrap(), b"hello");
        let modified = std::fs::metadata(dst.join("nested/a.txt"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(modified, old);
    }

    #[test]
    fn move_into_same_volume_and_rejects_collisions() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("report.pdf");
        std::fs::write(&file, b"x").unwrap();
        let dest = dir.path().join("archive");
        std::fs::create_dir(&dest).unwrap();
        let ops = PlatformFileOps;
        let moved = ops.move_into(&file, &dest).unwrap();
        assert_eq!(moved, dest.join("report.pdf"));
        assert!(!file.exists() && moved.exists());
        std::fs::write(&file, b"y").unwrap();
        assert!(matches!(
            ops.move_into(&file, &dest),
            Err(SentinelError::InvalidInput { .. })
        ));
        assert!(matches!(
            ops.move_into(&dir.path().join("missing"), &dest),
            Err(SentinelError::PathNotFound { .. })
        ));
    }
}

//! Pieces of `StorageProvider` shared by the OS implementations.

use std::path::{Path, PathBuf};

use crate::model::{CleanupCategory, CleanupLocation, VolumeInfo};
use crate::util::home_dir;

pub(crate) fn location(
    category: CleanupCategory,
    path: impl Into<PathBuf>,
    rationale: &'static str,
) -> CleanupLocation {
    CleanupLocation {
        category,
        path: path.into(),
        rationale,
    }
}

pub(crate) fn home_join(relative: &str) -> Option<PathBuf> {
    home_dir().map(|home| home.join(relative))
}

/// Volumes from `sysinfo`, with `keep` filtering out pseudo and duplicate mounts.
pub(crate) fn sysinfo_volumes(
    keep: impl Fn(&Path, &str) -> bool,
    name_of: impl Fn(&Path, &str) -> String,
    is_system: impl Fn(&Path) -> bool,
) -> Vec<VolumeInfo> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut volumes: Vec<VolumeInfo> = disks
        .list()
        .iter()
        .filter_map(|disk| {
            let mount = disk.mount_point();
            let file_system = disk.file_system().to_string_lossy().into_owned();
            if disk.total_space() == 0 || !keep(mount, &file_system) {
                return None;
            }
            let device_name = disk.name().to_string_lossy().into_owned();
            Some(VolumeInfo {
                mount_point: mount.to_string_lossy().into_owned(),
                name: name_of(mount, &device_name),
                file_system,
                total_bytes: disk.total_space(),
                available_bytes: disk.available_space(),
                is_removable: disk.is_removable(),
                is_system: is_system(mount),
            })
        })
        .collect();
    volumes.sort_by(|a, b| {
        b.is_system
            .cmp(&a.is_system)
            .then_with(|| a.mount_point.cmp(&b.mount_point))
    });
    volumes.dedup_by(|a, b| a.mount_point == b.mount_point);
    volumes
}

/// Package-manager and developer caches found under the home directory on every OS.
pub(crate) fn common_developer_caches() -> Vec<CleanupLocation> {
    let mut out = Vec::new();
    let entries: [(CleanupCategory, &str, &'static str); 8] = [
        (
            CleanupCategory::PackageManagerCache,
            ".npm/_cacache",
            "npm's download cache; packages are fetched again when a project installs them.",
        ),
        (
            CleanupCategory::PackageManagerCache,
            ".cargo/registry/cache",
            "Downloaded crate archives; Cargo re-downloads them when a build needs them.",
        ),
        (
            CleanupCategory::PackageManagerCache,
            ".gradle/caches",
            "Gradle's dependency and build cache; rebuilt on the next Gradle build.",
        ),
        (
            CleanupCategory::PackageManagerCache,
            ".yarn/berry/cache",
            "Yarn's global package cache; refilled on the next install.",
        ),
        (
            CleanupCategory::PackageManagerCache,
            "go/pkg/mod/cache",
            "Go module download cache; modules are fetched again when needed.",
        ),
        (
            CleanupCategory::PackageManagerCache,
            ".m2/repository",
            "Maven's local repository; artifacts are downloaded again on the next build.",
        ),
        (
            CleanupCategory::DeveloperCache,
            ".ollama/models/blobs",
            "Downloaded Ollama model weights; removing them requires pulling the models again.",
        ),
        (
            CleanupCategory::DeveloperCache,
            ".gradle/wrapper/dists",
            "Gradle distributions downloaded by project wrappers; fetched again on demand.",
        ),
    ];
    for (category, relative, rationale) in entries {
        if let Some(path) = home_join(relative) {
            out.push(location(category, path, rationale));
        }
    }
    out
}

pub(crate) fn downloads_location() -> Option<CleanupLocation> {
    home_join("Downloads").map(|path| {
        location(
            CleanupCategory::OldDownloads,
            path,
            "Downloaded more than 90 days ago and not opened since; installers and archives are \
             often no longer needed.",
        )
    })
}

#[cfg(unix)]
pub(crate) mod unix {
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    use crate::error::{CoreResult, SentinelError};
    use crate::model::{EntryKind, EntryMetadata};

    pub(crate) fn lstat(path: &Path) -> CoreResult<EntryMetadata> {
        let meta =
            std::fs::symlink_metadata(path).map_err(|err| SentinelError::io(&err, Some(path)))?;
        let file_type = meta.file_type();
        let kind = if file_type.is_symlink() {
            EntryKind::Symlink
        } else if file_type.is_dir() {
            EntryKind::Directory
        } else if file_type.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        let secs = |value: i64| u64::try_from(value).ok();
        Ok(EntryMetadata {
            kind,
            allocated_bytes: meta.blocks().saturating_mul(512),
            apparent_bytes: meta.size(),
            modified: secs(meta.mtime()),
            accessed: secs(meta.atime()),
            device: meta.dev(),
            file_id: meta.ino(),
            hard_links: meta.nlink(),
        })
    }

    pub(crate) fn statvfs(path: &Path) -> CoreResult<(u64, u64)> {
        use std::os::unix::ffi::OsStrExt;
        let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
            .map_err(|_| SentinelError::invalid("path contains a NUL byte"))?;
        // SAFETY: zeroed statvfs is a valid out buffer.
        let mut stats: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: c_path is NUL-terminated and stats is writable.
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stats) };
        if rc != 0 {
            return Err(SentinelError::io(
                &std::io::Error::last_os_error(),
                Some(path),
            ));
        }
        let frsize = stats.f_frsize as u64;
        Ok((
            (stats.f_blocks as u64).saturating_mul(frsize),
            (stats.f_bavail as u64).saturating_mul(frsize),
        ))
    }
}

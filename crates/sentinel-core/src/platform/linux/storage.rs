use std::path::Path;

use crate::error::CoreResult;
use crate::model::{CleanupCategory, CleanupLocation, EntryMetadata, VolumeInfo};
use crate::platform::storage_common::{
    common_developer_caches, downloads_location, home_join, location, sysinfo_volumes, unix,
};
use crate::provider::StorageProvider;

pub(crate) struct LinuxStorage;

/// Kernel pseudo filesystems: sizes are synthetic and some files block or never end.
const SKIP: [&str; 5] = ["/proc", "/sys", "/dev", "/run", "/var/lib/docker/overlay2"];

impl StorageProvider for LinuxStorage {
    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>> {
        Ok(sysinfo_volumes(
            |mount, fs| {
                let mount = mount.to_string_lossy();
                !(mount.starts_with("/snap/")
                    || mount.starts_with("/proc")
                    || mount.starts_with("/sys")
                    || mount.starts_with("/run/")
                    || matches!(fs, "squashfs" | "overlay" | "tmpfs" | "devtmpfs"))
            },
            |mount, device| {
                if mount == Path::new("/") {
                    "System".to_owned()
                } else {
                    mount
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| device.to_owned())
                }
            },
            |mount| mount == Path::new("/"),
        ))
    }

    fn metadata(&self, path: &Path) -> CoreResult<EntryMetadata> {
        unix::lstat(path)
    }

    fn should_skip(&self, path: &Path) -> bool {
        SKIP.iter().any(|skip| path == Path::new(skip))
    }

    fn cleanup_locations(&self) -> Vec<CleanupLocation> {
        let mut out = Vec::new();
        let home: [(CleanupCategory, &str, &'static str); 5] = [
            (
                CleanupCategory::AppCache,
                ".cache",
                "Per-user application caches (XDG cache directory); apps rebuild them as needed.",
            ),
            (
                CleanupCategory::Trash,
                ".local/share/Trash",
                "Items already in the Trash. Emptying it is permanent, so Sentinel only reports it.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                ".cache/pip",
                "pip's wheel cache; packages are downloaded again when needed.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                ".local/share/pnpm/store",
                "pnpm's content-addressed store; refilled on the next install.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                ".cache/yarn",
                "Yarn's package cache; refilled on the next install.",
            ),
        ];
        for (category, relative, rationale) in home {
            if let Some(path) = home_join(relative) {
                out.push(location(category, path, rationale));
            }
        }
        out.push(location(
            CleanupCategory::Logs,
            "/var/log",
            "System logs; rotated automatically, only needed when troubleshooting.",
        ));
        out.push(location(
            CleanupCategory::PackageManagerCache,
            "/var/cache/apt/archives",
            "Downloaded .deb packages kept after installation; apt fetches them again if needed.",
        ));
        out.extend(common_developer_caches());
        out.extend(downloads_location());
        out
    }

    fn volume_usage(&self, path: &Path) -> CoreResult<(u64, u64)> {
        unix::statvfs(path)
    }
}

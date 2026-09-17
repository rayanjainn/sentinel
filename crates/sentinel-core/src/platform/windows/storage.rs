use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use parking_lot::Mutex;
use windows::Win32::Storage::FileSystem::{
    GetCompressedFileSizeW, GetDiskFreeSpaceExW, GetDiskFreeSpaceW, INVALID_FILE_SIZE,
};
use windows::core::PCWSTR;

use crate::error::{CoreResult, SentinelError};
use crate::model::{CleanupCategory, CleanupLocation, EntryKind, EntryMetadata, VolumeInfo};
use crate::platform::storage_common::{
    common_developer_caches, downloads_location, location, sysinfo_volumes,
};
use crate::provider::StorageProvider;
use crate::util::system_time_secs;

const FILE_ATTRIBUTE_SPARSE_FILE: u32 = 0x200;
const FILE_ATTRIBUTE_COMPRESSED: u32 = 0x800;
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x40_0000;

#[derive(Default)]
pub(crate) struct WindowsStorage {
    cluster_sizes: Mutex<HashMap<String, u64>>,
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Drive or UNC share prefix ("C:", "\\server\share"), used as the device identity.
fn prefix(path: &Path) -> String {
    match path.components().next() {
        Some(Component::Prefix(p)) => p.as_os_str().to_string_lossy().to_ascii_uppercase(),
        _ => String::new(),
    }
}

fn device_id(prefix: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    prefix.hash(&mut hasher);
    hasher.finish()
}

fn env_path(var: &str, relative: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .filter(|v| !v.is_empty())
        .map(|base| PathBuf::from(base).join(relative))
}

impl WindowsStorage {
    fn cluster_size(&self, path: &Path) -> u64 {
        let key = prefix(path);
        if let Some(size) = self.cluster_sizes.lock().get(&key) {
            return *size;
        }
        let root = PathBuf::from(format!("{key}\\"));
        let root_w = wide(&root);
        let mut sectors = 0u32;
        let mut bytes = 0u32;
        // SAFETY: root_w is NUL-terminated; out pointers are valid.
        let ok = unsafe {
            GetDiskFreeSpaceW(
                PCWSTR(root_w.as_ptr()),
                Some(&mut sectors),
                Some(&mut bytes),
                None,
                None,
            )
        }
        .is_ok();
        let size = if ok && sectors > 0 && bytes > 0 {
            u64::from(sectors) * u64::from(bytes)
        } else {
            4096
        };
        self.cluster_sizes.lock().insert(key, size);
        size
    }

    fn convert(&self, path: &Path, meta: &std::fs::Metadata) -> EntryMetadata {
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
        let attributes = meta.file_attributes();
        let apparent = meta.len();
        let allocated = if kind != EntryKind::File {
            0
        } else if attributes & (FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS | FILE_ATTRIBUTE_OFFLINE) != 0
        {
            // Cloud placeholders (OneDrive "online-only") occupy no local space.
            0
        } else if attributes & (FILE_ATTRIBUTE_COMPRESSED | FILE_ATTRIBUTE_SPARSE_FILE) != 0 {
            compressed_size(path).unwrap_or(apparent)
        } else {
            let cluster = self.cluster_size(path);
            apparent.div_ceil(cluster) * cluster
        };
        let secs = |time: std::io::Result<SystemTime>| time.ok().and_then(system_time_secs);
        EntryMetadata {
            kind,
            allocated_bytes: allocated,
            apparent_bytes: apparent,
            modified: secs(meta.modified()),
            accessed: secs(meta.accessed()),
            device: device_id(&prefix(path)),
            // Hard link identity needs a file handle per entry, which would make scans far slower;
            // NTFS hard links are rare outside WinSxS, which is skipped.
            file_id: 0,
            hard_links: 1,
        }
    }
}

fn compressed_size(path: &Path) -> Option<u64> {
    let path_w = wide(path);
    let mut high = 0u32;
    // SAFETY: NUL-terminated path; out pointer is valid.
    let low = unsafe { GetCompressedFileSizeW(PCWSTR(path_w.as_ptr()), Some(&mut high)) };
    if low == INVALID_FILE_SIZE && std::io::Error::last_os_error().raw_os_error().unwrap_or(0) != 0
    {
        return None;
    }
    Some((u64::from(high) << 32) | u64::from(low))
}

impl StorageProvider for WindowsStorage {
    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>> {
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_owned());
        Ok(sysinfo_volumes(
            |_, _| true,
            |mount, device| {
                if device.is_empty() {
                    format!("Local Disk ({})", prefix(mount))
                } else {
                    format!("{device} ({})", prefix(mount))
                }
            },
            |mount| prefix(mount).eq_ignore_ascii_case(&system_drive),
        ))
    }

    fn metadata(&self, path: &Path) -> CoreResult<EntryMetadata> {
        let meta =
            std::fs::symlink_metadata(path).map_err(|err| SentinelError::io(&err, Some(path)))?;
        Ok(self.convert(path, &meta))
    }

    fn dir_entry_metadata(&self, entry: &std::fs::DirEntry) -> CoreResult<EntryMetadata> {
        let path = entry.path();
        // DirEntry metadata comes from the FindNextFile data already read; no extra open.
        let meta = entry
            .metadata()
            .map_err(|err| SentinelError::io(&err, Some(&path)))?;
        Ok(self.convert(&path, &meta))
    }

    fn should_skip(&self, path: &Path) -> bool {
        let text = path.to_string_lossy().to_ascii_lowercase();
        text.ends_with(r"\system volume information")
            || text.ends_with(r"\windows\winsxs")
            || text.ends_with(r"\pagefile.sys")
            || text.ends_with(r"\hiberfil.sys")
    }

    fn cleanup_locations(&self) -> Vec<CleanupLocation> {
        let mut out = Vec::new();
        let entries: [(CleanupCategory, &str, &str, &'static str); 9] = [
            (
                CleanupCategory::AppCache,
                "LOCALAPPDATA",
                "Temp",
                "Temporary files left by installers and apps; safe once the apps that wrote them are closed.",
            ),
            (
                CleanupCategory::AppCache,
                "LOCALAPPDATA",
                r"Microsoft\Windows\INetCache",
                "Web and app download cache; rebuilt automatically.",
            ),
            (
                CleanupCategory::Logs,
                "LOCALAPPDATA",
                "CrashDumps",
                "Crash dumps written when apps failed; only useful for debugging those crashes.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "LOCALAPPDATA",
                "npm-cache",
                "npm's download cache; packages are fetched again when a project installs them.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "APPDATA",
                "npm-cache",
                "npm's download cache; packages are fetched again when a project installs them.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "LOCALAPPDATA",
                r"pip\Cache",
                "pip's wheel cache; packages are downloaded again when needed.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "LOCALAPPDATA",
                r"Yarn\Cache",
                "Yarn's package cache; refilled on the next install.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "LOCALAPPDATA",
                r"pnpm\store",
                "pnpm's content-addressed store; refilled on the next install.",
            ),
            (
                CleanupCategory::Logs,
                "SystemRoot",
                "Logs",
                "Windows diagnostic logs; only useful when troubleshooting.",
            ),
        ];
        for (category, var, relative, rationale) in entries {
            if let Some(path) = env_path(var, relative) {
                out.push(location(category, path, rationale));
            }
        }
        if let Ok(drive) = std::env::var("SystemDrive") {
            out.push(location(
                CleanupCategory::Trash,
                format!("{drive}\\$Recycle.Bin"),
                "Items already in the Recycle Bin. Emptying it is permanent, so Sentinel only reports it.",
            ));
        }
        out.extend(common_developer_caches());
        out.extend(downloads_location());
        out
    }

    fn volume_usage(&self, path: &Path) -> CoreResult<(u64, u64)> {
        let path_w = wide(path);
        let mut available = 0u64;
        let mut total = 0u64;
        // SAFETY: NUL-terminated path; out pointers are valid.
        unsafe {
            GetDiskFreeSpaceExW(
                PCWSTR(path_w.as_ptr()),
                Some(&mut available),
                Some(&mut total),
                None,
            )
        }
        .map_err(|err| SentinelError::Io {
            detail: format!("could not read free space: {}", err.message()),
            path: Some(path.to_string_lossy().into_owned()),
        })?;
        Ok((total, available))
    }
}

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use crate::error::CoreResult;
use crate::model::{CleanupCategory, CleanupLocation, EntryMetadata, VolumeInfo};
use crate::platform::storage_common::{
    common_developer_caches, downloads_location, home_join, location, sysinfo_volumes, unix,
};
use crate::provider::StorageProvider;

const DATA_VOLUME: &str = "/System/Volumes/Data";

pub(crate) struct MacStorage;

/// Paths the scanner must not descend into: the firmlinked data volume mirror (its contents are
/// already reached through `/Users`, `/Applications`, … when scanning `/`) and automount points.
const SKIP: [&str; 6] = [
    DATA_VOLUME,
    "/dev",
    "/net",
    "/home",
    "/System/Volumes/Data/home",
    "/Volumes/Macintosh HD",
];

impl StorageProvider for MacStorage {
    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>> {
        Ok(sysinfo_volumes(
            |mount, fs| {
                let mount = mount.to_string_lossy();
                !(mount.starts_with("/System/Volumes/")
                    || mount.starts_with("/Library/Developer/CoreSimulator/")
                    || mount.starts_with("/private/var/")
                    || fs == "devfs"
                    || fs == "autofs")
            },
            |mount, device| volume_name(mount).unwrap_or_else(|| device.to_owned()),
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
        let home: [(CleanupCategory, &str, &'static str); 11] = [
            (
                CleanupCategory::AppCache,
                "Library/Caches",
                "Application caches are rebuilt automatically when the app needs them again.",
            ),
            (
                CleanupCategory::Logs,
                "Library/Logs",
                "Diagnostic logs written by apps; only useful when troubleshooting.",
            ),
            (
                CleanupCategory::Trash,
                ".Trash",
                "Items already in the Trash. Emptying it is permanent, so Sentinel only reports it.",
            ),
            (
                CleanupCategory::DeveloperCache,
                "Library/Developer/Xcode/DerivedData",
                "Xcode build products and indexes; regenerated on the next build.",
            ),
            (
                CleanupCategory::DeveloperCache,
                "Library/Developer/CoreSimulator/Caches",
                "iOS Simulator caches; recreated when simulators run.",
            ),
            (
                CleanupCategory::DeveloperCache,
                "Library/Developer/Xcode/iOS DeviceSupport",
                "Debug symbols copied from connected devices; copied again when a device connects.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "Library/Caches/Homebrew",
                "Downloaded Homebrew bottles; fetched again when a formula is reinstalled.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "Library/Caches/pip",
                "pip's wheel cache; packages are downloaded again when needed.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "Library/Caches/Yarn",
                "Yarn's package cache; refilled on the next install.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "Library/pnpm/store",
                "pnpm's content-addressed store; refilled on the next install.",
            ),
            (
                CleanupCategory::PackageManagerCache,
                "Library/Caches/CocoaPods",
                "CocoaPods download cache; pods are fetched again on the next install.",
            ),
        ];
        for (category, relative, rationale) in home {
            if let Some(path) = home_join(relative) {
                out.push(location(category, path, rationale));
            }
        }
        out.push(location(
            CleanupCategory::Logs,
            "/Library/Logs",
            "System-wide diagnostic logs; only useful when troubleshooting.",
        ));
        out.extend(common_developer_caches());
        out.extend(downloads_location());
        out
    }

    fn volume_usage(&self, path: &Path) -> CoreResult<(u64, u64)> {
        unix::statvfs(path)
    }

    fn volume_group(&self, root_device: u64) -> Vec<u64> {
        let system = std::fs::symlink_metadata("/").map(|m| m.dev()).ok();
        let data = std::fs::symlink_metadata(DATA_VOLUME).map(|m| m.dev()).ok();
        match (system, data) {
            (Some(system), Some(data)) if root_device == system || root_device == data => {
                vec![system, data]
            }
            _ => vec![root_device],
        }
    }
}

/// Volume label ("Macintosh HD") via `getattrlist(ATTR_VOL_NAME)`.
fn volume_name(mount: &Path) -> Option<String> {
    const ATTR_VOL_INFO: u32 = 0x8000_0000;
    let c_path = CString::new(mount.as_os_str().as_bytes()).ok()?;
    let mut request = libc::attrlist {
        bitmapcount: libc::ATTR_BIT_MAP_COUNT,
        reserved: 0,
        commonattr: 0,
        volattr: ATTR_VOL_INFO | libc::ATTR_VOL_NAME,
        dirattr: 0,
        fileattr: 0,
        forkattr: 0,
    };
    let mut buffer = [0u8; 4 + 8 + libc::PATH_MAX as usize];
    // SAFETY: request and buffer are valid for the sizes passed.
    let rc = unsafe {
        libc::getattrlist(
            c_path.as_ptr(),
            (&mut request as *mut libc::attrlist).cast(),
            buffer.as_mut_ptr().cast(),
            buffer.len(),
            0,
        )
    };
    if rc != 0 {
        return None;
    }
    // Layout: u32 total length, then attrreference_t { i32 offset, u32 length } relative to itself.
    let offset = i32::from_ne_bytes(buffer[4..8].try_into().ok()?);
    let length = u32::from_ne_bytes(buffer[8..12].try_into().ok()?) as usize;
    let start = usize::try_from(4 + offset).ok()?;
    let bytes = buffer.get(start..start + length)?;
    let name = String::from_utf8_lossy(bytes)
        .trim_end_matches('\0')
        .to_owned();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_volume_has_a_label_and_group() {
        let volumes = MacStorage.volumes().unwrap();
        let root = volumes
            .iter()
            .find(|v| v.mount_point == "/")
            .expect("root volume");
        assert!(root.is_system);
        assert!(!root.name.is_empty());
        assert!(root.total_bytes >= root.available_bytes);
        let dev = std::fs::symlink_metadata("/").unwrap().dev();
        assert_eq!(MacStorage.volume_group(dev).len(), 2);
        assert!(MacStorage.should_skip(Path::new(DATA_VOLUME)));
        let (total, available) = MacStorage.volume_usage(Path::new("/")).unwrap();
        assert!(total > 0 && available <= total);
    }
}

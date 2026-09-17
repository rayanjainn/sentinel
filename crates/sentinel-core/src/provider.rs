//! Per-OS provider traits — the cross-platform backbone.
//!
//! Each OS module (`platform::macos`, `platform::linux`, `platform::windows`) implements every
//! trait; `platform::current()` is selected with `cfg(target_os)`. Read traits take `&mut self`
//! because they hold refresh state (previous counters for rate computation). Control traits are
//! stateless `&self` so actions never wait on a sampling pass.
//!
//! OS-agnostic logic (enrichment, rate math, the parallel scanner, duplicate hashing, action
//! previews, audit) lives in the service layer and only talks to these traits.

use std::path::{Path, PathBuf};

use crate::error::CoreResult;
use crate::model::*;

pub trait ResourceProvider: Send {
    fn system_info(&self) -> SystemInfo;
    /// CPU values are deltas since the previous call; the first call primes counters.
    fn sample(&mut self) -> CoreResult<ResourceSample>;
}

pub trait ProcessProvider: Send {
    /// Refresh and return every process visible to the current user.
    fn refresh(&mut self) -> CoreResult<ProcessSnapshot>;
    /// Fresh single-process lookup (used by action previews).
    fn lookup(&mut self, pid: Pid) -> CoreResult<ProcessInfo>;
    fn open_files(&self, pid: Pid) -> CoreResult<Vec<OpenFile>>;
}

pub trait ProcessControl: Send + Sync {
    /// Current identity of `pid`; `ProcessNotFound` if gone.
    fn identity(&self, pid: Pid) -> CoreResult<ProcessIdentity>;
    fn has_window(&self, pid: Pid) -> bool;
    /// Implementations must verify `target.start_time` immediately before signalling and return
    /// `ProcessChanged` on mismatch.
    fn terminate(&self, target: &ProcessIdentity) -> CoreResult<TerminateMethod>;
    fn force_kill(&self, target: &ProcessIdentity) -> CoreResult<()>;
    fn set_priority(&self, target: &ProcessIdentity, nice: i32) -> CoreResult<()>;
}

pub trait NetworkProvider: Send {
    /// macOS: libproc socket fdinfo · Linux: /proc/net/{tcp,tcp6,udp,udp6} + /proc/*/fd inode map ·
    /// Windows: GetExtendedTcpTable / GetExtendedUdpTable.
    fn sockets(&mut self) -> CoreResult<Vec<RawSocket>>;
    /// Cumulative bytes across physical interfaces (loopback excluded).
    fn interface_counters(&mut self) -> CoreResult<(u64, u64)>;
    /// Best traffic granularity available; `InterfaceOnly` with empty vectors is valid.
    fn traffic(&mut self) -> CoreResult<TrafficReport>;
}

pub trait StorageProvider: Send + Sync {
    fn volumes(&self) -> CoreResult<Vec<VolumeInfo>>;
    /// lstat-equivalent: never follows symlinks.
    fn metadata(&self, path: &Path) -> CoreResult<EntryMetadata>;
    /// Pseudo filesystems and firmlink duplicates the scanner must not descend into
    /// (/proc, /sys, /System/Volumes/Data mirror, etc.).
    fn should_skip(&self, path: &Path) -> bool;
    fn cleanup_locations(&self) -> Vec<CleanupLocation>;
    /// (total, available) bytes of the volume holding `path` (statvfs / GetDiskFreeSpaceEx).
    fn volume_usage(&self, path: &Path) -> CoreResult<(u64, u64)>;
    /// Metadata for an entry produced by `read_dir`; platforms whose directory enumeration already
    /// carries metadata (Windows) avoid a second lookup.
    fn dir_entry_metadata(&self, entry: &std::fs::DirEntry) -> CoreResult<EntryMetadata> {
        self.metadata(&entry.path())
    }
    /// Devices that belong to the same logical volume as `root_device`, so a scan does not treat
    /// them as mount crossings (macOS firmlinks join the sealed system and data volumes).
    fn volume_group(&self, root_device: u64) -> Vec<u64> {
        vec![root_device]
    }
}

pub trait FileOps: Send + Sync {
    /// Moves to the OS trash / recycle bin. Implementations must never fall back to unlinking.
    fn trash(&self, path: &Path) -> CoreResult<()>;
    /// Moves `path` into `destination_dir`, falling back to copy-then-trash-source across volumes.
    /// Returns the new location.
    fn move_into(&self, path: &Path, destination_dir: &Path) -> CoreResult<PathBuf>;
    /// Finder `open -R` / `explorer /select,` / FileManager1 D-Bus ShowItems.
    fn reveal(&self, path: &Path) -> CoreResult<()>;
}

pub trait FirewallProvider: Send + Sync {
    fn status(&self) -> FirewallStatus;
    /// Install one Sentinel-owned rule. May prompt for elevation (`ElevationDeclined` if refused).
    fn install(&self, rule: &FirewallRule) -> CoreResult<()>;
    fn remove(&self, rule: &FirewallRule) -> CoreResult<()>;
    /// Ids of Sentinel rules currently loaded in the OS firewall, to reconcile `FirewallRule::active`.
    fn installed_rule_ids(&self) -> CoreResult<Vec<String>>;
}

pub trait PermissionProbe: Send + Sync {
    fn status(&self) -> PermissionStatus;
    /// Opens the relevant OS settings pane (e.g. Privacy & Security › Full Disk Access).
    fn open_settings(&self, kind: PermissionKind) -> CoreResult<()>;
}

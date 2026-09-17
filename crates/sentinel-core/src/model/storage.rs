use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{TimestampMs, TimestampSecs};

pub type ScanId = String;
pub type NodeId = u32;
pub type JobId = String;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VolumeInfo {
    pub mount_point: String,
    pub name: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub is_removable: bool,
    /// The volume holding the OS / user home.
    pub is_system: bool,
}

/// Per-entry metadata the scanner needs and that differs per OS (allocated size, hardlink identity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryMetadata {
    pub kind: EntryKind,
    /// On-disk allocation (st_blocks * 512 / GetCompressedFileSize), what actually frees space.
    pub allocated_bytes: u64,
    pub apparent_bytes: u64,
    pub modified: Option<TimestampSecs>,
    pub accessed: Option<TimestampSecs>,
    /// (device, inode / file index) — used to count hardlinked files once and to detect mount crossings.
    pub device: u64,
    pub file_id: u64,
    pub hard_links: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Directory,
    Symlink,
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanRequest {
    pub root: String,
    /// Descend into other mounted volumes under `root`.
    pub cross_mounts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NodeKind {
    Directory,
    File,
    Symlink,
    Other,
    /// Files below the retention threshold inside one directory, collapsed to bound memory.
    SmallFiles,
    /// Children beyond `max_children` in a tree query, summed.
    Remainder,
}

/// Coarse file type used for treemap colouring and the by-type breakdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FileKindGroup {
    Image,
    Video,
    Audio,
    Document,
    Archive,
    Code,
    Application,
    DiskImage,
    Data,
    System,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CleanupCategory {
    AppCache,
    Logs,
    /// Informational only: emptying the trash is permanent, which Sentinel never does.
    Trash,
    OldDownloads,
    BuildArtifacts,
    PackageManagerCache,
    DeveloperCache,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TreeNode {
    pub id: NodeId,
    pub name: String,
    pub path: String,
    pub kind: NodeKind,
    pub size_bytes: u64,
    /// Files beneath this node (1 for a file).
    pub item_count: u64,
    pub modified: Option<TimestampSecs>,
    pub accessed: Option<TimestampSecs>,
    pub extension: Option<String>,
    pub file_kind: Option<FileKindGroup>,
    pub category: Option<CleanupCategory>,
    /// Entries under this directory that could not be read (permission denied, vanished mid-scan).
    pub unreadable_entries: u32,
    pub has_children: bool,
    /// `None` when not loaded (beyond the requested depth); sorted by size descending.
    pub children: Option<Vec<TreeNode>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TreeQuery {
    pub scan_id: ScanId,
    /// `None` = scan root.
    pub node_id: Option<NodeId>,
    pub depth: u8,
    pub max_children: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScanPhase {
    Walking,
    Summarizing,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanProgress {
    pub scan_id: ScanId,
    pub phase: ScanPhase,
    pub files_scanned: u64,
    pub dirs_scanned: u64,
    pub bytes_scanned: u64,
    pub unreadable_entries: u64,
    pub current_path: Option<String>,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

/// Streamed while walking: the top of the tree with sizes discovered so far.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanPartial {
    pub scan_id: ScanId,
    pub root: TreeNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileEntry {
    pub node_id: NodeId,
    pub path: String,
    pub name: String,
    pub size_bytes: u64,
    pub modified: Option<TimestampSecs>,
    pub accessed: Option<TimestampSecs>,
    pub extension: Option<String>,
    pub file_kind: FileKindGroup,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExtensionStat {
    /// Lower-cased, without dot; `None` for files without an extension.
    pub extension: Option<String>,
    pub file_kind: FileKindGroup,
    pub bytes: u64,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CleanupSuggestion {
    pub id: String,
    pub category: CleanupCategory,
    pub path: String,
    pub size_bytes: u64,
    pub item_count: u64,
    pub last_modified: Option<TimestampSecs>,
    pub last_accessed: Option<TimestampSecs>,
    /// Why this is usually safe to remove and what regenerates it. Always a suggestion.
    pub rationale: String,
    /// False for categories Sentinel will not act on (e.g. Trash, whose removal is permanent).
    pub actionable: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScanSummary {
    pub scan_id: ScanId,
    pub root: String,
    pub started_at_ms: TimestampMs,
    pub duration_ms: u64,
    pub total_bytes: u64,
    pub file_count: u64,
    pub dir_count: u64,
    pub unreadable_entries: u64,
    /// First few unreadable directories, to point the user at Full Disk Access etc.
    pub unreadable_samples: Vec<String>,
    pub by_extension: Vec<ExtensionStat>,
    /// Top 100 by size.
    pub largest_files: Vec<FileEntry>,
    pub suggestions: Vec<CleanupSuggestion>,
}

/// OS-specific well-known location feeding cleanup suggestions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupLocation {
    pub category: CleanupCategory,
    pub path: std::path::PathBuf,
    pub rationale: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DuplicatePhase {
    GroupingBySize,
    HashingHeads,
    HashingFull,
    Complete,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateProgress {
    pub job_id: JobId,
    pub phase: DuplicatePhase,
    pub candidates: u64,
    pub hashed: u64,
    pub bytes_hashed: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateGroup {
    /// BLAKE3 of full contents, hex.
    pub hash: String,
    pub size_bytes: u64,
    pub files: Vec<FileEntry>,
    /// size * (copies - 1).
    pub reclaimable_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DuplicateReport {
    pub job_id: JobId,
    pub scan_id: ScanId,
    pub groups: Vec<DuplicateGroup>,
    pub reclaimable_bytes: u64,
    pub duration_ms: u64,
}

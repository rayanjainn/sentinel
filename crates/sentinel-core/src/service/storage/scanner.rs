//! Parallel directory walker built on `StorageProvider`.
//!
//! Directories fan out on a dedicated rayon pool. Files under 64 KiB are folded into one
//! `SmallFiles` aggregate per directory to bound memory; allocated sizes are summed; hard-linked
//! files count once; symlinks are never followed; other devices are not entered unless requested.
//! Unreadable or vanishing entries are counted on their directory and never abort the walk.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use parking_lot::Mutex;
use rayon::prelude::*;

use super::classify::extension;
use super::tree::{BuiltDir, BuiltFile, SmallFiles, newer};
use crate::error::SentinelError;
use crate::model::{CleanupCategory, EntryKind, EntryMetadata, NodeKind, TreeNode};
use crate::provider::StorageProvider;

pub const SMALL_FILE_BYTES: u64 = 64 * 1024;
const UNREADABLE_SAMPLES: usize = 20;
const HARDLINK_SHARDS: usize = 16;

/// Directory names that are rebuildable output when a sibling project marker exists.
const BUILD_ARTIFACTS: [(&str, &[&str]); 9] = [
    ("node_modules", &["package.json"]),
    ("target", &["Cargo.toml"]),
    (".next", &["package.json"]),
    ("dist", &["package.json"]),
    (
        "build",
        &[
            "package.json",
            "build.gradle",
            "build.gradle.kts",
            "CMakeLists.txt",
            "pyproject.toml",
            "setup.py",
        ],
    ),
    (
        ".gradle",
        &[
            "build.gradle",
            "build.gradle.kts",
            "settings.gradle",
            "settings.gradle.kts",
        ],
    ),
    (".venv", &["pyproject.toml", "requirements.txt", "setup.py"]),
    ("Pods", &["Podfile"]),
    (".dart_tool", &["pubspec.yaml"]),
];

pub fn artifact_marker(dir: &Path) -> Option<&'static str> {
    let name = dir.file_name()?.to_str()?;
    let parent = dir.parent()?;
    BUILD_ARTIFACTS
        .iter()
        .find(|(artifact, _)| *artifact == name)
        .and_then(|(_, markers)| markers.iter().copied().find(|m| parent.join(m).exists()))
}

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub unreadable: AtomicU64,
    pub current: Mutex<Option<String>>,
    pub unreadable_samples: Mutex<Vec<String>>,
    pub extensions: Mutex<HashMap<Option<String>, (u64, u64)>>,
}

/// Depth ≤ 2 view whose sizes grow while the walk runs, for `sentinel:scan-partial`.
pub struct LiveNode {
    pub name: String,
    pub path: PathBuf,
    pub bytes: AtomicU64,
    pub items: AtomicU64,
    pub children: Mutex<Vec<Arc<LiveNode>>>,
}

impl LiveNode {
    pub fn new(name: String, path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            name,
            path,
            bytes: AtomicU64::new(0),
            items: AtomicU64::new(0),
            children: Mutex::new(Vec::new()),
        })
    }

    pub fn snapshot(&self, depth: u8, next_id: &mut u32) -> TreeNode {
        let id = *next_id;
        *next_id += 1;
        let children: Vec<Arc<LiveNode>> = self.children.lock().clone();
        let has_children = !children.is_empty();
        let children = (depth > 0 && has_children).then(|| {
            let mut nodes: Vec<TreeNode> = children
                .iter()
                .map(|c| c.snapshot(depth - 1, next_id))
                .collect();
            nodes.sort_by(|a, b| b.size_bytes.cmp(&a.size_bytes));
            nodes
        });
        TreeNode {
            id,
            name: self.name.clone(),
            path: self.path.to_string_lossy().into_owned(),
            kind: NodeKind::Directory,
            size_bytes: self.bytes.load(Ordering::Relaxed),
            item_count: self.items.load(Ordering::Relaxed),
            modified: None,
            accessed: None,
            extension: None,
            file_kind: None,
            category: None,
            unreadable_entries: 0,
            has_children,
            children,
        }
    }
}

pub struct Walker<'a> {
    pub provider: &'a dyn StorageProvider,
    pub devices: Vec<u64>,
    pub cross_mounts: bool,
    pub cancel: &'a AtomicBool,
    pub progress: &'a Progress,
    hardlinks: Vec<Mutex<HashSet<(u64, u64)>>>,
}

impl<'a> Walker<'a> {
    pub fn new(
        provider: &'a dyn StorageProvider,
        root_device: u64,
        cross_mounts: bool,
        cancel: &'a AtomicBool,
        progress: &'a Progress,
    ) -> Self {
        Self {
            provider,
            devices: provider.volume_group(root_device),
            cross_mounts,
            cancel,
            progress,
            hardlinks: (0..HARDLINK_SHARDS)
                .map(|_| Mutex::new(HashSet::new()))
                .collect(),
        }
    }

    fn first_link(&self, meta: &EntryMetadata) -> bool {
        if meta.hard_links <= 1 || meta.file_id == 0 {
            return true;
        }
        let shard = (meta.file_id as usize) % HARDLINK_SHARDS;
        self.hardlinks[shard]
            .lock()
            .insert((meta.device, meta.file_id))
    }

    fn note_unreadable(&self, path: &Path) {
        self.progress.unreadable.fetch_add(1, Ordering::Relaxed);
        let mut samples = self.progress.unreadable_samples.lock();
        if samples.len() < UNREADABLE_SAMPLES {
            samples.push(path.to_string_lossy().into_owned());
        }
    }

    /// Walks `path`. `live` carries the root, depth-1 and depth-2 live nodes on this branch.
    pub fn walk(
        &self,
        path: &Path,
        name: String,
        depth: u32,
        inside_artifact: bool,
        live: &[Arc<LiveNode>],
    ) -> BuiltDir {
        let mut dir = BuiltDir {
            name,
            ..Default::default()
        };
        if self.cancel.load(Ordering::Relaxed) {
            return dir;
        }
        if !inside_artifact && artifact_marker(path).is_some() {
            dir.category = Some(CleanupCategory::BuildArtifacts);
        }
        let inside_artifact = inside_artifact || dir.category.is_some();

        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(_) => {
                // The directory exists but its contents cannot be listed (permissions, TCC,
                // or it vanished): count it and keep going.
                dir.unreadable = 1;
                self.note_unreadable(path);
                return dir;
            }
        };
        let mut subdirs: Vec<(PathBuf, String)> = Vec::new();
        let mut small = SmallFiles::default();
        let mut local_ext: HashMap<Option<String>, (u64, u64)> = HashMap::new();
        let mut local_bytes = 0u64;
        let mut local_items = 0u64;
        for entry in entries {
            let Ok(entry) = entry else {
                dir.unreadable += 1;
                self.note_unreadable(path);
                continue;
            };
            let child_path = entry.path();
            let meta = match self.provider.dir_entry_metadata(&entry) {
                Ok(meta) => meta,
                Err(SentinelError::PathNotFound { .. }) => continue,
                Err(_) => {
                    dir.unreadable += 1;
                    self.note_unreadable(&child_path);
                    continue;
                }
            };
            let child_name = entry.file_name().to_string_lossy().into_owned();
            match meta.kind {
                EntryKind::Directory => {
                    if self.provider.should_skip(&child_path)
                        || (!self.cross_mounts && !self.devices.contains(&meta.device))
                    {
                        continue;
                    }
                    subdirs.push((child_path, child_name));
                }
                EntryKind::File => {
                    let size = if self.first_link(&meta) {
                        meta.allocated_bytes
                    } else {
                        0
                    };
                    local_bytes += size;
                    local_items += 1;
                    let ext_stat = local_ext.entry(extension(&child_name)).or_default();
                    ext_stat.0 += size;
                    ext_stat.1 += 1;
                    if size >= SMALL_FILE_BYTES {
                        dir.files.push(BuiltFile {
                            name: child_name,
                            size,
                            modified: meta.modified,
                            accessed: meta.accessed,
                        });
                    } else {
                        small.size += size;
                        small.count += 1;
                        small.modified = newer(small.modified, meta.modified);
                        small.accessed = newer(small.accessed, meta.accessed);
                    }
                    dir.modified = newer(dir.modified, meta.modified);
                    dir.accessed = newer(dir.accessed, meta.accessed);
                }
                // Links and special files occupy (tiny) space but are not files the user owns.
                EntryKind::Symlink | EntryKind::Other => {
                    local_bytes += meta.allocated_bytes;
                    small.size += meta.allocated_bytes;
                }
            }
        }

        self.progress.dirs.fetch_add(1, Ordering::Relaxed);
        self.progress
            .files
            .fetch_add(local_items, Ordering::Relaxed);
        self.progress
            .bytes
            .fetch_add(local_bytes, Ordering::Relaxed);
        if !local_ext.is_empty() {
            let mut global = self.progress.extensions.lock();
            for (ext, (bytes, count)) in local_ext {
                let stat = global.entry(ext).or_default();
                stat.0 += bytes;
                stat.1 += count;
            }
        }
        if depth <= 3
            && let Some(mut current) = self.progress.current.try_lock()
        {
            *current = Some(path.to_string_lossy().into_owned());
        }
        for node in live {
            node.bytes.fetch_add(local_bytes, Ordering::Relaxed);
            node.items.fetch_add(local_items, Ordering::Relaxed);
        }

        dir.size = local_bytes;
        dir.items = local_items;
        if small.count > 0 {
            dir.small = Some(small);
        }

        let children: Vec<BuiltDir> = subdirs
            .into_par_iter()
            .map(|(child_path, child_name)| {
                if depth < 2 {
                    let node = LiveNode::new(child_name.clone(), child_path.clone());
                    if let Some(parent) = live.last() {
                        parent.children.lock().push(Arc::clone(&node));
                    }
                    let mut chain = live.to_vec();
                    chain.push(node);
                    self.walk(&child_path, child_name, depth + 1, inside_artifact, &chain)
                } else {
                    self.walk(&child_path, child_name, depth + 1, inside_artifact, live)
                }
            })
            .collect();
        for child in &children {
            dir.size += child.size;
            dir.items += child.items;
            dir.unreadable = dir.unreadable.saturating_add(child.unreadable);
            dir.modified = newer(dir.modified, child.modified);
            dir.accessed = newer(dir.accessed, child.accessed);
        }
        dir.dirs = children;
        dir
    }
}

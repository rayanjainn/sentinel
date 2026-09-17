//! Scan summary: by-type totals, largest files and cleanup suggestions.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::classify::kind_for_extension;
use super::scanner::artifact_marker;
use super::tree::ScanTree;
use crate::model::{
    CleanupCategory, CleanupLocation, CleanupSuggestion, ExtensionStat, FileEntry, NodeId,
    NodeKind, TimestampSecs,
};
use crate::util::tilde_path;

pub const LARGEST_FILES: usize = 100;
const MAX_EXTENSIONS: usize = 400;
const MIN_SUGGESTION_BYTES: u64 = 1024 * 1024;
const PER_LOCATION_ITEMS: usize = 50;
const MAX_ARTIFACTS: usize = 100;
pub const OLD_DOWNLOAD_DAYS: u64 = 90;

pub fn by_extension(stats: HashMap<Option<String>, (u64, u64)>) -> Vec<ExtensionStat> {
    let mut out: Vec<ExtensionStat> = stats
        .into_iter()
        .map(|(extension, (bytes, count))| ExtensionStat {
            file_kind: kind_for_extension(extension.as_deref()),
            extension,
            bytes,
            count,
        })
        .collect();
    out.sort_by(|a, b| {
        b.bytes
            .cmp(&a.bytes)
            .then_with(|| a.extension.cmp(&b.extension))
    });
    out.truncate(MAX_EXTENSIONS);
    out
}

pub fn largest_files(tree: &ScanTree, limit: usize) -> Vec<FileEntry> {
    let mut files: Vec<(NodeId, u64)> = tree.files().map(|(id, n)| (id, n.size)).collect();
    if files.len() > limit {
        files.select_nth_unstable_by_key(limit, |f| Reverse(f.1));
        files.truncate(limit);
    }
    files.sort_by_key(|f| Reverse(f.1));
    files
        .into_iter()
        .filter_map(|(id, _)| tree.file_entry(id))
        .collect()
}

fn category_key(category: CleanupCategory) -> &'static str {
    match category {
        CleanupCategory::AppCache => "appCache",
        CleanupCategory::Logs => "logs",
        CleanupCategory::Trash => "trash",
        CleanupCategory::OldDownloads => "oldDownloads",
        CleanupCategory::BuildArtifacts => "buildArtifacts",
        CleanupCategory::PackageManagerCache => "packageManagerCache",
        CleanupCategory::DeveloperCache => "developerCache",
    }
}

fn suggestion(
    tree: &ScanTree,
    id: NodeId,
    category: CleanupCategory,
    rationale: String,
) -> CleanupSuggestion {
    let node = tree.node(id);
    let path = tree.path(id).to_string_lossy().into_owned();
    CleanupSuggestion {
        id: format!("{}:{path}", category_key(category)),
        category,
        size_bytes: node.map(|n| n.size).unwrap_or(0),
        item_count: node.map(|n| n.items).unwrap_or(0),
        last_modified: node.and_then(|n| n.modified),
        last_accessed: node.and_then(|n| n.accessed),
        rationale,
        actionable: category != CleanupCategory::Trash,
        path,
    }
}

pub fn suggestions(
    tree: &ScanTree,
    locations: &[CleanupLocation],
    now_secs: TimestampSecs,
) -> Vec<CleanupSuggestion> {
    let mut out = Vec::new();
    let located: HashMap<NodeId, &CleanupLocation> = locations
        .iter()
        .filter_map(|loc| tree.find(&loc.path).map(|id| (id, loc)))
        .collect();
    // A specific location (e.g. Homebrew's cache) wins over the generic parent listing.
    let specific: HashSet<NodeId> = located.keys().copied().collect();
    let mut covered: HashSet<NodeId> = HashSet::new();

    let mut ordered: Vec<(&NodeId, &&CleanupLocation)> = located.iter().collect();
    ordered.sort_by_key(|(id, _)| **id);
    for (&id, location) in ordered {
        let Some(node) = tree.node(id) else {
            continue;
        };
        match location.category {
            CleanupCategory::Trash => {
                if node.size > 0 {
                    out.push(suggestion(
                        tree,
                        id,
                        CleanupCategory::Trash,
                        location.rationale.to_owned(),
                    ));
                }
            }
            CleanupCategory::AppCache => {
                let mut children: Vec<(NodeId, u64)> = tree
                    .children(id)
                    .filter(|(child, n)| {
                        !specific.contains(child)
                            && n.kind != NodeKind::SmallFiles
                            && n.size >= MIN_SUGGESTION_BYTES
                    })
                    .map(|(child, n)| (child, n.size))
                    .collect();
                children.sort_by_key(|c| Reverse(c.1));
                for (child, _) in children.into_iter().take(PER_LOCATION_ITEMS) {
                    covered.insert(child);
                    out.push(suggestion(
                        tree,
                        child,
                        CleanupCategory::AppCache,
                        location.rationale.to_owned(),
                    ));
                }
            }
            CleanupCategory::OldDownloads => {
                let cutoff = now_secs.saturating_sub(OLD_DOWNLOAD_DAYS * 86_400);
                let mut old: Vec<(NodeId, u64)> = tree
                    .children(id)
                    .filter(|(_, n)| {
                        n.kind != NodeKind::SmallFiles
                            && n.size >= MIN_SUGGESTION_BYTES
                            && n.modified.max(n.accessed).is_some_and(|t| t < cutoff)
                    })
                    .map(|(child, n)| (child, n.size))
                    .collect();
                old.sort_by_key(|o| Reverse(o.1));
                for (child, _) in old.into_iter().take(PER_LOCATION_ITEMS) {
                    covered.insert(child);
                    out.push(suggestion(
                        tree,
                        child,
                        CleanupCategory::OldDownloads,
                        location.rationale.to_owned(),
                    ));
                }
            }
            category => {
                if node.size >= MIN_SUGGESTION_BYTES {
                    covered.insert(id);
                    out.push(suggestion(
                        tree,
                        id,
                        category,
                        location.rationale.to_owned(),
                    ));
                }
            }
        }
    }

    let mut artifacts: Vec<(NodeId, u64)> = tree
        .nodes_with_category()
        .filter(|(id, n)| {
            n.category == Some(CleanupCategory::BuildArtifacts)
                && n.size >= MIN_SUGGESTION_BYTES
                && !covered.iter().any(|c| tree.is_descendant(*id, *c))
        })
        .map(|(id, n)| (id, n.size))
        .collect();
    artifacts.sort_by_key(|a| Reverse(a.1));
    for (id, _) in artifacts.into_iter().take(MAX_ARTIFACTS) {
        let path = tree.path(id);
        out.push(suggestion(
            tree,
            id,
            CleanupCategory::BuildArtifacts,
            artifact_rationale(&path),
        ));
    }
    out.sort_by_key(|s| Reverse(s.size_bytes));
    out
}

fn artifact_rationale(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let project = path.parent().map(tilde_path).unwrap_or_default();
    let how = match artifact_marker(path) {
        Some("package.json") if name == "node_modules" => {
            "`npm install` (or your package manager) recreates it"
        }
        Some("package.json") => "the project's build script regenerates it",
        Some("Cargo.toml") => "`cargo build` regenerates it",
        Some("Podfile") => "`pod install` recreates it",
        Some("pubspec.yaml") => "`flutter pub get` recreates it",
        Some("pyproject.toml") | Some("requirements.txt") | Some("setup.py") if name == ".venv" => {
            "recreate the virtual environment and reinstall dependencies"
        }
        _ => "the next build regenerates it",
    };
    format!("Build output ({name}) for the project at {project}; {how}.")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::storage::tree::{BuiltDir, BuiltFile};
    use std::path::PathBuf;

    #[test]
    fn suggests_cache_children_downloads_and_trash() {
        let mb = 1024 * 1024;
        let cache_child = |name: &str, size| BuiltDir {
            name: name.into(),
            size,
            items: 1,
            ..Default::default()
        };
        let caches = BuiltDir {
            name: "Caches".into(),
            size: 5 * mb,
            items: 3,
            dirs: vec![
                cache_child("com.old.app", 3 * mb),
                cache_child("Homebrew", 2 * mb),
                cache_child("tiny", 10),
            ],
            ..Default::default()
        };
        let downloads = BuiltDir {
            name: "Downloads".into(),
            size: 6 * mb,
            items: 2,
            files: vec![
                BuiltFile {
                    name: "old.dmg".into(),
                    size: 4 * mb,
                    modified: Some(1_000),
                    accessed: Some(2_000),
                },
                BuiltFile {
                    name: "new.zip".into(),
                    size: 2 * mb,
                    modified: Some(10_000_000),
                    accessed: None,
                },
            ],
            ..Default::default()
        };
        let trash = BuiltDir {
            name: ".Trash".into(),
            size: 7 * mb,
            items: 1,
            ..Default::default()
        };
        let root = BuiltDir {
            name: "home".into(),
            size: 18 * mb,
            items: 6,
            dirs: vec![caches, downloads, trash],
            ..Default::default()
        };
        let tree = ScanTree::from_built(PathBuf::from("/home/u"), root);
        let locations = vec![
            CleanupLocation {
                category: CleanupCategory::AppCache,
                path: "/home/u/Caches".into(),
                rationale: "caches",
            },
            CleanupLocation {
                category: CleanupCategory::PackageManagerCache,
                path: "/home/u/Caches/Homebrew".into(),
                rationale: "brew",
            },
            CleanupLocation {
                category: CleanupCategory::OldDownloads,
                path: "/home/u/Downloads".into(),
                rationale: "old",
            },
            CleanupLocation {
                category: CleanupCategory::Trash,
                path: "/home/u/.Trash".into(),
                rationale: "trash",
            },
            CleanupLocation {
                category: CleanupCategory::Logs,
                path: "/nowhere".into(),
                rationale: "absent",
            },
        ];
        let now = 10_000_000 + 86_400 * 10;
        let found = suggestions(&tree, &locations, now);
        let paths: Vec<(&str, CleanupCategory, bool)> = found
            .iter()
            .map(|s| (s.path.as_str(), s.category, s.actionable))
            .collect();
        assert!(paths.contains(&("/home/u/.Trash", CleanupCategory::Trash, false)));
        assert!(paths.contains(&(
            "/home/u/Downloads/old.dmg",
            CleanupCategory::OldDownloads,
            true
        )));
        assert!(paths.contains(&(
            "/home/u/Caches/com.old.app",
            CleanupCategory::AppCache,
            true
        )));
        assert!(paths.contains(&(
            "/home/u/Caches/Homebrew",
            CleanupCategory::PackageManagerCache,
            true
        )));
        assert!(
            !found
                .iter()
                .any(|s| s.path.ends_with("new.zip") || s.path.ends_with("tiny"))
        );
        assert_eq!(found.len(), 4);
        assert_eq!(found[0].size_bytes, 7 * mb, "sorted by size");
    }
}

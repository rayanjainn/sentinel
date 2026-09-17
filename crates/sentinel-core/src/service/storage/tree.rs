//! Compact arena of a finished scan. Children of a node are stored contiguously and sorted by size,
//! so tree slices for the treemap are cheap and lazily materialised.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::model::{CleanupCategory, FileEntry, NodeId, NodeKind, TimestampSecs, TreeNode};
use crate::service::storage::classify::{extension, kind_for_extension};

pub const NO_PARENT: NodeId = NodeId::MAX;
/// Remainder nodes are synthetic; their id marks the parent they summarise.
pub const REMAINDER_FLAG: NodeId = 0x8000_0000;

#[derive(Debug, Clone)]
pub struct ArenaNode {
    pub name: Box<str>,
    pub parent: NodeId,
    pub first_child: NodeId,
    pub child_count: u32,
    pub kind: NodeKind,
    pub size: u64,
    pub items: u64,
    pub modified: Option<TimestampSecs>,
    pub accessed: Option<TimestampSecs>,
    pub category: Option<CleanupCategory>,
    pub unreadable: u32,
}

/// Owned tree produced by the walker, flattened into the arena once complete.
#[derive(Debug, Default)]
pub struct BuiltDir {
    pub name: String,
    pub size: u64,
    pub items: u64,
    pub modified: Option<TimestampSecs>,
    pub accessed: Option<TimestampSecs>,
    pub unreadable: u32,
    pub category: Option<CleanupCategory>,
    pub dirs: Vec<BuiltDir>,
    pub files: Vec<BuiltFile>,
    pub small: Option<SmallFiles>,
}

#[derive(Debug, Clone)]
pub struct BuiltFile {
    pub name: String,
    pub size: u64,
    pub modified: Option<TimestampSecs>,
    pub accessed: Option<TimestampSecs>,
}

#[derive(Debug, Clone, Default)]
pub struct SmallFiles {
    pub size: u64,
    pub count: u64,
    pub modified: Option<TimestampSecs>,
    pub accessed: Option<TimestampSecs>,
}

pub fn newer(a: Option<TimestampSecs>, b: Option<TimestampSecs>) -> Option<TimestampSecs> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (x, None) => x,
        (None, y) => y,
    }
}

#[derive(Debug)]
pub struct ScanTree {
    root_path: PathBuf,
    nodes: Vec<ArenaNode>,
}

enum Pending {
    Dir(BuiltDir),
    File(BuiltFile),
    Small(SmallFiles),
}

impl Pending {
    fn size(&self) -> u64 {
        match self {
            Pending::Dir(d) => d.size,
            Pending::File(f) => f.size,
            Pending::Small(s) => s.size,
        }
    }
}

impl ScanTree {
    pub fn from_built(root_path: PathBuf, root: BuiltDir) -> Self {
        let mut nodes = Vec::new();
        let mut queue: VecDeque<(NodeId, BuiltDir)> = VecDeque::new();
        nodes.push(dir_node(
            &root,
            NO_PARENT,
            root_display_name(&root_path, &root.name),
        ));
        queue.push_back((0, root));
        while let Some((id, dir)) = queue.pop_front() {
            let BuiltDir {
                dirs, files, small, ..
            } = dir;
            let mut children: Vec<Pending> =
                Vec::with_capacity(dirs.len() + files.len() + usize::from(small.is_some()));
            children.extend(dirs.into_iter().map(Pending::Dir));
            children.extend(files.into_iter().map(Pending::File));
            children.extend(small.filter(|s| s.count > 0).map(Pending::Small));
            children.sort_by_key(|child| std::cmp::Reverse(child.size()));
            let first_child = nodes.len() as NodeId;
            nodes[id as usize].first_child = first_child;
            nodes[id as usize].child_count = children.len() as u32;
            for child in children {
                let child_id = nodes.len() as NodeId;
                match child {
                    Pending::Dir(d) => {
                        nodes.push(dir_node(&d, id, d.name.clone()));
                        queue.push_back((child_id, d));
                    }
                    Pending::File(f) => nodes.push(ArenaNode {
                        name: f.name.into_boxed_str(),
                        parent: id,
                        first_child: 0,
                        child_count: 0,
                        kind: NodeKind::File,
                        size: f.size,
                        items: 1,
                        modified: f.modified,
                        accessed: f.accessed,
                        category: None,
                        unreadable: 0,
                    }),
                    Pending::Small(s) => nodes.push(ArenaNode {
                        name: format!("{} small files", s.count).into_boxed_str(),
                        parent: id,
                        first_child: 0,
                        child_count: 0,
                        kind: NodeKind::SmallFiles,
                        size: s.size,
                        items: s.count,
                        modified: s.modified,
                        accessed: s.accessed,
                        category: None,
                        unreadable: 0,
                    }),
                }
            }
        }
        Self { root_path, nodes }
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn node(&self, id: NodeId) -> Option<&ArenaNode> {
        self.nodes.get(id as usize)
    }

    pub fn children(&self, id: NodeId) -> impl Iterator<Item = (NodeId, &ArenaNode)> {
        let (first, count) = self
            .node(id)
            .map(|n| (n.first_child, n.child_count))
            .unwrap_or((0, 0));
        (first..first + count).filter_map(move |child| self.node(child).map(|n| (child, n)))
    }

    pub fn path(&self, id: NodeId) -> PathBuf {
        let mut names = Vec::new();
        let mut current = id;
        while let Some(node) = self.node(current) {
            if node.parent == NO_PARENT {
                break;
            }
            if node.kind != NodeKind::SmallFiles {
                names.push(&*node.name);
            }
            current = node.parent;
        }
        let mut path = self.root_path.clone();
        for name in names.into_iter().rev() {
            path.push(name);
        }
        path
    }

    /// Node for an absolute path inside the scan root.
    pub fn find(&self, path: &Path) -> Option<NodeId> {
        let relative = path.strip_prefix(&self.root_path).ok()?;
        let mut current: NodeId = 0;
        for component in relative.components() {
            let name = component.as_os_str().to_string_lossy();
            current = self
                .children(current)
                .find(|(_, n)| n.kind != NodeKind::SmallFiles && *n.name == *name)
                .map(|(id, _)| id)?;
        }
        Some(current)
    }

    pub fn is_descendant(&self, id: NodeId, ancestor: NodeId) -> bool {
        let mut current = id;
        while let Some(node) = self.node(current) {
            if current == ancestor {
                return true;
            }
            if node.parent == NO_PARENT {
                return false;
            }
            current = node.parent;
        }
        false
    }

    /// Ids of every node beneath `id` (inclusive), depth first.
    pub fn subtree(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            out.push(current);
            stack.extend(self.children(current).map(|(child, _)| child));
        }
        out
    }

    pub fn file_entry(&self, id: NodeId) -> Option<FileEntry> {
        let node = self.node(id)?;
        if node.kind != NodeKind::File {
            return None;
        }
        let ext = extension(&node.name);
        Some(FileEntry {
            node_id: id,
            path: self.path(id).to_string_lossy().into_owned(),
            name: node.name.to_string(),
            size_bytes: node.size,
            modified: node.modified,
            accessed: node.accessed,
            file_kind: kind_for_extension(ext.as_deref()),
            extension: ext,
        })
    }

    pub fn files(&self) -> impl Iterator<Item = (NodeId, &ArenaNode)> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.kind == NodeKind::File)
            .map(|(i, n)| (i as NodeId, n))
    }

    pub fn nodes_with_category(&self) -> impl Iterator<Item = (NodeId, &ArenaNode)> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.category.is_some())
            .map(|(i, n)| (i as NodeId, n))
    }

    /// Tree slice for the treemap: `depth` levels below `id`, at most `max_children` per level
    /// (the rest summed into a `Remainder` node).
    pub fn slice(&self, id: NodeId, depth: u8, max_children: u32) -> Option<TreeNode> {
        self.node(id)?;
        Some(self.build(id, depth, max_children.max(1)))
    }

    fn build(&self, id: NodeId, depth: u8, max_children: u32) -> TreeNode {
        let node = &self.nodes[id as usize];
        let (extension_value, file_kind) = if node.kind == NodeKind::File {
            let ext = extension(&node.name);
            let kind = kind_for_extension(ext.as_deref());
            (ext, Some(kind))
        } else {
            (None, None)
        };
        let has_children = node.child_count > 0;
        let children = (depth > 0 && has_children).then(|| {
            let mut out: Vec<TreeNode> = self
                .children(id)
                .take(max_children as usize)
                .map(|(child, _)| self.build(child, depth - 1, max_children))
                .collect();
            if node.child_count > max_children {
                let (size, items) = self
                    .children(id)
                    .skip(max_children as usize)
                    .fold((0u64, 0u64), |(s, i), (_, n)| (s + n.size, i + n.items));
                let hidden = node.child_count - max_children;
                out.push(TreeNode {
                    id: REMAINDER_FLAG | id,
                    name: format!("{hidden} more items"),
                    path: self.path(id).to_string_lossy().into_owned(),
                    kind: NodeKind::Remainder,
                    size_bytes: size,
                    item_count: items,
                    modified: None,
                    accessed: None,
                    extension: None,
                    file_kind: None,
                    category: None,
                    unreadable_entries: 0,
                    has_children: false,
                    children: None,
                });
            }
            out
        });
        TreeNode {
            id,
            name: node.name.to_string(),
            path: self.path(id).to_string_lossy().into_owned(),
            kind: node.kind,
            size_bytes: node.size,
            item_count: node.items,
            modified: node.modified,
            accessed: node.accessed,
            extension: extension_value,
            file_kind,
            category: node.category,
            unreadable_entries: node.unreadable,
            has_children,
            children,
        }
    }
}

fn dir_node(dir: &BuiltDir, parent: NodeId, name: String) -> ArenaNode {
    ArenaNode {
        name: name.into_boxed_str(),
        parent,
        first_child: 0,
        child_count: 0,
        kind: NodeKind::Directory,
        size: dir.size,
        items: dir.items,
        modified: dir.modified,
        accessed: dir.accessed,
        category: dir.category,
        unreadable: dir.unreadable,
    }
}

fn root_display_name(path: &Path, fallback: &str) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            if fallback.is_empty() {
                path.to_string_lossy().into_owned()
            } else {
                fallback.to_owned()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str, size: u64) -> BuiltFile {
        BuiltFile {
            name: name.into(),
            size,
            modified: Some(10),
            accessed: None,
        }
    }

    pub(crate) fn sample() -> ScanTree {
        let docs = BuiltDir {
            name: "docs".into(),
            size: 700,
            items: 3,
            files: vec![file("a.pdf", 400), file("b.pdf", 200)],
            small: Some(SmallFiles {
                size: 100,
                count: 1,
                modified: Some(5),
                accessed: None,
            }),
            ..Default::default()
        };
        let root = BuiltDir {
            name: "root".into(),
            size: 1700,
            items: 4,
            dirs: vec![docs],
            files: vec![file("big.mov", 1000)],
            ..Default::default()
        };
        ScanTree::from_built(PathBuf::from("/data/root"), root)
    }

    #[test]
    fn flattens_sorted_and_contiguous() {
        let tree = sample();
        assert_eq!(tree.len(), 6);
        let names: Vec<_> = tree.children(0).map(|(_, n)| n.name.to_string()).collect();
        assert_eq!(names, vec!["big.mov", "docs"]);
        let docs = tree.find(Path::new("/data/root/docs")).unwrap();
        assert_eq!(tree.path(docs), PathBuf::from("/data/root/docs"));
        let pdf = tree.find(Path::new("/data/root/docs/b.pdf")).unwrap();
        assert!(tree.is_descendant(pdf, docs));
        assert_eq!(
            tree.file_entry(pdf).unwrap().extension.as_deref(),
            Some("pdf")
        );
        assert!(tree.find(Path::new("/elsewhere")).is_none());
    }

    #[test]
    fn slices_with_remainder() {
        let tree = sample();
        let docs = tree.find(Path::new("/data/root/docs")).unwrap();
        let slice = tree.slice(docs, 1, 1).unwrap();
        let children = slice.children.unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name, "a.pdf");
        assert_eq!(children[1].kind, NodeKind::Remainder);
        assert_eq!(children[1].size_bytes, 300);
        assert_eq!(children[1].item_count, 2);
        let shallow = tree.slice(0, 0, 10).unwrap();
        assert!(shallow.has_children && shallow.children.is_none());
        let small = tree
            .children(docs)
            .find(|(_, n)| n.kind == NodeKind::SmallFiles)
            .unwrap()
            .0;
        assert_eq!(tree.path(small), PathBuf::from("/data/root/docs"));
    }
}

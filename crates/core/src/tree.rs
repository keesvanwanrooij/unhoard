//! The scan tree: an arena of nodes with totals that stay correct while the scan runs.
//!
//! Every node keeps its aggregated logical size, size on disk and file count, plus, for
//! folders, the bytes per category of content, so the dominant kind of a folder is known
//! without walking it. Adding a child adds its sizes to all ancestors (O(depth)), which is
//! what makes the live build-up of the map possible.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::fs::{FileId, FileTime, FsError};

/// Index of a node in the tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u32);

const NONE: u32 = u32::MAX;

/// What a node is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    /// A regular file.
    File,
    /// A folder.
    Folder,
    /// A junction or symbolic link, recorded but never followed.
    Link,
}

/// The kind of content, which sets the colour on the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum Category {
    /// Photos and videos.
    Media,
    /// Programs and games.
    Apps,
    /// Installers, ISOs and archives in Downloads.
    Downloads,
    /// Windows itself.
    System,
    /// Documents.
    Documents,
    /// Developer tools, caches and build folders.
    Developer,
    /// Local copies of files in a cloud sync root.
    Cloud,
    /// Space the scan could not see.
    Hidden,
    /// Everything else.
    Other,
}

impl Category {
    /// Every category, in a fixed order used for the per-folder histogram.
    pub const ALL: [Category; 9] = [
        Category::Media,
        Category::Apps,
        Category::Downloads,
        Category::System,
        Category::Documents,
        Category::Developer,
        Category::Cloud,
        Category::Hidden,
        Category::Other,
    ];

    /// The position of this category in [`Category::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }
}

/// The node was a second name for a file already counted (a hard link): it adds nothing.
pub const FLAG_HARD_LINK_DUP: u8 = 1;
/// The folder could not be listed; see [`Tree::unscanned`].
pub const FLAG_UNSCANNED: u8 = 2;
/// The name is not valid Unicode, so it is shown approximately and never moved.
pub const FLAG_LOSSY_NAME: u8 = 4;
/// The node was removed by a rescan and is no longer part of the tree.
pub const FLAG_DETACHED: u8 = 8;
/// The folder's children are listed (it was scanned, completely or partly).
pub const FLAG_LISTED: u8 = 16;
/// The folder was measured by the elevated helper: the app itself has no rights there.
pub const FLAG_ELEVATED: u8 = 32;

/// One file, folder or link.
#[derive(Clone, Debug)]
pub struct Node {
    /// The name, without the path.
    pub name: Box<str>,
    /// The parent, or `None` for the root.
    pub parent: Option<NodeId>,
    /// File, folder or link.
    pub kind: NodeKind,
    /// Logical size; for a folder the total of its contents.
    pub logical: u64,
    /// Size on disk; for a folder the total of its contents.
    pub on_disk: u64,
    /// Number of files; 1 for a file.
    pub files: u64,
    /// The `FILE_ATTRIBUTE_*` flags.
    pub attributes: u32,
    /// The 128-bit file ID.
    pub file_id: FileId,
    /// Last write time.
    pub modified: FileTime,
    /// The newest last write time of any file inside (for a file, its own).
    pub newest: FileTime,
    /// The kind of content (for a folder: the kind its location gives it, if any).
    pub category: Category,
    /// Index of the known-location rule that matches this node, if any.
    pub rule: Option<u16>,
    /// `FLAG_*` bits.
    pub flags: u8,
    first_child: u32,
    next_sibling: u32,
    cat_bytes: Option<Box<[u64; 9]>>,
}

/// What the scanner knows about a new child.
#[derive(Clone, Debug)]
pub struct ChildSpec {
    /// The name.
    pub name: String,
    /// File, folder or link.
    pub kind: NodeKind,
    /// Logical size of a file.
    pub logical: u64,
    /// Size on disk of a file.
    pub on_disk: u64,
    /// Attributes.
    pub attributes: u32,
    /// File ID.
    pub file_id: FileId,
    /// Last write time.
    pub modified: FileTime,
    /// Kind of content.
    pub category: Category,
    /// Matching rule.
    pub rule: Option<u16>,
    /// `FLAG_LOSSY_NAME` when applicable.
    pub flags: u8,
}

/// Sizes of the children that are not in a top-N list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Others {
    /// How many children.
    pub count: u32,
    /// Their logical size.
    pub logical: u64,
    /// Their size on disk.
    pub on_disk: u64,
}

/// The tree of one scan.
#[derive(Clone, Debug)]
pub struct Tree {
    nodes: Vec<Node>,
    root_path: String,
    /// The serial number of the scanned volume.
    pub volume_serial: u64,
    /// Folders that could not be listed, with the reason.
    pub unscanned: Vec<(NodeId, FsError)>,
    seen: HashSet<FileId>,
}

impl Tree {
    /// Creates a tree whose root is the folder at `root_path`.
    pub fn new(root_path: &str, volume_serial: u64) -> Self {
        let root_path = normalize_root(root_path);
        let name: Box<str> = root_path.clone().into();
        let root = Node {
            name,
            parent: None,
            kind: NodeKind::Folder,
            logical: 0,
            on_disk: 0,
            files: 0,
            attributes: crate::fs::ATTR_DIRECTORY,
            file_id: FileId(0),
            modified: 0,
            newest: 0,
            category: Category::Other,
            rule: None,
            flags: 0,
            first_child: NONE,
            next_sibling: NONE,
            cat_bytes: Some(Box::new([0; 9])),
        };
        Self {
            nodes: vec![root],
            root_path,
            volume_serial,
            unscanned: vec![],
            seen: HashSet::new(),
        }
    }

    /// The root node.
    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    /// The path of the root, for example `C:\` or `C:\Users\you`.
    pub fn root_path(&self) -> &str {
        &self.root_path
    }

    /// The node with this ID, unless it does not exist or was detached by a rescan.
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes
            .get(id.0 as usize)
            .filter(|n| n.flags & FLAG_DETACHED == 0)
    }

    pub(crate) fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes
            .get_mut(id.0 as usize)
            .filter(|n| n.flags & FLAG_DETACHED == 0)
    }

    /// Number of nodes ever added, detached ones included.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// True if the tree holds only its root.
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }

    /// The direct children of a node.
    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = vec![];
        let Some(n) = self.get(id) else { return out };
        let mut c = n.first_child;
        while c != NONE {
            if let Some(child) = self.nodes.get(c as usize) {
                if child.flags & FLAG_DETACHED == 0 {
                    out.push(NodeId(c));
                }
                c = child.next_sibling;
            } else {
                break;
            }
        }
        out
    }

    /// The full path of a node.
    pub fn path(&self, id: NodeId) -> String {
        let mut parts = vec![];
        let mut cur = Some(id);
        while let Some(c) = cur {
            match self.nodes.get(c.0 as usize) {
                Some(n) if n.parent.is_some() => {
                    parts.push(n.name.as_ref());
                    cur = n.parent;
                }
                _ => break,
            }
        }
        let mut p = self.root_path.clone();
        for part in parts.iter().rev() {
            if !p.ends_with('\\') {
                p.push('\\');
            }
            p.push_str(part);
        }
        p
    }

    /// Finds the node at `path` (case-insensitive), if it was scanned.
    pub fn find(&self, path: &str) -> Option<NodeId> {
        let root = self.root_path.trim_end_matches('\\').to_lowercase();
        let p = path.trim_end_matches('\\').to_lowercase();
        if p == root {
            return Some(self.root());
        }
        let rest = p.strip_prefix(&format!("{root}\\"))?;
        let mut cur = self.root();
        for part in rest.split('\\') {
            cur = self.children(cur).into_iter().find(|c| {
                self.get(*c)
                    .map(|n| n.name.to_lowercase() == part)
                    .unwrap_or(false)
            })?;
        }
        Some(cur)
    }

    /// True if `ancestor` is `id` or one of its ancestors.
    pub fn is_ancestor(&self, ancestor: NodeId, id: NodeId) -> bool {
        let mut cur = Some(id);
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.nodes.get(c.0 as usize).and_then(|n| n.parent);
        }
        false
    }

    /// Adds a child and adds its sizes to every ancestor. Returns the new node, or `None`
    /// if the parent does not exist.
    pub fn add_child(&mut self, parent: NodeId, spec: ChildSpec) -> Option<NodeId> {
        let id = u32::try_from(self.nodes.len()).ok()?;
        let p = self.get(parent)?;
        let first = p.first_child;
        let mut flags = spec.flags;
        let (logical, on_disk, files) = match spec.kind {
            NodeKind::File => {
                if self.seen.insert(spec.file_id) || spec.file_id == FileId(0) {
                    (spec.logical, spec.on_disk, 1)
                } else {
                    flags |= FLAG_HARD_LINK_DUP;
                    (0, 0, 0)
                }
            }
            _ => (0, 0, 0),
        };
        self.nodes.push(Node {
            name: spec.name.into(),
            parent: Some(parent),
            kind: spec.kind,
            logical,
            on_disk,
            files,
            attributes: spec.attributes,
            file_id: spec.file_id,
            modified: spec.modified,
            newest: if spec.kind == NodeKind::File {
                spec.modified
            } else {
                0
            },
            category: spec.category,
            rule: spec.rule,
            flags,
            first_child: NONE,
            next_sibling: first,
            cat_bytes: (spec.kind == NodeKind::Folder).then(|| Box::new([0; 9])),
        });
        if let Some(p) = self.nodes.get_mut(parent.0 as usize) {
            p.first_child = id;
        }
        if spec.kind == NodeKind::File && files == 1 {
            let mut cats = [0i128; 9];
            cats[spec.category.index()] = on_disk as i128;
            self.adjust_ancestors(parent, logical as i128, on_disk as i128, 1, &cats);
        }
        if spec.kind == NodeKind::File {
            let mut cur = Some(parent);
            while let Some(c) = cur {
                let Some(n) = self.nodes.get_mut(c.0 as usize) else {
                    break;
                };
                if n.newest >= spec.modified {
                    break;
                }
                n.newest = spec.modified;
                cur = n.parent;
            }
        }
        Some(NodeId(id))
    }

    /// Gives an unscanned folder the totals the elevated helper measured, adds them to its
    /// ancestors and marks it [`FLAG_ELEVATED`]. Returns false if the node is not an
    /// unscanned folder, so an item is never counted twice.
    pub fn set_measured(&mut self, id: NodeId, logical: u64, on_disk: u64, files: u64) -> bool {
        let Some(n) = self.get_mut(id) else {
            return false;
        };
        if n.flags & FLAG_UNSCANNED == 0 || n.flags & FLAG_ELEVATED != 0 {
            return false;
        }
        n.flags = (n.flags & !FLAG_UNSCANNED) | FLAG_ELEVATED;
        let mut cats = [0i128; 9];
        cats[n.category.index()] = on_disk as i128;
        self.unscanned.retain(|(u, _)| *u != id);
        self.adjust_ancestors(
            id,
            logical as i128,
            on_disk as i128,
            files.min(i64::MAX as u64) as i64,
            &cats,
        );
        true
    }

    fn adjust_ancestors(
        &mut self,
        from: NodeId,
        logical: i128,
        on_disk: i128,
        files: i64,
        cats: &[i128; 9],
    ) {
        let mut cur = Some(from);
        while let Some(c) = cur {
            let Some(n) = self.nodes.get_mut(c.0 as usize) else {
                break;
            };
            n.logical = (n.logical as i128 + logical).max(0) as u64;
            n.on_disk = (n.on_disk as i128 + on_disk).max(0) as u64;
            n.files = (n.files as i64 + files).max(0) as u64;
            if let Some(cb) = n.cat_bytes.as_mut() {
                for (slot, d) in cb.iter_mut().zip(cats.iter()) {
                    *slot = (*slot as i128 + d).max(0) as u64;
                }
            }
            cur = n.parent;
        }
    }

    /// Marks a folder as listed, or as unscanned with the reason it could not be read.
    pub fn mark_listed(&mut self, id: NodeId, error: Option<FsError>) {
        if let Some(n) = self.get_mut(id) {
            match error {
                None => n.flags |= FLAG_LISTED,
                Some(_) => n.flags |= FLAG_UNSCANNED,
            }
        }
        if let Some(e) = error {
            self.unscanned.push((id, e));
        }
    }

    /// Removes every descendant of `id` and subtracts their sizes from the ancestors, so the
    /// folder can be scanned again.
    pub fn clear_children(&mut self, id: NodeId) {
        let Some(n) = self.get(id) else { return };
        let (logical, on_disk, files) = (n.logical, n.on_disk, n.files);
        let cats = n.cat_bytes.as_deref().copied();
        let mut stack = self.children(id);
        while let Some(c) = stack.pop() {
            stack.extend(self.children(c));
            if let Some(node) = self.nodes.get_mut(c.0 as usize) {
                node.flags |= FLAG_DETACHED;
                if node.kind == NodeKind::File && node.flags & FLAG_HARD_LINK_DUP == 0 {
                    self.seen.remove(&node.file_id);
                }
            }
        }
        let gone: Vec<NodeId> = self
            .unscanned
            .iter()
            .filter(|(u, _)| self.is_ancestor(id, *u))
            .map(|(u, _)| *u)
            .collect();
        self.unscanned.retain(|(u, _)| !gone.contains(u));
        if let Some(n) = self.nodes.get_mut(id.0 as usize) {
            n.first_child = NONE;
            n.logical = 0;
            n.on_disk = 0;
            n.files = 0;
            n.flags &= !(FLAG_UNSCANNED | FLAG_LISTED);
            if let Some(cb) = n.cat_bytes.as_mut() {
                **cb = [0; 9];
            }
        }
        if let Some(parent) = self.get(id).and_then(|n| n.parent) {
            let mut neg = [0i128; 9];
            if let Some(cb) = cats {
                for (d, v) in neg.iter_mut().zip(cb.iter()) {
                    *d = -(*v as i128);
                }
            }
            self.adjust_ancestors(
                parent,
                -(logical as i128),
                -(on_disk as i128),
                -(files as i64),
                &neg,
            );
        }
    }

    /// Removes a node and its subtree from the tree (after it was moved away) and subtracts
    /// its sizes from every ancestor. The root cannot be detached.
    pub fn detach(&mut self, id: NodeId) {
        if id == self.root() {
            return;
        }
        let Some(n) = self.get(id) else { return };
        let (logical, on_disk, files, parent) = (n.logical, n.on_disk, n.files, n.parent);
        let mut cats = [0i128; 9];
        match n.cat_bytes.as_deref() {
            Some(cb) => {
                for (d, v) in cats.iter_mut().zip(cb.iter()) {
                    *d = -(*v as i128);
                }
            }
            None => cats[n.category.index()] = -(on_disk as i128),
        }
        let mut stack = vec![id];
        while let Some(c) = stack.pop() {
            stack.extend(self.children(c));
            if let Some(node) = self.nodes.get_mut(c.0 as usize) {
                node.flags |= FLAG_DETACHED;
                if node.kind == NodeKind::File && node.flags & FLAG_HARD_LINK_DUP == 0 {
                    self.seen.remove(&node.file_id);
                }
            }
        }
        if let Some(p) = parent {
            self.adjust_ancestors(
                p,
                -(logical as i128),
                -(on_disk as i128),
                -(files as i64),
                &cats,
            );
        }
    }

    /// The bytes on disk of a folder per kind of content, indexed by [`Category::index`].
    pub fn category_bytes(&self, id: NodeId) -> Option<&[u64; 9]> {
        self.get(id)?.cat_bytes.as_deref()
    }

    /// The kind of content that holds most of a node's bytes on disk.
    pub fn dominant_category(&self, id: NodeId) -> Category {
        let Some(n) = self.get(id) else {
            return Category::Other;
        };
        match &n.cat_bytes {
            Some(cb) if n.category == Category::Other || n.rule.is_none() => {
                let mut best = (0u64, n.category);
                for c in Category::ALL {
                    if cb[c.index()] > best.0 {
                        best = (cb[c.index()], c);
                    }
                }
                best.1
            }
            _ => n.category,
        }
    }

    /// The `n` largest children of a node, plus the totals of the rest.
    pub fn top_children(&self, id: NodeId, n: usize, logical: bool) -> (Vec<NodeId>, Others) {
        let mut kids = self.children(id);
        let size = |c: &NodeId| {
            self.get(*c)
                .map(|x| if logical { x.logical } else { x.on_disk })
                .unwrap_or(0)
        };
        kids.sort_by_key(|c| std::cmp::Reverse(size(c)));
        let rest = kids.split_off(n.min(kids.len()));
        let mut others = Others::default();
        for c in rest {
            if let Some(x) = self.get(c) {
                others.count += 1;
                others.logical += x.logical;
                others.on_disk += x.on_disk;
            }
        }
        (kids, others)
    }

    /// Iterates over every live node.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.flags & FLAG_DETACHED == 0)
            .map(|(i, n)| (NodeId(i as u32), n))
    }
}

/// Normalises a root path: `C:` and `C:\` become `C:\`; other paths lose a trailing `\`.
pub fn normalize_root(path: &str) -> String {
    let p = path.replace('/', "\\");
    let trimmed = p.trim_end_matches('\\');
    if trimmed.len() == 2 && trimmed.ends_with(':') {
        format!("{trimmed}\\")
    } else {
        trimmed.to_string()
    }
}

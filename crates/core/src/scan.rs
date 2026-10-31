//! The parallel scan: lists folders through the [`FileSystem`] and builds the [`Tree`].
//!
//! One task per folder runs on a work-stealing pool (rayon). Each task lists its folder in
//! one batched call, classifies the entries against the known-locations rules with the
//! listing as context (so sibling markers and ages come for free), adds them to the shared
//! tree under a short lock, and spawns tasks for the subfolders. Links are recorded but
//! never followed; folders that cannot be listed are kept as "unscanned" with their reason
//! and never stop the scan. Scanning only reads: it never changes a file, a timestamp or
//! an attribute.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use crate::classify::category_for_file;
use crate::fs::{FileSystem, FsError};
use crate::roots::{Roots, join};
use crate::rules::Rules;
use crate::tree::{Category, ChildSpec, FLAG_LOSSY_NAME, NodeId, NodeKind, Tree};

/// What the scan needs to classify items.
pub struct ScanContext {
    /// The known-locations rules.
    pub rules: Rules,
    /// The well-known locations of this PC.
    pub roots: Roots,
    /// The current time (seconds since 1970), for age conditions.
    pub now: i64,
}

/// Cancellation and live progress of a scan, shared with whoever started it.
#[derive(Default)]
pub struct ScanControl {
    /// Set to stop the scan; the partial result is kept.
    pub cancel: AtomicBool,
    /// Files added so far.
    pub files: AtomicU64,
    /// Bytes on disk added so far.
    pub on_disk: AtomicU64,
}

/// How a scan ended.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScanOutcome {
    /// True when the scan was cancelled before it finished.
    pub partial: bool,
}

fn lock(tree: &Mutex<Tree>) -> MutexGuard<'_, Tree> {
    // A panic while holding the lock leaves the tree consistent enough to show; recover it.
    match tree.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// Scans the folder at `root` into a new tree. Fails only if `root` itself cannot be read.
pub fn scan(
    fs: &dyn FileSystem,
    root: &str,
    ctx: &ScanContext,
    control: &ScanControl,
) -> Result<(Tree, ScanOutcome), FsError> {
    let serial = fs.volume(root).map(|v| v.serial).unwrap_or(0);
    let tree = Mutex::new(Tree::new(root, serial));
    let start = lock(&tree).root();
    let outcome = scan_into(fs, &tree, start, ctx, control);
    let tree = match tree.into_inner() {
        Ok(t) => t,
        Err(p) => p.into_inner(),
    };
    if let Some((id, e)) = tree.unscanned.first()
        && *id == tree.root()
    {
        return Err(e.clone());
    }
    Ok((tree, outcome))
}

/// Scans the folder `start` of a shared tree. Other threads can read the tree meanwhile
/// (the live map); totals are always consistent with what was added so far.
pub fn scan_into(
    fs: &dyn FileSystem,
    tree: &Mutex<Tree>,
    start: NodeId,
    ctx: &ScanContext,
    control: &ScanControl,
) -> ScanOutcome {
    let (path, inherited) = {
        let t = lock(tree);
        let path = t.path(start);
        let own = t
            .get(start)
            .map(|n| n.category)
            .filter(|c| *c != Category::Other);
        (
            path.clone(),
            own.or_else(|| ctx.roots.location_category(&path)),
        )
    };
    rayon::scope(|s| scan_dir(s, fs, tree, start, path, inherited, ctx, control));
    ScanOutcome {
        partial: control.cancel.load(Ordering::SeqCst),
    }
}

/// Clears a folder and scans it again; only that subtree changes.
pub fn rescan(
    fs: &dyn FileSystem,
    tree: &Mutex<Tree>,
    node: NodeId,
    ctx: &ScanContext,
    control: &ScanControl,
) -> ScanOutcome {
    lock(tree).clear_children(node);
    scan_into(fs, tree, node, ctx, control)
}

#[allow(clippy::too_many_arguments)]
fn scan_dir<'s>(
    scope: &rayon::Scope<'s>,
    fs: &'s dyn FileSystem,
    tree: &'s Mutex<Tree>,
    id: NodeId,
    path: String,
    inherited: Option<Category>,
    ctx: &'s ScanContext,
    control: &'s ScanControl,
) {
    let cancelled = |c: &AtomicBool| c.load(Ordering::Relaxed);
    if cancelled(&control.cancel) {
        return;
    }
    let entries = match fs.read_dir(&path) {
        Ok(e) => e,
        Err(e) => {
            lock(tree).mark_listed(id, Some(e));
            return;
        }
    };
    let matches = ctx.rules.match_listing(&path, &entries, ctx.now);
    let mut subdirs: Vec<(NodeId, String, Option<Category>)> = vec![];
    {
        let mut t = lock(tree);
        for (e, m) in entries.iter().zip(matches) {
            if cancelled(&control.cancel) {
                break;
            }
            let kind = if e.is_link() {
                NodeKind::Link
            } else if e.is_dir {
                NodeKind::Folder
            } else {
                NodeKind::File
            };
            let rule_cat = m.and_then(|r| ctx.rules.get(r)).map(|r| r.category);
            let child_path = join(&path, &e.name);
            let (category, pass_down) = match kind {
                NodeKind::Folder => {
                    let c = rule_cat
                        .or_else(|| ctx.roots.location_category(&child_path))
                        .or(inherited);
                    (c.unwrap_or(Category::Other), c)
                }
                _ => (
                    rule_cat
                        .or(inherited)
                        .or_else(|| category_for_file(&e.name))
                        .unwrap_or(Category::Other),
                    None,
                ),
            };
            let Some(child) = t.add_child(
                id,
                ChildSpec {
                    name: e.name.clone(),
                    kind,
                    logical: e.logical,
                    on_disk: e.on_disk,
                    attributes: e.attributes,
                    file_id: e.file_id,
                    modified: e.modified,
                    category,
                    rule: m,
                    flags: if e.lossy_name { FLAG_LOSSY_NAME } else { 0 },
                },
            ) else {
                continue;
            };
            if kind == NodeKind::File
                && let Some(n) = t.get(child)
            {
                control.files.fetch_add(n.files, Ordering::Relaxed);
                control.on_disk.fetch_add(n.on_disk, Ordering::Relaxed);
            }
            if kind == NodeKind::Folder {
                if e.lossy_name {
                    // The path of a folder whose name is not valid Unicode cannot be rebuilt
                    // exactly, so it is not listed; it shows as unscanned with the reason.
                    t.mark_listed(
                        child,
                        Some(FsError::Unsupported {
                            reason: "name is not valid Unicode".into(),
                        }),
                    );
                } else {
                    subdirs.push((child, child_path, pass_down));
                }
            }
        }
        t.mark_listed(id, None);
    }
    for (child, child_path, pass_down) in subdirs {
        scope.spawn(move |s| scan_dir(s, fs, tree, child, child_path, pass_down, ctx, control));
    }
}

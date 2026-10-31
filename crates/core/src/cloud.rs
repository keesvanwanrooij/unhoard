//! Cloud placeholders: states, sizes and "free up space" (F6). Risk class High.
//!
//! Everything here works from the attributes in the scan and from pin-state calls that
//! open files for attributes only. Nothing reads file data, so an online-only file is
//! never downloaded (S2). "Free up space" changes only the pin state, and only for files
//! that are fully uploaded, so the cloud always keeps a copy.

use serde::{Deserialize, Serialize};

use crate::fs::{ATTR_PINNED, ATTR_RECALL_ON_DATA_ACCESS, ATTR_UNPINNED, FileSystem, FsError};
use crate::roots::{file_name, parent_of};
use crate::scan::ScanContext;
use crate::tree::{NodeId, NodeKind, Tree};

/// The state of a file in a sync root, as Explorer shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PinState {
    /// Only in the cloud; takes no space on this PC.
    OnlineOnly,
    /// On this PC and in the cloud; can be freed.
    LocallyAvailable,
    /// "Always keep on this device".
    AlwaysKeep,
}

/// The pin state of a file from its attributes.
pub fn pin_state(attributes: u32) -> PinState {
    if attributes & ATTR_PINNED != 0 {
        PinState::AlwaysKeep
    } else if attributes & ATTR_RECALL_ON_DATA_ACCESS != 0 || attributes & ATTR_UNPINNED != 0 {
        PinState::OnlineOnly
    } else {
        PinState::LocallyAvailable
    }
}

/// Cloud versus local size and file counts of a folder in a sync root.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSummary {
    /// Size in the cloud (logical).
    pub cloud_bytes: u64,
    /// Size on this PC.
    pub local_bytes: u64,
    /// Online-only files.
    pub online_only: u64,
    /// Locally available files.
    pub locally_available: u64,
    /// "Always keep" files.
    pub always_keep: u64,
}

/// Counts the placeholder states below `id`.
pub fn summary(tree: &Tree, id: NodeId) -> CloudSummary {
    let mut s = CloudSummary::default();
    if let Some(n) = tree.get(id) {
        s.cloud_bytes = n.logical;
        s.local_bytes = n.on_disk;
    }
    let mut stack = vec![id];
    while let Some(c) = stack.pop() {
        let Some(n) = tree.get(c) else { continue };
        match n.kind {
            NodeKind::File => match pin_state(n.attributes) {
                PinState::OnlineOnly => s.online_only += 1,
                PinState::LocallyAvailable => s.locally_available += 1,
                PinState::AlwaysKeep => s.always_keep += 1,
            },
            NodeKind::Folder => stack.extend(tree.children(c)),
            NodeKind::Link => {}
        }
    }
    s
}

/// Why a file is not part of "free up space".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum FreeUpSkip {
    /// Not uploaded yet; freeing it would lose the only copy.
    NotInSync,
    /// Not inside a cloud sync root.
    NotInSyncRoot,
    /// The state could not be read.
    Unavailable(FsError),
}

/// The preview of "free up space".
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeUpPlan {
    /// Files that will become online-only.
    pub files: Vec<(NodeId, String)>,
    /// Size on disk they take now.
    pub bytes_to_free: u64,
    /// Files left out, with the reason.
    pub skipped: Vec<(String, FreeUpSkip)>,
}

/// Previews "free up space" for files and folders. Opens files for attributes only.
pub fn plan_free_up(
    fs: &dyn FileSystem,
    tree: &Tree,
    ids: &[NodeId],
    ctx: &ScanContext,
) -> FreeUpPlan {
    let mut plan = FreeUpPlan::default();
    let mut stack: Vec<NodeId> = ids.to_vec();
    while let Some(id) = stack.pop() {
        let Some(n) = tree.get(id) else { continue };
        let path = tree.path(id);
        match n.kind {
            NodeKind::Folder => stack.extend(tree.children(id)),
            NodeKind::Link => {}
            NodeKind::File => {
                if ctx.roots.sync_root_of(&path).is_none() {
                    plan.skipped.push((path, FreeUpSkip::NotInSyncRoot));
                    continue;
                }
                if pin_state(n.attributes) == PinState::OnlineOnly || n.on_disk == 0 {
                    continue;
                }
                match fs.placeholder_in_sync(&path) {
                    Ok(true) => {
                        plan.bytes_to_free += n.on_disk;
                        plan.files.push((id, path));
                    }
                    Ok(false) => plan.skipped.push((path, FreeUpSkip::NotInSync)),
                    Err(e) => plan.skipped.push((path, FreeUpSkip::Unavailable(e))),
                }
            }
        }
    }
    plan
}

/// Makes the planned files online-only by changing their pin state. Returns a result per
/// file; the space is reported only after [`remeasure`] (S10).
pub fn apply_free_up(fs: &dyn FileSystem, plan: &FreeUpPlan) -> Vec<(String, Result<(), FsError>)> {
    plan.files
        .iter()
        .map(|(_, p)| (p.clone(), fs.set_unpinned(p)))
        .collect()
}

/// Re-reads the size on disk of the given files from their folder listings (no file is
/// opened). Returns how much they still take.
pub fn remeasure(fs: &dyn FileSystem, paths: &[String]) -> u64 {
    let mut total = 0;
    let mut by_dir: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for p in paths {
        if let Some(d) = parent_of(p) {
            by_dir
                .entry(d)
                .or_default()
                .push(file_name(p).to_lowercase());
        }
    }
    for (dir, names) in by_dir {
        if let Ok(entries) = fs.read_dir(&dir) {
            total += entries
                .iter()
                .filter(|e| names.contains(&e.name.to_lowercase()))
                .map(|e| e.on_disk)
                .sum::<u64>();
        }
    }
    total
}

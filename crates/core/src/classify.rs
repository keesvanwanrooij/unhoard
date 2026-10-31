//! Describes an item for the user: its rule, tier, actions and protection.
//!
//! This is where the safety decisions about what may be offered live, so the UI only
//! renders them: protected paths never get a move action (S4), cloud items only get the
//! cloud quarantine inside their sync root (S3), links take no space and are never moved,
//! and large items without a rule get a generic explanation with tier Review.

use serde::{Deserialize, Serialize};

use crate::protect::{ProtectReason, protection};
pub use crate::rules::{Action, Tier, WindowsTool};
use crate::scan::ScanContext;
use crate::tree::{Category, FLAG_LOSSY_NAME, NodeId, NodeKind, Tree};

/// Items at least this large get the generic explanation when no rule matches (1 GiB).
pub const GENERIC_MIN_BYTES: u64 = 1 << 30;

/// What Unhoard tells the user about one item.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    /// The matching known-location rule.
    pub rule_id: Option<String>,
    /// The risk tier, if Unhoard has an opinion.
    pub tier: Option<Tier>,
    /// The actions on offer, the primary one first.
    pub actions: Vec<Action>,
    /// True when the explanation is the generic one for a large unmatched item.
    pub generic: bool,
    /// Why the item may not be moved, if it is protected.
    pub protected: Option<ProtectReason>,
    /// True for an item inside a cloud sync root.
    pub cloud: bool,
    /// Where the item would go when quarantined.
    pub quarantine_root: Option<String>,
    /// The rule's action needs admin rights.
    pub requires_admin: bool,
}

/// The kind of content of a file, from its extension.
pub fn category_for_file(name: &str) -> Option<Category> {
    let (_, ext) = crate::roots::split_ext(name);
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    const MEDIA: [&str; 26] = [
        "jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "bmp", "tif", "tiff", "raw", "cr2",
        "cr3", "nef", "arw", "dng", "mp4", "mov", "avi", "mkv", "wmv", "m4v", "3gp", "mts", "webm",
        "avif",
    ];
    const DOCS: [&str; 16] = [
        "doc", "docx", "xls", "xlsx", "ppt", "pptx", "pdf", "txt", "odt", "ods", "odp", "rtf",
        "md", "csv", "epub", "one",
    ];
    if MEDIA.contains(&ext.as_str()) {
        Some(Category::Media)
    } else if DOCS.contains(&ext.as_str()) {
        Some(Category::Documents)
    } else {
        None
    }
}

/// Describes the node `id`.
pub fn describe(tree: &Tree, id: NodeId, ctx: &ScanContext) -> Description {
    let Some(node) = tree.get(id) else {
        return Description::default();
    };
    let path = tree.path(id);
    let protected = protection(&path, &ctx.roots);
    let cloud = ctx.roots.sync_root_of(&path).is_some();
    let rule = node.rule.and_then(|r| ctx.rules.get(r));
    let mut d = Description {
        protected,
        cloud,
        ..Description::default()
    };
    let mut actions = match rule {
        Some(r) => {
            d.rule_id = Some(r.id.clone());
            d.tier = Some(r.tier);
            d.requires_admin = r.requires_admin;
            match r.action {
                Action::ShowInExplorer | Action::ExplainOnly => vec![r.action],
                a => vec![a, Action::ShowInExplorer],
            }
        }
        None if node.logical.max(node.on_disk) >= GENERIC_MIN_BYTES
            && node.kind != NodeKind::Link =>
        {
            d.generic = true;
            d.tier = Some(Tier::Review);
            vec![Action::ShowInExplorer, Action::Quarantine]
        }
        None => vec![Action::ShowInExplorer, Action::Quarantine],
    };
    if cloud && node.on_disk > 0 && !actions.contains(&Action::FreeUpSpace) {
        actions.insert(0, Action::FreeUpSpace);
    }
    let movable = protected.is_none()
        && node.kind != NodeKind::Link
        && node.flags & FLAG_LOSSY_NAME == 0
        && id != tree.root();
    if !movable {
        actions.retain(|a| *a != Action::Quarantine);
    }
    if actions.contains(&Action::Quarantine) {
        d.quarantine_root = ctx.roots.quarantine_root_for(&path);
    }
    d.actions = actions;
    d
}

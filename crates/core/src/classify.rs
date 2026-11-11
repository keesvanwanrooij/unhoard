//! Describes an item for the user: its rule, tier, actions and protection.
//!
//! This is where the safety decisions about what may be offered live, so the UI only
//! renders them: protected paths never get a move action (S4), cloud items only get the
//! cloud quarantine inside their sync root (S3), links take no space and are never moved,
//! items read with admin rights are never moved, and large items without a rule get tier
//! Review. Every item also carries the facts its explanation is built from
//! ([`crate::explain`]); a definition adds a written explanation without an opinion. What
//! Steam and Windows' list of installed programs know adds a name, a last-use date and the
//! right uninstall action.

use serde::{Deserialize, Serialize};

use crate::explain::{Explain, explain};
use crate::knowledge::{LastUse, Program, SteamInfo};
use crate::protect::{ProtectReason, protection};
pub use crate::rules::{Action, Tier, WindowsTool};
use crate::scan::ScanContext;
use crate::tree::{Category, FLAG_ELEVATED, FLAG_LOSSY_NAME, NodeId, NodeKind, Tree};

/// Items at least this large get tier Review when no rule has an opinion (1 GiB).
pub const LARGE_MIN_BYTES: u64 = 1 << 30;

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
    /// True for a large item no rule has an opinion about: worth a look for its size.
    pub large: bool,
    /// True when the rule is a definition: it explains the item, without an opinion.
    pub definition: bool,
    /// The facts the explanation is built from: place, owner, file type, contents.
    pub explain: Explain,
    /// Why the item may not be moved, if it is protected.
    pub protected: Option<ProtectReason>,
    /// True for an item inside a cloud sync root.
    pub cloud: bool,
    /// Where the item would go when quarantined.
    pub quarantine_root: Option<String>,
    /// The rule's action needs admin rights.
    pub requires_admin: bool,
    /// The installed program whose install folder this is.
    pub program: Option<Program>,
    /// What Steam knows about this folder.
    pub steam: Option<SteamInfo>,
    /// The folder was read with admin rights; the app itself cannot move it.
    pub elevated: bool,
    /// When the item was last used, and the source of that date.
    pub last_use: Option<LastUse>,
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
    let is_dir = node.kind == NodeKind::Folder;
    let steam = is_dir.then(|| ctx.knowledge.steam_info(&path)).flatten();
    let program = is_dir
        .then(|| ctx.knowledge.program_at(&path).cloned())
        .flatten();
    let mut d = Description {
        protected,
        cloud,
        elevated: node.flags & FLAG_ELEVATED != 0,
        last_use: ctx.knowledge.last_use(&path, node.newest),
        explain: explain(tree, id, &ctx.roots),
        ..Description::default()
    };
    let large = node.logical.max(node.on_disk) >= LARGE_MIN_BYTES && node.kind != NodeKind::Link;
    // A definition only explains; the cases below decide as if no rule matched, unless it
    // says "keep".
    let definition = rule.filter(|r| r.define);
    if let Some(r) = definition {
        d.rule_id = Some(r.id.clone());
        d.definition = true;
    }
    let rule = rule.filter(|r| !r.define);
    let mut actions = match (&steam, rule, &program) {
        (Some(SteamInfo::Game { .. }), ..) => {
            // An installed game goes through Steam, so Steam stays consistent.
            d.tier = Some(Tier::Review);
            vec![Action::AppUninstall, Action::ShowInExplorer]
        }
        (Some(SteamInfo::Leftover), ..) => {
            d.tier = Some(Tier::LikelySafe);
            vec![Action::Quarantine, Action::ShowInExplorer]
        }
        (None, Some(r), _) => {
            d.rule_id = Some(r.id.clone());
            d.tier = Some(r.tier);
            d.requires_admin = r.requires_admin;
            match r.action {
                Action::ShowInExplorer => vec![Action::ShowInExplorer],
                a => vec![a, Action::ShowInExplorer],
            }
        }
        (None, None, Some(_)) => {
            d.tier = Some(Tier::Review);
            vec![Action::Uninstaller, Action::ShowInExplorer]
        }
        (None, None, None) if definition.is_some_and(|r| r.opinion == Some(Tier::Keep)) => {
            d.tier = Some(Tier::Keep);
            vec![Action::ExplainOnly, Action::ShowInExplorer]
        }
        (None, None, None) if large && definition.is_none() => {
            d.large = true;
            d.tier = Some(Tier::Review);
            vec![Action::ShowInExplorer, Action::Quarantine]
        }
        (None, None, None) => vec![Action::ShowInExplorer, Action::Quarantine],
    };
    d.steam = steam;
    d.program = program;
    if cloud && node.on_disk > 0 && !actions.contains(&Action::FreeUpSpace) {
        actions.insert(0, Action::FreeUpSpace);
    }
    let movable = protected.is_none()
        && node.kind != NodeKind::Link
        && node.flags & (FLAG_LOSSY_NAME | FLAG_ELEVATED) == 0
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

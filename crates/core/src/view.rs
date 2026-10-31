//! What the UI asks for: one level of the tree at a time, and the suggestions list.
//!
//! The UI never receives the whole tree (memory and IPC stay bounded). A view is a node,
//! its largest children with everything the map and the list need, the totals of the
//! rest, and the breadcrumb. Grouping of items under 0.5% is done by the UI per view.

use serde::{Deserialize, Serialize};

use crate::classify::{GENERIC_MIN_BYTES, describe};
use crate::cloud::{PinState, pin_state};
use crate::rules::Tier;
use crate::scan::ScanContext;
use crate::tree::{Category, FLAG_HARD_LINK_DUP, FLAG_UNSCANNED, NodeId, NodeKind, Others, Tree};

/// One item as the map and the list show it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewNode {
    /// The scan node.
    pub id: NodeId,
    /// Its name.
    pub name: String,
    /// File, folder or link.
    pub kind: NodeKind,
    /// Logical size.
    pub logical: u64,
    /// Size on disk.
    pub on_disk: u64,
    /// Number of files.
    pub files: u64,
    /// The kind of content (dominant for a folder).
    pub category: Category,
    /// The risk tier, if Unhoard has one.
    pub tier: Option<Tier>,
    /// The matching rule.
    pub rule_id: Option<String>,
    /// True when it has children to zoom into.
    pub has_children: bool,
    /// True when it could not be read; its size is unknown.
    pub unscanned: bool,
    /// The pin state of a cloud file.
    pub cloud: Option<PinState>,
}

/// One step of the breadcrumb.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Crumb {
    /// The node.
    pub id: NodeId,
    /// Its name (the root shows its full path).
    pub name: String,
}

/// One level of the tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    /// The node itself.
    pub node: ViewNode,
    /// Its full path.
    pub path: String,
    /// From the root to the node.
    pub breadcrumb: Vec<Crumb>,
    /// The largest children, largest first.
    pub children: Vec<ViewNode>,
    /// The totals of the other children.
    pub others: Others,
}

/// Builds the view of one node.
pub fn node_view(tree: &Tree, id: NodeId, ctx: &ScanContext) -> Option<ViewNode> {
    let n = tree.get(id)?;
    let rule = n.rule.and_then(|r| ctx.rules.get(r));
    let generic =
        rule.is_none() && n.logical.max(n.on_disk) >= GENERIC_MIN_BYTES && n.kind != NodeKind::Link;
    let in_cloud = ctx.roots.sync_root_of(&tree.path(id)).is_some();
    Some(ViewNode {
        id,
        name: if id == tree.root() {
            tree.root_path().to_string()
        } else {
            n.name.to_string()
        },
        kind: n.kind,
        logical: n.logical,
        on_disk: n.on_disk,
        files: n.files,
        category: tree.dominant_category(id),
        tier: rule.map(|r| r.tier).or(generic.then_some(Tier::Review)),
        rule_id: rule.map(|r| r.id.clone()),
        has_children: n.kind == NodeKind::Folder && n.files > 0,
        unscanned: n.flags & FLAG_UNSCANNED != 0,
        cloud: (in_cloud && n.kind == NodeKind::File).then(|| pin_state(n.attributes)),
    })
}

/// The view of `id` with its `top` largest children.
pub fn view(tree: &Tree, id: NodeId, top: usize, logical: bool, ctx: &ScanContext) -> Option<View> {
    let node = node_view(tree, id, ctx)?;
    let (kids, others) = tree.top_children(id, top, logical);
    let mut breadcrumb = vec![];
    let mut cur = Some(id);
    while let Some(c) = cur {
        let name = if c == tree.root() {
            tree.root_path().to_string()
        } else {
            tree.get(c).map(|n| n.name.to_string()).unwrap_or_default()
        };
        breadcrumb.push(Crumb { id: c, name });
        cur = tree.get(c).and_then(|n| n.parent);
    }
    breadcrumb.reverse();
    Some(View {
        node,
        path: tree.path(id),
        breadcrumb,
        children: kids
            .into_iter()
            .filter_map(|c| node_view(tree, c, ctx))
            .collect(),
        others,
    })
}

/// One entry of the Suggestions view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    /// The item.
    pub node: ViewNode,
    /// Its path.
    pub path: String,
    /// Its tier.
    pub tier: Tier,
    /// What Unhoard says about it.
    pub description: crate::classify::Description,
}

/// Suggestions smaller than this on disk free too little to be worth a click.
pub const SUGGESTION_MIN_BYTES: u64 = 1 << 20;

/// Every matched item plus large unmatched files, largest first, counted by size on disk.
/// An item inside another suggested item is left out, so nothing is counted twice, and so
/// is anything under [`SUGGESTION_MIN_BYTES`] on disk, such as online-only files. Nothing
/// is pre-selected (S1).
pub fn suggestions(tree: &Tree, ctx: &ScanContext, limit: usize) -> Vec<Suggestion> {
    // Size on disk decides: an online-only file or an empty cache frees nothing here.
    let mut picked: Vec<NodeId> = tree
        .iter()
        .filter(|(_, n)| n.flags & FLAG_HARD_LINK_DUP == 0 && n.kind != NodeKind::Link)
        .filter(|(_, n)| n.on_disk >= SUGGESTION_MIN_BYTES)
        .filter(|(_, n)| {
            n.rule.is_some() || (n.kind == NodeKind::File && n.on_disk >= GENERIC_MIN_BYTES)
        })
        .map(|(id, _)| id)
        .collect();
    let all: std::collections::HashSet<NodeId> = picked.iter().copied().collect();
    picked.retain(|&id| {
        let mut cur = tree.get(id).and_then(|n| n.parent);
        while let Some(c) = cur {
            if all.contains(&c) {
                return false;
            }
            cur = tree.get(c).and_then(|n| n.parent);
        }
        true
    });
    let mut out: Vec<Suggestion> = picked
        .into_iter()
        .filter_map(|id| {
            let node = node_view(tree, id, ctx)?;
            let description = describe(tree, id, ctx);
            let tier = description.tier?;
            Some(Suggestion {
                path: tree.path(id),
                node,
                tier,
                description,
            })
        })
        .collect();
    out.sort_by_key(|s| std::cmp::Reverse(s.node.on_disk));
    out.truncate(limit);
    out
}

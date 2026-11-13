//! What the UI asks for: one level of the tree at a time, and the suggestions list.
//!
//! The UI never receives the whole tree (memory and IPC stay bounded). A view is a node,
//! its largest children with everything the map and the list need, the totals of the
//! rest, and the breadcrumb. Grouping of items under 0.5% is done by the UI per view.

use serde::{Deserialize, Serialize};

use std::collections::HashSet;

use crate::classify::{LARGE_MIN_BYTES, describe};
use crate::cloud::{PinState, pin_state};
use crate::explain::{Explain, explain};
use crate::knowledge::{LastUse, SteamInfo};
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
    /// The facts a one-line description is built from.
    pub explain: Explain,
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
    let large =
        rule.is_none() && n.logical.max(n.on_disk) >= LARGE_MIN_BYTES && n.kind != NodeKind::Link;
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
        tier: rule
            .and_then(|r| r.opinion)
            .or(large.then_some(Tier::Review)),
        rule_id: rule.map(|r| r.id.clone()),
        explain: explain(tree, id, &ctx.roots),
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

/// Program folders at least this large on disk are suggested (256 MiB).
pub const PROGRAM_MIN_BYTES: u64 = 256 << 20;

/// Every matched item plus large unmatched files, largest first, counted by size on disk.
/// In a Steam library the games and leftovers stand in for the whole `common` folder, and
/// a large program folder is suggested when nothing inside it is. An item inside another
/// suggested item is left out, so nothing is counted twice, and so is anything under
/// [`SUGGESTION_MIN_BYTES`] on disk, such as online-only files. Nothing is pre-selected
/// (S1).
pub fn suggestions(tree: &Tree, ctx: &ScanContext, limit: usize) -> Vec<Suggestion> {
    let big = |id: &NodeId| {
        tree.get(*id).is_some_and(|n| {
            n.flags & FLAG_HARD_LINK_DUP == 0
                && n.kind != NodeKind::Link
                && n.on_disk >= SUGGESTION_MIN_BYTES
        })
    };
    // Steam libraries: their games and leftovers are the candidates, not the folder.
    let steam_common: HashSet<NodeId> = ctx
        .knowledge
        .steam
        .iter()
        .filter_map(|l| tree.find(&l.common))
        .collect();
    let mut picked: Vec<NodeId> = tree
        .iter()
        .filter(|(id, n)| {
            !steam_common.contains(id)
                && (n
                    .rule
                    .and_then(|r| ctx.rules.get(r))
                    .is_some_and(|r| !r.define)
                    || (n.kind == NodeKind::File && n.on_disk >= LARGE_MIN_BYTES))
        })
        .map(|(id, _)| id)
        .filter(big)
        .chain(
            steam_common
                .iter()
                .flat_map(|c| tree.children(*c))
                .filter(|id| tree.get(*id).is_some_and(|n| n.kind == NodeKind::Folder))
                .filter(big),
        )
        .collect();
    // Size on disk decides: an online-only file or an empty cache frees nothing here.
    let primary: HashSet<NodeId> = picked.iter().copied().collect();
    let inside = |id: NodeId, set: &HashSet<NodeId>| {
        let mut cur = tree.get(id).and_then(|n| n.parent);
        while let Some(c) = cur {
            if set.contains(&c) {
                return true;
            }
            cur = tree.get(c).and_then(|n| n.parent);
        }
        false
    };
    picked.retain(|&id| !inside(id, &primary));
    // A program folder counts only when no other suggestion lies inside or around it.
    let programs: Vec<NodeId> = ctx
        .knowledge
        .programs
        .iter()
        .filter_map(|p| tree.find(&p.install_location))
        .filter(|id| !primary.contains(id) && !inside(*id, &primary))
        .filter(|id| {
            tree.get(*id)
                .is_some_and(|n| n.kind == NodeKind::Folder && n.on_disk >= PROGRAM_MIN_BYTES)
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let program_set: HashSet<NodeId> = programs.iter().copied().collect();
    let has_primary_inside: HashSet<NodeId> = primary
        .iter()
        .flat_map(|&id| {
            let mut out = vec![];
            let mut cur = tree.get(id).and_then(|n| n.parent);
            while let Some(c) = cur {
                if program_set.contains(&c) {
                    out.push(c);
                }
                cur = tree.get(c).and_then(|n| n.parent);
            }
            out
        })
        .collect();
    picked.extend(
        programs
            .into_iter()
            .filter(|id| !has_primary_inside.contains(id)),
    );
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

/// One bubble of the priority matrix (AC2.10).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatrixPoint {
    /// The scan node.
    pub id: NodeId,
    /// Its name.
    pub name: String,
    /// The name of the program or game it belongs to, when known.
    pub label: Option<String>,
    /// Its matching rule.
    pub rule_id: Option<String>,
    /// Size on disk: the x axis and the bubble size.
    pub on_disk: u64,
    /// Last use: the y axis.
    pub last_use: Option<LastUse>,
    /// The kind of content: the colour.
    pub category: Category,
    /// The risk tier.
    pub tier: Tier,
}

/// The priority matrix: every suggestion, largest first, at most `limit`. Items Unhoard
/// only explains (tier Keep) are left out: there is nothing to clear.
pub fn matrix(tree: &Tree, ctx: &ScanContext, limit: usize) -> Vec<MatrixPoint> {
    suggestions(tree, ctx, usize::MAX)
        .into_iter()
        .filter(|s| s.tier != Tier::Keep)
        .map(|s| {
            let d = s.description;
            let label = d
                .program
                .as_ref()
                .map(|p| p.name.clone())
                .or_else(|| match &d.steam {
                    Some(SteamInfo::Game { name, .. }) => Some(name.clone()),
                    _ => None,
                });
            MatrixPoint {
                id: s.node.id,
                name: s.node.name,
                label,
                rule_id: d.rule_id,
                on_disk: s.node.on_disk,
                last_use: d.last_use,
                category: s.node.category,
                tier: s.tier,
            }
        })
        .take(limit)
        .collect()
}

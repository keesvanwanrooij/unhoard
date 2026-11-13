//! The scan report: the JSON contract of `unhoard scan --json` (F10).
//!
//! Field names are part of the contract (`docs/05-mvp-scope.md`, F10). Paths inside the
//! user's profile are written as `%USERPROFILE%`, so a shared report never shows the user
//! name (S9), and a size that could not be read is `null`, never 0.

use serde::Serialize;

use crate::fs::{FsError, VolumeInfo};
use crate::hidden::hidden_bytes;
use crate::roots::is_within;
use crate::rules::Tier;
use crate::scan::ScanContext;
use crate::tree::{Category, FLAG_UNSCANNED, NodeId, NodeKind, Tree};

/// What the report includes.
#[derive(Clone, Debug)]
pub struct ReportOptions {
    /// The largest items per level.
    pub top: usize,
    /// How many levels below the root.
    pub depth: usize,
    /// Sort by logical size instead of size on disk.
    pub logical: bool,
    /// The user's profile folder, replaced by `%USERPROFILE%`.
    pub profile: Option<String>,
}

/// The report (schema version 1).
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// The schema version.
    pub schema_version: u32,
    /// The scanned folder.
    pub root: String,
    /// True when the scan was cancelled.
    pub partial: bool,
    /// How long the scan took.
    pub duration_ms: u64,
    /// The volume, when known.
    pub volume: Option<VolumeOut>,
    /// Totals of the scanned folder.
    pub totals: Totals,
    /// Hidden space, for a whole-drive scan only.
    pub hidden_bytes: Option<u64>,
    /// Folders that could not be read.
    pub unscanned: Vec<Unscanned>,
    /// The largest items.
    pub items: Vec<ItemOut>,
}

/// Volume information in the report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeOut {
    /// Total size.
    pub total_bytes: u64,
    /// Free bytes.
    pub free_bytes: u64,
    /// File system name.
    pub file_system: String,
}

/// Totals in the report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    /// Logical size.
    pub logical_bytes: u64,
    /// Size on disk.
    pub on_disk_bytes: u64,
    /// Number of files.
    pub files: u64,
    /// Number of folders.
    pub folders: u64,
}

/// A folder that could not be read.
#[derive(Clone, Debug, Serialize)]
pub struct Unscanned {
    /// Its path.
    pub path: String,
    /// Why: `accessDenied`, `notFound`, `locked` or `unsupported`.
    pub reason: String,
}

/// A known-location match in the report.
#[derive(Clone, Debug, Serialize)]
pub struct KnownOut {
    /// The rule ID.
    pub id: String,
    /// The risk tier.
    pub tier: Tier,
}

/// Cloud sizes of an item in a sync root.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudOut {
    /// Size in the cloud (logical).
    pub cloud_bytes: u64,
    /// Size on this PC.
    pub local_bytes: u64,
}

/// One item in the report.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemOut {
    /// Its path.
    pub path: String,
    /// `file`, `folder` or `link`.
    pub kind: NodeKind,
    /// Logical size, `null` when unknown.
    pub logical_bytes: Option<u64>,
    /// Size on disk, `null` when unknown.
    pub on_disk_bytes: Option<u64>,
    /// Number of files, `null` when unknown.
    pub files: Option<u64>,
    /// The kind of content.
    pub category: Category,
    /// The matching known location.
    pub known_location: Option<KnownOut>,
    /// Cloud sizes, for items in a sync root.
    pub cloud: Option<CloudOut>,
    /// The largest children, down to the requested depth.
    pub children: Vec<ItemOut>,
}

/// Replaces the profile path at the start of `path` with `%USERPROFILE%`.
pub fn redact(path: &str, profile: Option<&str>) -> String {
    match profile {
        Some(p) if !p.is_empty() && is_within(path, p) => {
            let base = p.trim_end_matches('\\');
            let rest = path.get(base.len()..).unwrap_or("");
            format!("%USERPROFILE%{rest}")
        }
        _ => path.to_string(),
    }
}

/// The reason code of an unscanned folder.
pub fn reason_code(e: &FsError) -> &'static str {
    match e {
        FsError::AccessDenied => "accessDenied",
        FsError::NotFound => "notFound",
        FsError::Locked => "locked",
        FsError::Unsupported { .. } => "unsupported",
        _ => "error",
    }
}

fn item(
    t: &Tree,
    id: NodeId,
    depth: usize,
    ctx: &ScanContext,
    o: &ReportOptions,
) -> Option<ItemOut> {
    let n = t.get(id)?;
    let path = t.path(id);
    let unknown = n.flags & FLAG_UNSCANNED != 0;
    let known = n.rule.and_then(|r| ctx.rules.get(r)).map(|r| KnownOut {
        id: r.id.clone(),
        tier: r.tier,
    });
    let cloud = ctx.roots.sync_root_of(&path).map(|_| CloudOut {
        cloud_bytes: n.logical,
        local_bytes: n.on_disk,
    });
    let children = if depth > 1 && n.kind == NodeKind::Folder {
        t.top_children(id, o.top, o.logical)
            .0
            .into_iter()
            .filter_map(|c| item(t, c, depth - 1, ctx, o))
            .collect()
    } else {
        vec![]
    };
    Some(ItemOut {
        path: redact(&path, o.profile.as_deref()),
        kind: n.kind,
        logical_bytes: (!unknown).then_some(n.logical),
        on_disk_bytes: (!unknown).then_some(n.on_disk),
        files: (!unknown).then_some(n.files),
        category: t.dominant_category(id),
        known_location: known,
        cloud,
        children,
    })
}

/// Builds the report of a scanned tree.
pub fn scan_report(
    tree: &Tree,
    volume: Option<&VolumeInfo>,
    partial: bool,
    duration_ms: u64,
    ctx: &ScanContext,
    o: &ReportOptions,
) -> Report {
    let root = tree.root();
    let r = tree.get(root);
    let folders = tree
        .iter()
        .filter(|(_, n)| n.kind == NodeKind::Folder)
        .count()
        .saturating_sub(1) as u64;
    let whole_drive = tree.root_path().len() <= 3;
    let hidden = match (volume, whole_drive) {
        (Some(v), true) => Some(hidden_bytes(v, r.map(|n| n.on_disk).unwrap_or(0), 0)),
        _ => None,
    };
    let items = tree
        .top_children(root, o.top, o.logical)
        .0
        .into_iter()
        .filter_map(|c| item(tree, c, o.depth, ctx, o))
        .collect();
    Report {
        schema_version: 1,
        root: redact(tree.root_path(), o.profile.as_deref()),
        partial,
        duration_ms,
        volume: volume.map(|v| VolumeOut {
            total_bytes: v.total_bytes,
            free_bytes: v.free_bytes,
            file_system: v.file_system.clone(),
        }),
        totals: Totals {
            logical_bytes: r.map(|n| n.logical).unwrap_or(0),
            on_disk_bytes: r.map(|n| n.on_disk).unwrap_or(0),
            files: r.map(|n| n.files).unwrap_or(0),
            folders,
        },
        hidden_bytes: hidden,
        unscanned: tree
            .unscanned
            .iter()
            .filter(|(id, _)| tree.get(*id).is_some())
            .map(|(id, e)| Unscanned {
                path: redact(&tree.path(*id), o.profile.as_deref()),
                reason: reason_code(e).into(),
            })
            .collect(),
        items,
    }
}

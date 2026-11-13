//! Storage history: small snapshots of the largest folders over time, and what grew
//! between two of them (F17).
//!
//! A snapshot holds the size on disk and file count of the largest folders up to three
//! levels below the scanned root, plus every folder a known-location rule or definition
//! matched within those levels and one level deeper, and the drive's totals. It holds no
//! file names and nothing below those folders. Snapshots are stored as one JSON line each;
//! the app appends and compacts the file, this module only builds, parses, thins out and
//! compares them.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::scan::ScanContext;
use crate::tree::{NodeId, NodeKind, Tree};

/// Seconds in a day.
pub const DAY: i64 = 86_400;
/// The most folders a snapshot keeps.
pub const MAX_FOLDERS: usize = 1_000;
/// Folders deeper than this below the root are kept only when a rule matched them.
pub const MAX_DEPTH: usize = 3;
/// Changes smaller than this are noise on a drive of hundreds of gigabytes (100 MiB).
pub const MIN_GROWTH: u64 = 100 << 20;
/// Every snapshot is kept for this long; older ones are thinned to one per week.
pub const DAILY_DAYS: i64 = 90;
/// Weekly snapshots are kept for this long.
pub const KEEP_DAYS: i64 = 730;
/// The history file may grow to this many bytes; the oldest snapshots go first.
pub const MAX_BYTES: usize = 20 << 20;
/// A folder holding at least this share of its parent's growth stands for the parent.
const CAUSE_SHARE: f64 = 0.8;

/// What made a snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotKind {
    /// A scan the user ran.
    Scan,
    /// The daily measurement.
    Daily,
}

/// The drive's totals at the moment of a snapshot; unknown values are `None`, never 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Totals {
    /// Used space on the drive.
    pub used: Option<u64>,
    /// Free space on the drive.
    pub free: Option<u64>,
    /// Used space the scan could not see.
    pub hidden: Option<u64>,
}

/// One folder in a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    /// Its full path.
    pub path: String,
    /// Size on disk of everything inside.
    pub on_disk: u64,
    /// Number of files inside.
    pub files: u64,
}

/// The largest folders of one root at one moment. On disk it is stored compactly
/// ([`Compact`]): each folder as its name and the index of its parent, so a snapshot of a
/// thousand folders takes tens of kilobytes instead of a hundred.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", into = "Compact", try_from = "Compact")]
pub struct Snapshot {
    /// When, in seconds since 1970.
    pub at: i64,
    /// A scan or the daily measurement.
    pub kind: SnapshotKind,
    /// The scanned root.
    pub root: String,
    /// The drive's totals.
    pub totals: Totals,
    /// The kept folders.
    pub folders: Vec<Folder>,
}

/// A snapshot as stored: folders as `[parent, name, onDisk, files]`, where `parent` is the
/// index of an earlier folder, or -1 for a folder directly in the root.
#[derive(Serialize, Deserialize)]
struct Compact {
    at: i64,
    kind: SnapshotKind,
    root: String,
    totals: Totals,
    f: Vec<(i64, String, u64, u64)>,
}

impl From<Snapshot> for Compact {
    fn from(s: Snapshot) -> Self {
        let mut index: HashMap<String, i64> = HashMap::new();
        let mut folders = s.folders;
        // Parents sort before their children, so every parent has an index already.
        folders.sort_by(|a, b| a.path.cmp(&b.path));
        let root = s.root.trim_end_matches('\\').to_string();
        let mut f = Vec::with_capacity(folders.len());
        for folder in folders {
            let (parent, name) = match folder.path.rfind('\\') {
                Some(i) => (&folder.path[..i], &folder.path[i + 1..]),
                None => ("", folder.path.as_str()),
            };
            let p = if parent.eq_ignore_ascii_case(&root) {
                -1
            } else {
                match index.get(parent) {
                    Some(i) => *i,
                    // A folder whose parent is not kept is stored with its full path.
                    None => {
                        f.push((-2, folder.path.clone(), folder.on_disk, folder.files));
                        index.insert(folder.path, (f.len() - 1) as i64);
                        continue;
                    }
                }
            };
            f.push((p, name.to_string(), folder.on_disk, folder.files));
            index.insert(folder.path, (f.len() - 1) as i64);
        }
        Compact {
            at: s.at,
            kind: s.kind,
            root: s.root,
            totals: s.totals,
            f,
        }
    }
}

impl TryFrom<Compact> for Snapshot {
    type Error = String;

    fn try_from(c: Compact) -> Result<Self, String> {
        let root = c.root.trim_end_matches('\\').to_string();
        let mut paths: Vec<String> = Vec::with_capacity(c.f.len());
        let mut folders = Vec::with_capacity(c.f.len());
        for (parent, name, on_disk, files) in c.f {
            let path = match parent {
                -2 => name,
                -1 => format!("{root}\\{name}"),
                i => {
                    let p = usize::try_from(i)
                        .ok()
                        .and_then(|i| paths.get(i))
                        .ok_or("a folder names a parent that comes after it")?;
                    format!("{p}\\{name}")
                }
            };
            paths.push(path.clone());
            folders.push(Folder {
                path,
                on_disk,
                files,
            });
        }
        Ok(Snapshot {
            at: c.at,
            kind: c.kind,
            root: c.root,
            totals: c.totals,
            folders,
        })
    }
}

/// How one folder changed between two snapshots.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Growth {
    /// The folder.
    pub path: String,
    /// Size on disk in the older snapshot (0 when it was not there).
    pub before: u64,
    /// Size on disk in the newer snapshot (0 when it is gone).
    pub after: u64,
    /// `after - before`.
    pub delta: i64,
}

/// The depth of `id` below the root.
fn depth(tree: &Tree, id: NodeId) -> usize {
    let mut d = 0;
    let mut cur = tree.get(id).and_then(|n| n.parent);
    while let Some(c) = cur {
        d += 1;
        cur = tree.get(c).and_then(|n| n.parent);
    }
    d
}

/// Builds a snapshot of `tree` at `at`.
pub fn snapshot(
    tree: &Tree,
    ctx: &ScanContext,
    at: i64,
    kind: SnapshotKind,
    totals: Totals,
) -> Snapshot {
    let root = tree.root();
    let mut candidates: Vec<(NodeId, u64, bool)> = tree
        .iter()
        .filter(|(id, n)| *id != root && n.kind == NodeKind::Folder)
        .filter_map(|(id, n)| {
            let d = depth(tree, id);
            let ruled = n.rule.and_then(|r| ctx.rules.get(r)).is_some();
            (d <= MAX_DEPTH || (ruled && d <= MAX_DEPTH + 1)).then_some((id, n.on_disk, ruled))
        })
        .collect();
    // Rule matches first, then the largest folders, until the limit.
    candidates.sort_by(|a, b| b.2.cmp(&a.2).then(b.1.cmp(&a.1)));
    candidates.truncate(MAX_FOLDERS);
    let mut folders: Vec<Folder> = candidates
        .into_iter()
        .filter_map(|(id, on_disk, _)| {
            tree.get(id).map(|n| Folder {
                path: tree.path(id),
                on_disk,
                files: n.files,
            })
        })
        .collect();
    folders.sort_by(|a, b| a.path.cmp(&b.path));
    Snapshot {
        at,
        kind,
        root: tree.root_path().to_string(),
        totals,
        folders,
    }
}

/// One snapshot as a line of the history file (without the newline).
pub fn to_line(s: &Snapshot) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// The snapshots in the text of a history file, oldest first, and how many lines could not
/// be read. Empty lines do not count.
pub fn parse(text: &str) -> (Vec<Snapshot>, usize) {
    let mut out = vec![];
    let mut skipped = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        match serde_json::from_str::<Snapshot>(line) {
            Ok(s) => out.push(s),
            Err(_) => skipped += 1,
        }
    }
    out.sort_by_key(|s| s.at);
    (out, skipped)
}

/// Thins `list` for storage: every snapshot of the last 90 days, then the last one of each
/// week up to two years, then drops the oldest until the lines fit in `max_bytes`. Returns
/// oldest first.
pub fn retain(mut list: Vec<Snapshot>, now: i64, max_bytes: usize) -> Vec<Snapshot> {
    list.sort_by_key(|s| s.at);
    let mut seen_weeks = HashSet::new();
    // Walk newest first, so the last snapshot of each week wins.
    let mut kept: Vec<Snapshot> = list
        .into_iter()
        .rev()
        .filter(|s| {
            let age = now - s.at;
            if age < DAILY_DAYS * DAY {
                return true;
            }
            age <= KEEP_DAYS * DAY && seen_weeks.insert((s.root.clone(), s.at.div_euclid(7 * DAY)))
        })
        .collect();
    kept.reverse();
    let mut total: usize = kept.iter().map(|s| to_line(s).len() + 1).sum();
    let mut drop = 0;
    while total > max_bytes && drop < kept.len() {
        total -= to_line(&kept[drop]).len() + 1;
        drop += 1;
    }
    kept.split_off(drop)
}

/// The newest snapshot taken at or before `moment`, if any.
pub fn baseline(list: &[Snapshot], moment: i64) -> Option<&Snapshot> {
    list.iter().filter(|s| s.at <= moment).max_by_key(|s| s.at)
}

/// The parent path of `path`, if it has one.
fn parent(path: &str) -> Option<&str> {
    path.rfind('\\').map(|i| &path[..i])
}

/// What changed between `old` and `new`, largest change first. A folder whose change is
/// almost all (80%) inside one of its children gives way to that child, so the list names
/// the cause instead of its parents. Changes under [`MIN_GROWTH`] are left out.
pub fn growth(old: &Snapshot, new: &Snapshot) -> Vec<Growth> {
    let before: HashMap<&str, u64> = old
        .folders
        .iter()
        .map(|f| (f.path.as_str(), f.on_disk))
        .collect();
    let after: HashMap<&str, u64> = new
        .folders
        .iter()
        .map(|f| (f.path.as_str(), f.on_disk))
        .collect();
    let paths: HashSet<&str> = before.keys().chain(after.keys()).copied().collect();
    let delta = |p: &str| -> i64 {
        let a = after.get(p).copied().unwrap_or(0);
        let b = before.get(p).copied().unwrap_or(0);
        i128::from(a)
            .saturating_sub(i128::from(b))
            .clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    };
    // The children of each folder that are in either snapshot.
    let mut children: HashMap<&str, Vec<&str>> = HashMap::new();
    for p in &paths {
        if let Some(par) = parent(p)
            && paths.contains(par)
        {
            children.entry(par).or_default().push(p);
        }
    }
    // A folder stands back when one child holds at least 80% of its change, in the same
    // direction.
    let covered = |p: &str| -> bool {
        let d = delta(p);
        d != 0
            && children.get(p).is_some_and(|kids| {
                kids.iter().any(|k| {
                    let dk = delta(k);
                    dk.signum() == d.signum() && dk.abs() as f64 >= d.abs() as f64 * CAUSE_SHARE
                })
            })
    };
    let mut out: Vec<Growth> = paths
        .iter()
        .filter(|p| delta(p).unsigned_abs() >= MIN_GROWTH && !covered(p))
        .map(|p| Growth {
            path: (*p).to_string(),
            before: before.get(p).copied().unwrap_or(0),
            after: after.get(p).copied().unwrap_or(0),
            delta: delta(p),
        })
        .collect();
    out.sort_by(|a, b| {
        b.delta
            .unsigned_abs()
            .cmp(&a.delta.unsigned_abs())
            .then(a.path.cmp(&b.path))
    });
    out
}

/// Every folder's change between `old` and `new` (no threshold, no attribution), for
/// colouring the map by growth.
pub fn deltas(old: &Snapshot, new: &Snapshot) -> Vec<(String, i64)> {
    let before: HashMap<&str, u64> = old
        .folders
        .iter()
        .map(|f| (f.path.as_str(), f.on_disk))
        .collect();
    new.folders
        .iter()
        .map(|f| {
            let b = before.get(f.path.as_str()).copied();
            let d = match b {
                Some(b) => i128::from(f.on_disk) - i128::from(b),
                None => i128::from(f.on_disk),
            };
            (
                f.path.clone(),
                d.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64,
            )
        })
        .collect()
}

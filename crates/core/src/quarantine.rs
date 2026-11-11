//! The quarantine: move, restore, warn and purge (F5). Risk class Critical.
//!
//! Safety argument, in the order things happen:
//! - **Preview first.** [`plan_move`] and [`plan_purge`] only read identities; they never
//!   write (rule 3.2, T-5.9).
//! - **Refuse what may not move.** Protected paths, links, names that are not valid
//!   Unicode, the scan root and items already in a quarantine are skipped with a reason
//!   (S4). Items inside a cloud sync root go to the cloud quarantine of that sync root,
//!   never to a local one (S3).
//! - **Write ahead.** Before an item moves, its manifest entry (original path, identity,
//!   status `moving`) is written and flushed. If that write fails the move does not start
//!   (S5, rule 3.4). At start-up [`reconcile`] settles entries left in `moving`.
//! - **Verify, then act through the same handle.** The platform opens the item without
//!   following links, compares volume, file ID, size and last write with the scan, and
//!   renames through that handle (rule 3.5). A same-volume rename never copies.
//! - **Restore never overwrites** (rule 3.9) and recreates missing parents.
//! - **Purge only inside a quarantine root, only matching items, only after a warning.**
//!   A batch is purged 30 days after it was made, and only if the companion showed its
//!   warning at least 3 days earlier (S1). Each item is re-verified against its manifest
//!   entry before deletion; links are removed as links (S6).
//! - **Every move, restore and purge appends one audit line** with the operation, IDs
//!   and result, never paths or contents (rule 3.11).

use serde::{Deserialize, Serialize};

use crate::fs::{FileSystem, FsError, Identity, VolumeInfo};
use crate::protect::{ProtectReason, protection};
use crate::roots::{drive_of, file_name, is_within, join, parent_of, same_path, split_ext};
use crate::rules::Tier;
use crate::scan::ScanContext;
use crate::time::{fmt_date, fmt_iso};
use crate::tree::{FLAG_LOSSY_NAME, NodeId, NodeKind, Tree};

/// The manifest file in every batch folder.
pub const MANIFEST_NAME: &str = "batch.unhoard-manifest.json";
/// The read-me file in every batch folder.
pub const README_NAME: &str = "READ ME - where these came from.txt";
/// Days a batch stays before the purge.
pub const RETENTION_DAYS: i64 = 30;
/// Days before the purge date at which the companion warns.
pub const WARNING_DAYS: [i64; 3] = [5, 3, 1];
/// The purge waits at least this many days after a shown warning.
pub const WARNING_LEAD_DAYS: i64 = 3;
const DAY: i64 = 86_400;
const MANIFEST_MAX: usize = 16 << 20;

/// Where an item is in its life in the quarantine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemStatus {
    /// The move is about to happen or was interrupted.
    Moving,
    /// The item is in the quarantine.
    Quarantined,
    /// The item is back where it was (or was never moved).
    Restored,
    /// The user deleted it from the quarantine in Explorer.
    DeletedByUser,
    /// The 30-day purge deleted it.
    Purged,
}

/// One item of a batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestItem {
    /// Unique ID of the item.
    pub id: String,
    /// ID of the operation that moved it.
    pub operation_id: String,
    /// Where it came from.
    pub original_path: String,
    /// Where it is now.
    pub quarantine_path: String,
    /// File, folder or link.
    pub kind: NodeKind,
    /// Logical size.
    pub logical_bytes: u64,
    /// Size on disk.
    pub on_disk_bytes: u64,
    /// Number of files.
    pub file_count: u64,
    /// Identity, with timestamps and attributes, checked before every later action.
    pub identity: Identity,
    /// The known-location rule that suggested it.
    pub rule_id: Option<String>,
    /// Its tier when it was moved.
    pub tier: Option<Tier>,
    /// True for an item in a cloud quarantine.
    pub cloud: bool,
    /// Its status.
    pub status: ItemStatus,
}

/// A purge warning the companion showed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarningShown {
    /// Days left when it was shown (5, 3 or 1).
    pub days_left: i64,
    /// When it was shown (seconds since 1970).
    pub at: i64,
}

/// The manifest of one batch.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Format version.
    pub schema_version: u32,
    /// Batch ID.
    pub batch_id: String,
    /// When the batch was made (ISO 8601).
    pub created_at: String,
    /// When the batch was made (seconds since 1970).
    pub created_unix: i64,
    /// When the batch may be purged (ISO 8601).
    pub purge_after: String,
    /// When the batch may be purged (seconds since 1970).
    pub purge_after_unix: i64,
    /// Warnings shown for this batch.
    pub warnings_shown: Vec<WarningShown>,
    /// The items.
    pub items: Vec<ManifestItem>,
}

/// A batch folder with its manifest.
#[derive(Clone, Debug)]
pub struct Batch {
    /// The batch folder.
    pub dir: String,
    /// Its manifest.
    pub manifest: Manifest,
}

/// Why an item is not part of a move.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum SkipReason {
    /// The path is protected (S4).
    Protected(ProtectReason),
    /// It changed since the scan; rescan first.
    Changed,
    /// It is a link; links take no space and are never moved.
    Link,
    /// Its name is not valid Unicode.
    UnsupportedName,
    /// A selected folder already contains it.
    InsideSelection,
    /// It no longer exists or could not be read.
    Unavailable(FsError),
}

/// An item left out of a move, with the reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    /// The item's path.
    pub path: String,
    /// Why.
    pub reason: SkipReason,
}

/// One item the move will handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedMove {
    /// The scan node.
    pub node: NodeId,
    /// Its path.
    pub path: String,
    /// The quarantine root it goes to.
    pub dest_root: String,
    /// Its identity at the time of the preview.
    pub identity: Identity,
    /// File, folder or link.
    pub kind: NodeKind,
    /// Logical size.
    pub logical: u64,
    /// Size on disk.
    pub on_disk: u64,
    /// Number of files.
    pub files: u64,
    /// The suggesting rule.
    pub rule_id: Option<String>,
    /// The tier.
    pub tier: Option<Tier>,
    /// True for a cloud item.
    pub cloud: bool,
}

/// The preview of a move: what will happen, before anything happens.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePlan {
    /// Items that will move.
    pub moves: Vec<PlannedMove>,
    /// Items left out, with the reason.
    pub skipped: Vec<Skipped>,
    /// Total logical size of the items that will move.
    pub total_logical: u64,
    /// Total size on disk.
    pub total_on_disk: u64,
    /// The quarantine roots the items go to.
    pub destinations: Vec<String>,
}

/// What happened to one item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "outcome")]
pub enum Outcome {
    /// Moved into the quarantine.
    Moved {
        /// Its new path.
        to: String,
    },
    /// Put back.
    Restored {
        /// Where it went.
        to: String,
        /// True if the original path existed and the item got a `(restored)` name.
        renamed: bool,
    },
    /// Failed; nothing happened to this item.
    Failed(FsError),
}

/// The result for one item.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    /// The item's original path.
    pub path: String,
    /// What happened.
    pub outcome: Outcome,
}

/// The report of a move.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveReport {
    /// The batch folders that were made.
    pub batch_dirs: Vec<String>,
    /// One result per planned item.
    pub results: Vec<ItemResult>,
    /// Size on disk that moved (not freed yet on the same drive).
    pub moved_on_disk: u64,
}

/// The preview of a purge.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgePlan {
    /// Items that would be deleted: batch ID, item ID, size on disk.
    pub items: Vec<(String, String, u64)>,
    /// Batches past their date that still wait for a warning.
    pub waiting_for_warning: Vec<String>,
    /// Items that no longer match their manifest entry and would be left alone.
    pub mismatched: Vec<(String, String)>,
}

/// The report of a purge.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    /// Items deleted.
    pub purged: usize,
    /// Size on disk freed.
    pub freed_on_disk: u64,
    /// Items left alone because they changed.
    pub mismatched: usize,
    /// Items that failed, with the error.
    pub failed: Vec<(String, FsError)>,
}

/// A purge warning that is due.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    /// The batch.
    pub batch_id: String,
    /// The batch folder.
    pub dir: String,
    /// The threshold to show: 5, 3 or 1.
    pub days_left: i64,
    /// Size on disk that will be purged.
    pub on_disk_bytes: u64,
    /// When the purge may happen.
    pub purge_after_unix: i64,
}

/// Free space as Unhoard reports it for a drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveSpace {
    /// Free space as Windows measures it; a quarantine move never raises it (S10).
    pub free_bytes: u64,
    /// Size on disk of items in the quarantine on this drive.
    pub not_freed_bytes: u64,
}

fn audit(
    fs: &dyn FileSystem,
    log: &str,
    now: i64,
    op: &str,
    batch: &str,
    item: &str,
    result: &Result<(), FsError>,
) {
    let line = serde_json::json!({
        "time": fmt_iso(now),
        "op": op,
        "batchId": batch,
        "itemId": item,
        "result": match result { Ok(()) => "ok".to_string(), Err(e) => format!("{e:?}") },
    });
    // The audit log is a record, not a gate: a failed append never blocks a restore. It is
    // created on first use; a missing parent folder is created by the app at start.
    let _ignored_on_purpose = fs.append_line(log, &line.to_string());
}

/// Writes a batch's manifest durably.
pub fn save(fs: &dyn FileSystem, batch: &Batch) -> Result<(), FsError> {
    let bytes = serde_json::to_vec_pretty(&batch.manifest).map_err(|_| FsError::Unsupported {
        reason: "manifest".into(),
    })?;
    fs.write_durable(&join(&batch.dir, MANIFEST_NAME), &bytes)
}

/// Previews a move of the given scan nodes. Reads identities only; writes nothing.
pub fn plan_move(fs: &dyn FileSystem, tree: &Tree, ids: &[NodeId], ctx: &ScanContext) -> MovePlan {
    let mut plan = MovePlan::default();
    // First the checks that do not touch the disk, so a refused folder never hides the
    // items selected inside it.
    let mut candidates = vec![];
    for &id in ids {
        let Some(node) = tree.get(id) else { continue };
        let path = tree.path(id);
        let reason = if id == tree.root() {
            Some(SkipReason::Protected(ProtectReason::DriveRoot))
        } else if let Some(r) = protection(&path, &ctx.roots) {
            Some(SkipReason::Protected(r))
        } else if node.kind == NodeKind::Link {
            Some(SkipReason::Link)
        } else if node.flags & FLAG_LOSSY_NAME != 0 {
            Some(SkipReason::UnsupportedName)
        } else {
            None
        };
        match reason {
            Some(reason) => plan.skipped.push(Skipped { path, reason }),
            None => candidates.push(id),
        }
    }
    for &id in &candidates {
        let Some(node) = tree.get(id) else { continue };
        let path = tree.path(id);
        let skip = |reason| Skipped {
            path: path.clone(),
            reason,
        };
        if candidates
            .iter()
            .any(|&other| other != id && tree.is_ancestor(other, id))
        {
            plan.skipped.push(skip(SkipReason::InsideSelection));
            continue;
        }
        let identity = match fs.identity(&path) {
            Ok(i) => i,
            Err(e) => {
                plan.skipped.push(skip(SkipReason::Unavailable(e)));
                continue;
            }
        };
        let scanned_size = if node.kind == NodeKind::File {
            node.logical
        } else {
            0
        };
        if identity.file_id != node.file_id
            || identity.modified != node.modified
            || identity.size != scanned_size
        {
            plan.skipped.push(skip(SkipReason::Changed));
            continue;
        }
        let Some(dest_root) = ctx.roots.quarantine_root_for(&path) else {
            plan.skipped
                .push(skip(SkipReason::Unavailable(FsError::Unsupported {
                    reason: "no drive".into(),
                })));
            continue;
        };
        let rule = node.rule.and_then(|r| ctx.rules.get(r));
        plan.total_logical += node.logical;
        plan.total_on_disk += node.on_disk;
        if !plan.destinations.iter().any(|d| same_path(d, &dest_root)) {
            plan.destinations.push(dest_root.clone());
        }
        plan.moves.push(PlannedMove {
            node: id,
            cloud: ctx.roots.sync_root_of(&path).is_some(),
            path,
            dest_root,
            identity,
            kind: node.kind,
            logical: node.logical,
            on_disk: node.on_disk,
            files: node.files,
            rule_id: rule.map(|r| r.id.clone()),
            tier: rule.and_then(|r| r.opinion),
        });
    }
    plan
}

/// A name that does not exist in `dir` yet: `name`, `name (2).ext`, `name (3).ext`, …
fn free_name(fs: &dyn FileSystem, dir: &str, name: &str, taken: &[String]) -> String {
    let (stem, ext) = split_ext(name);
    let mut candidate = name.to_string();
    let mut n = 2;
    while fs.exists(&join(dir, &candidate))
        || taken.iter().any(|t| t.eq_ignore_ascii_case(&candidate))
    {
        candidate = format!("{stem} ({n}){ext}");
        n += 1;
    }
    candidate
}

fn new_manifest(now: i64, batch_id: String) -> Manifest {
    let purge = now + RETENTION_DAYS * DAY;
    Manifest {
        schema_version: 1,
        batch_id,
        created_at: fmt_iso(now),
        created_unix: now,
        purge_after: fmt_iso(purge),
        purge_after_unix: purge,
        warnings_shown: vec![],
        items: vec![],
    }
}

/// Carries out a previewed move. Each item is verified and renamed on its own; a failure
/// affects only that item. `readme` writes the localised read-me of a batch.
pub fn execute_move(
    fs: &dyn FileSystem,
    plan: &MovePlan,
    readme: &dyn Fn(&Manifest) -> String,
    audit_log: &str,
    now: i64,
) -> MoveReport {
    let mut report = MoveReport::default();
    for root in &plan.destinations {
        let moves: Vec<&PlannedMove> = plan
            .moves
            .iter()
            .filter(|m| same_path(&m.dest_root, root))
            .collect();
        let fail_all = |report: &mut MoveReport, e: FsError| {
            for m in &moves {
                report.results.push(ItemResult {
                    path: m.path.clone(),
                    outcome: Outcome::Failed(e.clone()),
                });
            }
        };
        let cloud = moves.iter().any(|m| m.cloud);
        if !fs.exists(root) {
            let made = if cloud {
                fs.create_dir_all(root)
            } else {
                fs.create_private_dir(root)
            };
            if let Err(e) = made {
                fail_all(&mut report, e);
                continue;
            }
        }
        let base = format!(
            "{} \u{b7} purged after {}",
            fmt_date(now),
            fmt_date(now + RETENTION_DAYS * DAY)
        );
        let dir_name = free_name(fs, root, &base, &[]);
        let dir = join(root, &dir_name);
        if let Err(e) = fs.create_dir_all(&dir) {
            fail_all(&mut report, e);
            continue;
        }
        let mut batch = Batch {
            dir: dir.clone(),
            manifest: new_manifest(now, format!("b{now}-{}", report.batch_dirs.len())),
        };
        if let Err(e) = save(fs, &batch) {
            fail_all(&mut report, e);
            let _cleanup = fs.delete_tree(&dir);
            continue;
        }
        let mut taken: Vec<String> = vec![MANIFEST_NAME.into(), README_NAME.into()];
        for (i, m) in moves.iter().enumerate() {
            let name = free_name(fs, &dir, file_name(&m.path), &taken);
            taken.push(name.clone());
            let item = ManifestItem {
                id: format!("{}-{i}", batch.manifest.batch_id),
                operation_id: format!("move-{now}"),
                original_path: m.path.clone(),
                quarantine_path: join(&dir, &name),
                kind: m.kind,
                logical_bytes: m.logical,
                on_disk_bytes: m.on_disk,
                file_count: m.files,
                identity: m.identity,
                rule_id: m.rule_id.clone(),
                tier: m.tier,
                cloud: m.cloud,
                status: ItemStatus::Moving,
            };
            batch.manifest.items.push(item.clone());
            // Write ahead: the entry is on disk before the move starts (S5).
            if let Err(e) = save(fs, &batch) {
                batch.manifest.items.pop();
                report.results.push(ItemResult {
                    path: m.path.clone(),
                    outcome: Outcome::Failed(e),
                });
                continue;
            }
            let result = fs.move_verified(&m.path, &m.identity, &item.quarantine_path);
            match &result {
                Ok(()) => {
                    if let Some(last) = batch.manifest.items.last_mut() {
                        last.status = ItemStatus::Quarantined;
                    }
                    // If this write fails the entry stays `moving` and start-up reconciles it.
                    let _reconciled_at_start = save(fs, &batch);
                    report.moved_on_disk += m.on_disk;
                    report.results.push(ItemResult {
                        path: m.path.clone(),
                        outcome: Outcome::Moved {
                            to: item.quarantine_path.clone(),
                        },
                    });
                }
                Err(e) => {
                    batch.manifest.items.pop();
                    let _entry_removed = save(fs, &batch);
                    report.results.push(ItemResult {
                        path: m.path.clone(),
                        outcome: Outcome::Failed(e.clone()),
                    });
                }
            }
            audit(
                fs,
                audit_log,
                now,
                "move",
                &batch.manifest.batch_id,
                &item.id,
                &result,
            );
        }
        if batch.manifest.items.is_empty() {
            let _empty_batch = fs.delete_tree(&dir);
            continue;
        }
        let _readme_is_a_courtesy =
            fs.write_durable(&join(&dir, README_NAME), readme(&batch.manifest).as_bytes());
        report.batch_dirs.push(dir);
    }
    report
}

/// Reads every batch in the given quarantine roots. Folders without a readable manifest
/// are left alone.
pub fn load_batches(fs: &dyn FileSystem, roots: &[String]) -> Vec<Batch> {
    let mut out = vec![];
    for root in roots {
        let Ok(entries) = fs.read_dir(root) else {
            continue;
        };
        for e in entries.iter().filter(|e| e.is_dir && !e.is_link()) {
            let dir = join(root, &e.name);
            let Ok(bytes) = fs.read_small(&join(&dir, MANIFEST_NAME), MANIFEST_MAX) else {
                continue;
            };
            if let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) {
                out.push(Batch { dir, manifest });
            }
        }
    }
    out.sort_by_key(|b| b.manifest.created_unix);
    out
}

/// Marks quarantined items the user deleted in Explorer as `deletedByUser`. Returns true
/// if anything changed (and was saved).
pub fn refresh(fs: &dyn FileSystem, batch: &mut Batch) -> bool {
    let mut changed = false;
    for item in batch
        .manifest
        .items
        .iter_mut()
        .filter(|i| i.status == ItemStatus::Quarantined)
    {
        if !fs.exists(&item.quarantine_path) {
            item.status = ItemStatus::DeletedByUser;
            changed = true;
        }
    }
    if changed {
        let _shown_next_time = save(fs, batch);
    }
    changed
}

fn identity_matches(fs: &dyn FileSystem, path: &str, expected: &Identity) -> Result<bool, FsError> {
    let now = fs.identity(path)?;
    Ok(now.file_id == expected.file_id
        && now.size == expected.size
        && now.modified == expected.modified
        && now.volume_serial == expected.volume_serial)
}

/// Settles entries left in `moving` by a crash: the item is either in the quarantine
/// (`quarantined`) or still at its original path (`restored`). Returns how many changed.
pub fn reconcile(fs: &dyn FileSystem, batch: &mut Batch, audit_log: &str, now: i64) -> usize {
    let mut fixed = 0;
    for item in batch
        .manifest
        .items
        .iter_mut()
        .filter(|i| i.status == ItemStatus::Moving)
    {
        if identity_matches(fs, &item.quarantine_path, &item.identity).unwrap_or(false) {
            item.status = ItemStatus::Quarantined;
        } else if identity_matches(fs, &item.original_path, &item.identity).unwrap_or(false) {
            item.status = ItemStatus::Restored;
        } else {
            continue;
        }
        fixed += 1;
        audit(
            fs,
            audit_log,
            now,
            "reconcile",
            &batch.manifest.batch_id,
            &item.id,
            &Ok(()),
        );
    }
    if fixed > 0 {
        let _retried_next_start = save(fs, batch);
    }
    fixed
}

/// The path to restore to: the original, or `name (restored).ext` next to it if the
/// original exists, so nothing is ever overwritten.
fn restore_target(fs: &dyn FileSystem, original: &str) -> (String, bool) {
    if !fs.exists(original) {
        return (original.to_string(), false);
    }
    let dir = parent_of(original).unwrap_or_default();
    let (stem, ext) = split_ext(file_name(original));
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{stem} (restored){ext}")
        } else {
            format!("{stem} (restored {n}){ext}")
        };
        let p = join(&dir, &name);
        if !fs.exists(&p) {
            return (p, true);
        }
        n += 1;
    }
}

/// Puts the given items of a batch back where they came from.
pub fn restore(
    fs: &dyn FileSystem,
    batch: &mut Batch,
    item_ids: &[String],
    audit_log: &str,
    now: i64,
) -> Vec<ItemResult> {
    let mut results = vec![];
    let batch_id = batch.manifest.batch_id.clone();
    for item in batch
        .manifest
        .items
        .iter_mut()
        .filter(|i| item_ids.contains(&i.id) && i.status == ItemStatus::Quarantined)
    {
        let (target, renamed) = restore_target(fs, &item.original_path);
        let result = match parent_of(&target) {
            Some(parent) => fs
                .create_dir_all(&parent)
                .and_then(|()| fs.move_verified(&item.quarantine_path, &item.identity, &target)),
            None => Err(FsError::Unsupported {
                reason: "no parent folder".into(),
            }),
        };
        audit(fs, audit_log, now, "restore", &batch_id, &item.id, &result);
        match result {
            Ok(()) => {
                item.status = ItemStatus::Restored;
                results.push(ItemResult {
                    path: item.original_path.clone(),
                    outcome: Outcome::Restored {
                        to: target,
                        renamed,
                    },
                });
            }
            Err(FsError::NotFound) if !fs.exists(&item.quarantine_path) => {
                item.status = ItemStatus::DeletedByUser;
                results.push(ItemResult {
                    path: item.original_path.clone(),
                    outcome: Outcome::Failed(FsError::NotFound),
                });
            }
            Err(e) => results.push(ItemResult {
                path: item.original_path.clone(),
                outcome: Outcome::Failed(e),
            }),
        }
    }
    let _ = save(fs, batch);
    if batch
        .manifest
        .items
        .iter()
        .all(|i| i.status != ItemStatus::Quarantined && i.status != ItemStatus::Moving)
    {
        let _emptied_batch = fs.delete_tree(&batch.dir);
    }
    results
}

fn purge_ready(m: &Manifest, now: i64) -> bool {
    now >= m.purge_after_unix
        && m.warnings_shown
            .iter()
            .any(|w| w.at <= now - WARNING_LEAD_DAYS * DAY)
}

fn in_any_quarantine_root(path: &str, batch: &Batch) -> bool {
    // The batch folder sits directly in a quarantine root; the item must sit inside it.
    is_within(path, &batch.dir) && !same_path(path, &batch.dir)
}

/// Previews the purge. Reads identities only; writes nothing.
pub fn plan_purge(fs: &dyn FileSystem, batches: &[Batch], now: i64) -> PurgePlan {
    let mut plan = PurgePlan::default();
    for b in batches {
        if now < b.manifest.purge_after_unix {
            continue;
        }
        if !purge_ready(&b.manifest, now) {
            plan.waiting_for_warning.push(b.manifest.batch_id.clone());
            continue;
        }
        for i in b
            .manifest
            .items
            .iter()
            .filter(|i| i.status == ItemStatus::Quarantined)
        {
            match identity_matches(fs, &i.quarantine_path, &i.identity) {
                Ok(true) if in_any_quarantine_root(&i.quarantine_path, b) => {
                    plan.items
                        .push((b.manifest.batch_id.clone(), i.id.clone(), i.on_disk_bytes))
                }
                Err(FsError::NotFound) => {}
                _ => plan
                    .mismatched
                    .push((b.manifest.batch_id.clone(), i.id.clone())),
            }
        }
    }
    plan
}

/// Permanently deletes batches that are past their date and were warned about at least
/// three days earlier. The only automatic action in Unhoard.
pub fn execute_purge(
    fs: &dyn FileSystem,
    batches: &mut [Batch],
    now: i64,
    audit_log: &str,
) -> PurgeReport {
    let mut report = PurgeReport::default();
    for b in batches.iter_mut().filter(|b| purge_ready(&b.manifest, now)) {
        let batch_id = b.manifest.batch_id.clone();
        let dir = b.dir.clone();
        for i in b
            .manifest
            .items
            .iter_mut()
            .filter(|i| i.status == ItemStatus::Quarantined)
        {
            if !is_within(&i.quarantine_path, &dir) || same_path(&i.quarantine_path, &dir) {
                report.mismatched += 1;
                continue;
            }
            match identity_matches(fs, &i.quarantine_path, &i.identity) {
                Err(FsError::NotFound) => {
                    i.status = ItemStatus::DeletedByUser;
                    continue;
                }
                Ok(true) => {}
                _ => {
                    report.mismatched += 1;
                    continue;
                }
            }
            let result = fs.delete_verified(&i.quarantine_path, &i.identity);
            audit(fs, audit_log, now, "purge", &batch_id, &i.id, &result);
            match result {
                Ok(()) => {
                    i.status = ItemStatus::Purged;
                    report.purged += 1;
                    report.freed_on_disk += i.on_disk_bytes;
                }
                Err(FsError::Changed) => report.mismatched += 1,
                Err(e) => report.failed.push((i.id.clone(), e)),
            }
        }
        let done = b.manifest.items.iter().all(|i| {
            matches!(
                i.status,
                ItemStatus::Restored | ItemStatus::DeletedByUser | ItemStatus::Purged
            )
        });
        if done {
            let _emptied_batch = fs.delete_tree(&b.dir);
        } else {
            let _ = save(fs, b);
        }
    }
    report
}

/// Purge warnings that are due: per batch, the most urgent threshold (5, 3 or 1 days)
/// that was reached and not shown yet.
pub fn warnings_due(batches: &[Batch], now: i64) -> Vec<Warning> {
    let mut out = vec![];
    for b in batches {
        let pending: u64 = b
            .manifest
            .items
            .iter()
            .filter(|i| i.status == ItemStatus::Quarantined)
            .map(|i| i.on_disk_bytes.max(1))
            .sum();
        if pending == 0 {
            continue;
        }
        let left = (b.manifest.purge_after_unix - now + DAY - 1)
            .div_euclid(DAY)
            .max(0);
        let due =
            WARNING_DAYS.iter().rev().copied().find(|&t| {
                left <= t && !b.manifest.warnings_shown.iter().any(|w| w.days_left <= t)
            });
        if let Some(days_left) = due {
            out.push(Warning {
                batch_id: b.manifest.batch_id.clone(),
                dir: b.dir.clone(),
                days_left,
                on_disk_bytes: pending,
                purge_after_unix: b.manifest.purge_after_unix,
            });
        }
    }
    out
}

/// Records that the companion showed the warning for `days_left`, and saves it.
pub fn record_warning(
    fs: &dyn FileSystem,
    batch: &mut Batch,
    days_left: i64,
    now: i64,
) -> Result<(), FsError> {
    batch
        .manifest
        .warnings_shown
        .push(WarningShown { days_left, at: now });
    save(fs, batch)
}

/// Free space and quarantined space of a drive. Free space is what Windows reports; a
/// quarantine move on the same drive is shown as "not freed yet", never as free (S10).
pub fn drive_space(volume: &VolumeInfo, batches: &[Batch], drive: &str) -> DriveSpace {
    let not_freed = batches
        .iter()
        .flat_map(|b| b.manifest.items.iter())
        .filter(|i| {
            i.status == ItemStatus::Quarantined
                && drive_of(&i.quarantine_path).is_some_and(|d| d.eq_ignore_ascii_case(drive))
        })
        .map(|i| i.on_disk_bytes)
        .sum();
    DriveSpace {
        free_bytes: volume.free_bytes,
        not_freed_bytes: not_freed,
    }
}

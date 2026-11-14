//! The app service: scan sessions, views, quarantine, cloud and photos, on top of the core.
//!
//! Everything the webview can trigger goes through here, and nothing here trusts it: items
//! are addressed by scan-index ID plus the scan generation, never by path; unknown and
//! stale IDs are refused (rule 2.2, T-5.16); every destructive step re-plans and re-checks
//! the item on disk before acting. The service works on any [`FileSystem`], so the tests
//! run it on the in-memory fake.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use crate::history::HistoryStore;
use serde::{Deserialize, Serialize};
use unhoard_core::classify::{Description, describe};
use unhoard_core::cloud::{
    CloudSummary, FreeUpPlan, apply_free_up, plan_free_up, remeasure, summary,
};
use unhoard_core::explain::{Explain, explain, place_of};
use unhoard_core::fs::{FileSystem, FsError, VolumeInfo};
use unhoard_core::hidden::hidden_bytes;
use unhoard_core::history::{
    DAY, Snapshot, SnapshotKind, Totals, baseline, deltas, growth, snapshot,
};
use unhoard_core::knowledge::{Knowledge, SteamInfo};
use unhoard_core::photos::{
    Features, Group, MediaItem, bursts, by_month, cloud_copies, cloud_duplicates,
    exact_duplicates_local, feature_candidates, media_index, near_duplicates, suggest_keepers,
    video_duplicates,
};
use unhoard_core::quarantine::{
    self, Batch, DriveSpace, ItemResult, ItemStatus, Manifest, MovePlan, MoveReport, Outcome,
    PurgeReport, Warning, drive_space, execute_move, execute_purge, load_batches, plan_move,
    reconcile, record_warning, refresh, restore, warnings_due,
};
use unhoard_core::roots::{Roots, drive_of};
use unhoard_core::scan::{ScanContext, ScanControl, scan_into};
use unhoard_core::tree::{FLAG_UNSCANNED, NodeId, NodeKind, Tree};
use unhoard_core::view::{
    MatrixPoint, Suggestion, View, ViewNode, matrix, node_view, suggestions, view,
};

/// An error a command returns to the UI: a code the UI translates, never internals.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "code")]
pub enum CmdError {
    /// No scan has run yet.
    NoScan,
    /// The IDs belong to an earlier scan.
    StaleScan,
    /// An ID is not in the scan.
    UnknownItem {
        /// The ID.
        id: u32,
    },
    /// An argument failed validation.
    InvalidArgument {
        /// Which argument.
        what: String,
    },
    /// A scan is still running.
    Busy,
    /// A file-system error.
    Fs {
        /// The error.
        error: FsError,
    },
}

impl From<FsError> for CmdError {
    fn from(error: FsError) -> Self {
        CmdError::Fs { error }
    }
}

/// The result of changing pin states: one entry per file.
pub type PinResults = Vec<(String, Result<(), FsError>)>;

/// Provides photo and video features without downloads (the platform, or a fake in tests).
pub trait MediaSource: Send + Sync {
    /// Thumbnail hash, sharpness, pixel size and duration.
    fn features(&self, path: &str, video: bool, in_cloud: bool) -> Features;
}

/// One scan and its live tree.
pub struct Session {
    /// Increases with every scan; IDs from another generation are stale.
    pub generation: u64,
    /// The scanned folder or drive.
    pub root: String,
    /// True for a whole-drive scan (hidden space applies).
    pub drive_scan: bool,
    /// The live tree.
    pub tree: Arc<Mutex<Tree>>,
    /// Cancellation and progress.
    pub control: Arc<ScanControl>,
    /// Set when the scan ended.
    pub done: Arc<AtomicBool>,
    /// The volume, read at the start.
    pub volume: Option<VolumeInfo>,
    /// When the scan started.
    pub started: Instant,
    /// How long it took, once done.
    pub duration_ms: Arc<AtomicU64>,
}

/// Local pictures all get a thumbnail for near-duplicate matching up to this many; a larger
/// local library is matched on the groups that need no thumbnail.
const LOCAL_FEATURE_LIMIT: usize = 5_000;
/// Cloud files of the same size that only a thumbnail can tell apart are asked up to this
/// many; a thumbnail of an online-only file is rarely in the cache, and each lookup takes
/// about ten milliseconds.
const CLOUD_FEATURE_LIMIT: usize = 2_000;
/// The photo analysis reports its progress once per this many items.
const PHOTO_PROGRESS_EVERY: usize = 50;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// A view plus what only the app knows: hidden space and scan state.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewResponse {
    /// The scan generation the IDs belong to.
    pub generation: u64,
    /// The view.
    pub view: View,
    /// Hidden space, on the root of a finished whole-drive scan.
    pub hidden_bytes: Option<u64>,
    /// True while the scan runs.
    pub scanning: bool,
}

/// Everything the details panel shows about one item.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    /// The item.
    pub node: ViewNode,
    /// Its full path (shown to the user on this PC only; never exported).
    pub path: String,
    /// Tier, actions and protection.
    pub description: Description,
    /// Cloud sizes for an item in a sync root.
    pub cloud: Option<CloudSummary>,
    /// For a folder read with admin rights: its largest children and their sizes.
    pub measured: Option<Vec<(String, u64)>>,
}

/// What the elevated helper measured in one folder.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    /// The folder.
    pub path: String,
    /// Logical size.
    pub logical: u64,
    /// Size on disk.
    pub on_disk: u64,
    /// Number of files.
    pub files: u64,
    /// Its largest children.
    pub children: Vec<(String, u64)>,
}

/// What reading with admin rights added to the scan.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasuredSummary {
    /// Folders that now have a size.
    pub folders: u32,
    /// Their total size on disk.
    pub on_disk: u64,
}

/// A folder's largest children: name and size on disk.
pub type Children = Vec<(String, u64)>;

/// Gathers what is known about this PC besides the files, at the start of every scan.
pub type Gather = Arc<dyn Fn(&Roots) -> Knowledge + Send + Sync>;

/// One batch for the Quarantine view.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchView {
    /// The batch folder.
    pub dir: String,
    /// The manifest.
    pub manifest: Manifest,
}

/// The Quarantine view.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuarantineState {
    /// All batches, oldest first.
    pub batches: Vec<BatchView>,
    /// Purge warnings that are due.
    pub warnings: Vec<Warning>,
    /// Free and not-yet-freed space per drive.
    pub space: Vec<(String, DriveSpace)>,
}

/// A cloud sync root and, if it was scanned, its sizes.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudRoot {
    /// The sync root.
    pub path: String,
    /// Its node in the current scan.
    pub id: Option<u32>,
    /// Cloud versus local size.
    pub summary: Option<CloudSummary>,
}

/// Photo review results.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoState {
    /// The scan generation.
    pub generation: u64,
    /// Every photo and video.
    pub media: Vec<MediaItem>,
    /// The groups to review.
    pub groups: Vec<Group>,
}

/// The app service.
pub struct Service {
    fs: Arc<dyn FileSystem>,
    /// The context of the current scan; every scan gets a fresh one with fresh knowledge.
    ctx: Arc<Mutex<Arc<ScanContext>>>,
    gather: Gather,
    /// Children of folders read with admin rights, by scan generation and node.
    measured: Mutex<HashMap<(u64, u32), Children>>,
    media: Arc<dyn MediaSource>,
    audit_log: String,
    quarantine_roots: Vec<String>,
    session: Mutex<Option<Arc<Session>>>,
    generation: AtomicU64,
    readme: fn(&Manifest) -> String,
    photos: Mutex<Option<PhotoState>>,
    history: Option<Arc<HistoryStore>>,
}

/// One point of the storage-history timeline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePoint {
    /// When, in seconds since 1970.
    pub at: i64,
    /// A scan or the daily measurement.
    pub kind: SnapshotKind,
    /// The drive's totals.
    pub totals: Totals,
}

/// A folder that grew or shrank, with what is known about it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrowthItem {
    /// Its full path.
    pub path: String,
    /// Its name.
    pub name: String,
    /// Size on disk at the baseline.
    pub before: u64,
    /// Size on disk now.
    pub after: u64,
    /// `after - before`.
    pub delta: i64,
    /// Its scan-index ID in the current scan, when the scan has it.
    pub id: Option<u32>,
    /// Its known-location rule or definition.
    pub rule_id: Option<String>,
    /// The facts its explanation is built from.
    pub explain: Explain,
    /// Its size in every snapshot of the period, oldest first, for a sparkline.
    pub trend: Vec<(i64, u64)>,
}

/// What changed in a period.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrowthView {
    /// The baseline's time, if there is one.
    pub from: Option<i64>,
    /// The newest snapshot's time.
    pub to: Option<i64>,
    /// The largest changes, largest first.
    pub items: Vec<GrowthItem>,
    /// Every folder's change, for colouring the map.
    pub deltas: Vec<(String, i64)>,
}

/// The largest growers a view returns.
const GROWTH_ITEMS: usize = 40;

impl Service {
    /// Creates the service.
    pub fn new(
        fs: Arc<dyn FileSystem>,
        ctx: ScanContext,
        media: Arc<dyn MediaSource>,
        audit_log: String,
        quarantine_roots: Vec<String>,
        readme: fn(&Manifest) -> String,
    ) -> Self {
        Self {
            fs,
            ctx: Arc::new(Mutex::new(Arc::new(ctx))),
            gather: Arc::new(|_| Knowledge::default()),
            measured: Mutex::new(HashMap::new()),
            media,
            audit_log,
            quarantine_roots,
            session: Mutex::new(None),
            generation: AtomicU64::new(0),
            readme,
            photos: Mutex::new(None),
            history: None,
        }
    }

    /// Keeps a storage-history snapshot of every finished drive scan in `store`.
    pub fn with_history(mut self, store: Arc<HistoryStore>) -> Self {
        self.history = Some(store);
        self
    }

    /// The timeline of the drive `root` (`C:\`), oldest first.
    pub fn history_timeline(&self, root: &str) -> Vec<TimelinePoint> {
        let Some(store) = &self.history else {
            return vec![];
        };
        store
            .load()
            .0
            .into_iter()
            .filter(|s| s.root.eq_ignore_ascii_case(root))
            .map(|s| TimelinePoint {
                at: s.at,
                kind: s.kind,
                totals: s.totals,
            })
            .collect()
    }

    /// What changed on the drive `root` over the last `days` days.
    pub fn history_growth(&self, root: &str, days: u32) -> GrowthView {
        let empty = GrowthView {
            from: None,
            to: None,
            items: vec![],
            deltas: vec![],
        };
        let Some(store) = &self.history else {
            return empty;
        };
        let list: Vec<Snapshot> = store
            .load()
            .0
            .into_iter()
            .filter(|s| s.root.eq_ignore_ascii_case(root))
            .collect();
        let Some(newest) = list.last() else {
            return empty;
        };
        let moment = newest.at - i64::from(days) * DAY;
        let Some(base) = baseline(&list, moment).or_else(|| list.first()) else {
            return empty;
        };
        if std::ptr::eq(base, newest) {
            return GrowthView {
                to: Some(newest.at),
                ..empty
            };
        }
        let ctx = self.context();
        let session = self.current().ok();
        let tree = session.as_ref().map(|s| lock(&s.tree));
        let items = growth(base, newest)
            .into_iter()
            .take(GROWTH_ITEMS)
            .map(|g| {
                let id = tree.as_ref().and_then(|t| t.find(&g.path));
                let rule_id = id
                    .and_then(|i| tree.as_ref().and_then(|t| t.get(i)))
                    .and_then(|n| n.rule)
                    .and_then(|r| ctx.rules.get(r))
                    .map(|r| r.id.clone());
                let facts = match (id, tree.as_ref()) {
                    (Some(i), Some(t)) => explain(t, i, &ctx.roots),
                    _ => {
                        let (place, owner) = place_of(&g.path, &ctx.roots);
                        Explain {
                            place,
                            owner,
                            ..Explain::default()
                        }
                    }
                };
                let trend = list
                    .iter()
                    .filter(|s| s.at >= base.at)
                    .map(|s| {
                        let size = s
                            .folders
                            .iter()
                            .find(|f| f.path == g.path)
                            .map(|f| f.on_disk)
                            .unwrap_or(0);
                        (s.at, size)
                    })
                    .collect();
                GrowthItem {
                    name: unhoard_core::roots::file_name(&g.path).to_string(),
                    path: g.path,
                    before: g.before,
                    after: g.after,
                    delta: g.delta,
                    id: id.map(|i| i.0),
                    rule_id,
                    explain: facts,
                    trend,
                }
            })
            .collect();
        GrowthView {
            from: Some(base.at),
            to: Some(newest.at),
            items,
            deltas: deltas(base, newest),
        }
    }

    /// Appends a startup-toggle entry to the activity log: the operation, the entry's name
    /// (never a path) and the result.
    pub fn audit_startup(&self, op: &str, name: &str, ok: bool) {
        let now = self.fs.now();
        let line = serde_json::json!({
            "time": unhoard_core::time::fmt_iso(now),
            "op": op,
            "itemId": name,
            "result": if ok { "ok" } else { "failed" },
        });
        // A record, not a gate: a failed append never undoes the switch.
        let _ignored_on_purpose = self.fs.append_line(&self.audit_log, &line.to_string());
    }

    /// Free and total space of the system drive, for the check-up.
    pub fn system_space(&self) -> (Option<u64>, Option<u64>) {
        let drive = self
            .context()
            .roots
            .var("SYSTEMDRIVE")
            .unwrap_or("C:")
            .to_string();
        match self.fs.volume(&format!("{drive}\\")) {
            Ok(v) => (Some(v.free_bytes), Some(v.total_bytes)),
            Err(_) => (None, None),
        }
    }

    /// Deletes the storage history.
    pub fn history_clear(&self) -> std::io::Result<()> {
        match &self.history {
            Some(store) => store.clear(),
            None => Ok(()),
        }
    }

    /// Writes a snapshot of the finished session `s`, when it is a whole drive that was not
    /// cancelled. Failures are counted in nothing and lose nothing: the next scan tries
    /// again.
    fn record_snapshot(&self, s: &Session, kind: SnapshotKind) {
        let Some(store) = &self.history else {
            return;
        };
        if !s.drive_scan || s.control.cancel.load(Ordering::SeqCst) {
            return;
        }
        let ctx = self.context();
        let tree = lock(&s.tree);
        let scanned = tree.get(tree.root()).map(|r| r.on_disk);
        let totals = match (&s.volume, scanned) {
            (Some(v), Some(on_disk)) => Totals {
                used: Some(v.total_bytes.saturating_sub(v.free_bytes)),
                free: Some(v.free_bytes),
                hidden: Some(hidden_bytes(v, on_disk, 0)),
            },
            _ => Totals::default(),
        };
        let snap = snapshot(&tree, &ctx, ctx.now, kind, totals);
        drop(tree);
        let _kept_for_next_time = store.add(snap, ctx.now);
    }

    /// Scans the drive `root` and writes a daily snapshot; blocks until done. `stop` is
    /// checked once a second and cancels the scan when it returns true (the CPU budget).
    pub fn measure(
        self: &Arc<Self>,
        root: &str,
        stop: &dyn Fn() -> bool,
    ) -> Result<bool, CmdError> {
        let s = self.start_scan_as(root, true, Some(SnapshotKind::Daily))?;
        while !s.done.load(Ordering::SeqCst) {
            if stop() {
                s.control.cancel.store(true, Ordering::SeqCst);
            }
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
        Ok(!s.control.cancel.load(Ordering::SeqCst))
    }

    /// Gathers knowledge with `gather` at the start of every scan (installed programs,
    /// program history, Steam), instead of none.
    pub fn with_knowledge(mut self, gather: Gather) -> Self {
        self.gather = gather;
        self
    }

    /// The scan context (rules, roots and what is known about this PC).
    pub fn context(&self) -> Arc<ScanContext> {
        lock(&self.ctx).clone()
    }

    /// Starts a scan of `root` on a background thread and returns its session. A finished
    /// drive scan leaves a storage-history snapshot.
    pub fn start_scan(
        self: &Arc<Self>,
        root: &str,
        drive_scan: bool,
    ) -> Result<Arc<Session>, CmdError> {
        self.start_scan_as(root, drive_scan, Some(SnapshotKind::Scan))
    }

    /// Starts a scan that records a snapshot of `kind` when it ends, or none.
    fn start_scan_as(
        self: &Arc<Self>,
        root: &str,
        drive_scan: bool,
        kind: Option<SnapshotKind>,
    ) -> Result<Arc<Session>, CmdError> {
        if let Some(s) = lock(&self.session).as_ref()
            && !s.done.load(Ordering::SeqCst)
        {
            return Err(CmdError::Busy);
        }
        let volume = self.fs.volume(root).ok();
        let tree = Arc::new(Mutex::new(Tree::new(
            root,
            volume.as_ref().map(|v| v.serial).unwrap_or(0),
        )));
        let session = Arc::new(Session {
            generation: self.generation.fetch_add(1, Ordering::SeqCst) + 1,
            root: root.to_string(),
            drive_scan,
            tree: tree.clone(),
            control: Arc::new(ScanControl::default()),
            done: Arc::new(AtomicBool::new(false)),
            volume,
            started: Instant::now(),
            duration_ms: Arc::new(AtomicU64::new(0)),
        });
        *lock(&self.session) = Some(session.clone());
        let (fs, shared, gather, s, me) = (
            self.fs.clone(),
            self.ctx.clone(),
            self.gather.clone(),
            session.clone(),
            self.clone(),
        );
        std::thread::spawn(move || {
            // Knowledge is gathered here, off the UI thread, fresh for every scan.
            let old = lock(&shared).clone();
            let ctx = Arc::new(ScanContext {
                rules: old.rules.clone(),
                roots: old.roots.clone(),
                now: fs.now(),
                knowledge: gather(&old.roots),
            });
            *lock(&shared) = ctx.clone();
            let start = lock(&s.tree).root();
            scan_into(&*fs, &s.tree, start, &ctx, &s.control);
            s.duration_ms
                .store(s.started.elapsed().as_millis() as u64, Ordering::SeqCst);
            if let Some(kind) = kind {
                me.record_snapshot(&s, kind);
            }
            s.done.store(true, Ordering::SeqCst);
        });
        Ok(session)
    }

    /// Asks the running scan to stop; the partial result stays.
    pub fn cancel_scan(&self) {
        if let Some(s) = lock(&self.session).as_ref() {
            s.control.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// The current session.
    pub fn current(&self) -> Result<Arc<Session>, CmdError> {
        lock(&self.session).clone().ok_or(CmdError::NoScan)
    }

    /// Resolves IDs from the UI: they must belong to the current scan and exist (T-5.16).
    pub fn resolve(
        &self,
        generation: u64,
        ids: &[u32],
    ) -> Result<(Arc<Session>, Vec<NodeId>), CmdError> {
        let s = self.current()?;
        if generation != s.generation {
            return Err(CmdError::StaleScan);
        }
        if ids.is_empty() || ids.len() > 10_000 {
            return Err(CmdError::InvalidArgument { what: "ids".into() });
        }
        let tree = lock(&s.tree);
        let out = ids
            .iter()
            .map(|&id| {
                tree.get(NodeId(id))
                    .map(|_| NodeId(id))
                    .ok_or(CmdError::UnknownItem { id })
            })
            .collect::<Result<Vec<_>, _>>()?;
        drop(tree);
        Ok((s, out))
    }

    /// One level of the tree.
    pub fn view(
        &self,
        generation: u64,
        id: u32,
        top: u16,
        logical: bool,
    ) -> Result<ViewResponse, CmdError> {
        let (s, ids) = self.resolve(generation, &[id])?;
        let tree = lock(&s.tree);
        let top = usize::from(top.clamp(1, 500));
        let v = view(&tree, ids[0], top, logical, &self.context())
            .ok_or(CmdError::UnknownItem { id })?;
        let scanning = !s.done.load(Ordering::SeqCst);
        let hidden = match (
            &s.volume,
            s.drive_scan && !scanning && ids[0] == tree.root(),
        ) {
            // The user's own Recycle Bin is readable, so it is part of the scan already.
            (Some(vol), true) => tree
                .get(tree.root())
                .map(|r| hidden_bytes(vol, r.on_disk, 0)),
            _ => None,
        };
        Ok(ViewResponse {
            generation,
            view: v,
            hidden_bytes: hidden,
            scanning,
        })
    }

    /// The details of one item.
    pub fn details(&self, generation: u64, id: u32) -> Result<Details, CmdError> {
        let (s, ids) = self.resolve(generation, &[id])?;
        let tree = lock(&s.tree);
        let ctx = self.context();
        let node = node_view(&tree, ids[0], &ctx).ok_or(CmdError::UnknownItem { id })?;
        let path = tree.path(ids[0]);
        let cloud = ctx
            .roots
            .sync_root_of(&path)
            .map(|_| summary(&tree, ids[0]));
        Ok(Details {
            node,
            description: describe(&tree, ids[0], &ctx),
            path,
            cloud,
            measured: lock(&self.measured).get(&(generation, id)).cloned(),
        })
    }

    /// The Suggestions view.
    pub fn suggestions(&self, limit: u16) -> Result<(u64, Vec<Suggestion>), CmdError> {
        let s = self.current()?;
        let tree = lock(&s.tree);
        Ok((
            s.generation,
            suggestions(&tree, &self.context(), usize::from(limit.clamp(1, 2000))),
        ))
    }

    /// The priority matrix of the current scan.
    pub fn matrix(&self, limit: u16) -> Result<(u64, Vec<MatrixPoint>), CmdError> {
        let s = self.current()?;
        let tree = lock(&s.tree);
        Ok((
            s.generation,
            matrix(&tree, &self.context(), usize::from(limit.clamp(1, 1000))),
        ))
    }

    /// The paths of folders the scan could not read: the given ones (which must be
    /// unscanned), or all of them, at most 4,096.
    pub fn unscanned_paths(
        &self,
        generation: u64,
        ids: Option<&[u32]>,
    ) -> Result<Vec<String>, CmdError> {
        let s = self.current()?;
        if s.generation != generation {
            return Err(CmdError::StaleScan);
        }
        let wanted: Vec<NodeId> = match ids {
            Some(ids) => self.resolve(generation, ids)?.1,
            None => lock(&s.tree).unscanned.iter().map(|(id, _)| *id).collect(),
        };
        let tree = lock(&s.tree);
        let mut out = vec![];
        for id in wanted.into_iter().take(4096) {
            let n = tree.get(id).ok_or(CmdError::UnknownItem { id: id.0 })?;
            if n.flags & FLAG_UNSCANNED == 0 || n.kind != NodeKind::Folder {
                return Err(CmdError::InvalidArgument {
                    what: "not an unreadable folder".into(),
                });
            }
            out.push(tree.path(id));
        }
        Ok(out)
    }

    /// Fills unscanned folders of scan `generation` with what the elevated helper measured.
    /// Folders that are not unscanned (or no longer in the tree) are ignored, so nothing is
    /// counted twice.
    pub fn apply_measured(
        &self,
        generation: u64,
        results: &[Measured],
    ) -> Result<MeasuredSummary, CmdError> {
        let s = self.current()?;
        if s.generation != generation {
            return Err(CmdError::StaleScan);
        }
        let mut tree = lock(&s.tree);
        let mut measured = lock(&self.measured);
        let mut sum = MeasuredSummary::default();
        for r in results {
            let Some(id) = tree.find(&r.path) else {
                continue;
            };
            if tree.set_measured(id, r.logical, r.on_disk, r.files) {
                sum.folders += 1;
                sum.on_disk += r.on_disk;
                measured.insert((generation, id.0), r.children.clone());
            }
        }
        Ok(sum)
    }

    /// The node IDs of paths in the current scan, `None` for a path it does not hold.
    pub fn ids_for_paths(
        &self,
        generation: u64,
        paths: &[String],
    ) -> Result<Vec<Option<u32>>, CmdError> {
        let s = self.current()?;
        if s.generation != generation {
            return Err(CmdError::StaleScan);
        }
        let tree = lock(&s.tree);
        Ok(paths.iter().map(|p| tree.find(p).map(|id| id.0)).collect())
    }

    /// The Steam app ID of a game folder in the current scan.
    pub fn steam_app_id(&self, generation: u64, id: u32) -> Result<u32, CmdError> {
        let (s, ids) = self.resolve(generation, &[id])?;
        let tree = lock(&s.tree);
        match describe(&tree, ids[0], &self.context()).steam {
            Some(SteamInfo::Game { app_id, .. }) => Ok(app_id),
            _ => Err(CmdError::InvalidArgument {
                what: "not a Steam game".into(),
            }),
        }
    }

    /// The preview of a move. Writes nothing.
    pub fn quarantine_preview(&self, generation: u64, ids: &[u32]) -> Result<MovePlan, CmdError> {
        let (s, ids) = self.resolve(generation, ids)?;
        let tree = lock(&s.tree);
        Ok(plan_move(&*self.fs, &tree, &ids, &self.context()))
    }

    /// Moves items to the quarantine. Re-plans (and so re-verifies) before acting, and
    /// removes the moved items from the tree.
    pub fn quarantine_move(&self, generation: u64, ids: &[u32]) -> Result<MoveReport, CmdError> {
        let (s, ids) = self.resolve(generation, ids)?;
        if !s.done.load(Ordering::SeqCst) {
            return Err(CmdError::Busy);
        }
        let plan = {
            let tree = lock(&s.tree);
            plan_move(&*self.fs, &tree, &ids, &self.context())
        };
        let report = execute_move(
            &*self.fs,
            &plan,
            &self.readme,
            &self.audit_log,
            self.fs.now(),
        );
        let mut tree = lock(&s.tree);
        for (m, r) in plan.moves.iter().zip(report.results.iter()) {
            if matches!(r.outcome, Outcome::Moved { .. }) {
                tree.detach(m.node);
            }
        }
        Ok(report)
    }

    fn load(&self) -> Vec<Batch> {
        load_batches(&*self.fs, &self.quarantine_roots)
    }

    /// Settles interrupted moves and runs the 30-day purge. Called once at start.
    pub fn startup_maintenance(&self) -> PurgeReport {
        let mut batches = self.load();
        let now = self.fs.now();
        for b in batches.iter_mut() {
            reconcile(&*self.fs, b, &self.audit_log, now);
            refresh(&*self.fs, b);
        }
        execute_purge(&*self.fs, &mut batches, now, &self.audit_log)
    }

    /// The Quarantine view.
    pub fn quarantine_state(&self) -> QuarantineState {
        let mut batches = self.load();
        for b in batches.iter_mut() {
            refresh(&*self.fs, b);
        }
        let now = self.fs.now();
        let mut drives: Vec<String> = batches.iter().filter_map(|b| drive_of(&b.dir)).collect();
        drives.sort();
        drives.dedup();
        let space = drives
            .into_iter()
            .filter_map(|d| {
                self.fs
                    .volume(&format!("{d}\\"))
                    .ok()
                    .map(|v| (d.clone(), drive_space(&v, &batches, &d)))
            })
            .collect();
        QuarantineState {
            warnings: warnings_due(&batches, now),
            space,
            batches: batches
                .into_iter()
                .map(|b| BatchView {
                    dir: b.dir,
                    manifest: b.manifest,
                })
                .collect(),
        }
    }

    fn batch(&self, batch_id: &str) -> Result<Batch, CmdError> {
        self.load()
            .into_iter()
            .find(|b| b.manifest.batch_id == batch_id)
            .ok_or(CmdError::InvalidArgument {
                what: "batch".into(),
            })
    }

    /// Restores items of a batch.
    pub fn restore(
        &self,
        batch_id: &str,
        item_ids: &[String],
    ) -> Result<Vec<ItemResult>, CmdError> {
        let mut b = self.batch(batch_id)?;
        Ok(restore(
            &*self.fs,
            &mut b,
            item_ids,
            &self.audit_log,
            self.fs.now(),
        ))
    }

    /// Records that the companion showed a purge warning.
    pub fn warning_shown(&self, batch_id: &str, days_left: i64) -> Result<(), CmdError> {
        if !quarantine::WARNING_DAYS.contains(&days_left) {
            return Err(CmdError::InvalidArgument {
                what: "daysLeft".into(),
            });
        }
        let mut b = self.batch(batch_id)?;
        record_warning(&*self.fs, &mut b, days_left, self.fs.now()).map_err(CmdError::from)
    }

    /// The folder of a batch, for "Open in Explorer".
    pub fn batch_dir(&self, batch_id: &str) -> Result<String, CmdError> {
        Ok(self.batch(batch_id)?.dir)
    }

    /// The path of a scanned item, for "Show in Explorer".
    pub fn path_of(&self, generation: u64, id: u32) -> Result<String, CmdError> {
        let (s, ids) = self.resolve(generation, &[id])?;
        let tree = lock(&s.tree);
        Ok(tree.path(ids[0]))
    }

    /// The sync roots of this PC with their sizes in the current scan.
    pub fn cloud_roots(&self, generation: u64) -> Result<Vec<CloudRoot>, CmdError> {
        let s = self.current()?;
        if s.generation != generation {
            return Err(CmdError::StaleScan);
        }
        let tree = lock(&s.tree);
        Ok(self
            .context()
            .roots
            .sync_roots
            .iter()
            .map(|p| {
                let id = tree.find(p);
                CloudRoot {
                    path: p.clone(),
                    id: id.map(|i| i.0),
                    summary: id.map(|i| summary(&tree, i)),
                }
            })
            .collect())
    }

    /// The preview of "free up space". Opens files for attributes only.
    pub fn free_up_preview(&self, generation: u64, ids: &[u32]) -> Result<FreeUpPlan, CmdError> {
        let (s, ids) = self.resolve(generation, ids)?;
        let tree = lock(&s.tree);
        Ok(plan_free_up(&*self.fs, &tree, &ids, &self.context()))
    }

    /// Frees up space (pin state only) and reports what is still on this PC afterwards.
    pub fn free_up_apply(
        &self,
        generation: u64,
        ids: &[u32],
    ) -> Result<(PinResults, u64, u64), CmdError> {
        let plan = self.free_up_preview(generation, ids)?;
        let results = apply_free_up(&*self.fs, &plan);
        let paths: Vec<String> = plan.files.iter().map(|f| f.1.clone()).collect();
        let still_local = remeasure(&*self.fs, &paths);
        Ok((results, plan.bytes_to_free, still_local))
    }

    /// Analyses the photos and videos of the current scan. Thumbnails and media properties
    /// are asked only for items that can be in a group that needs them, and `progress`
    /// hears how many of those are done.
    pub fn photos(&self, progress: &(dyn Fn(usize, usize) + Sync)) -> Result<PhotoState, CmdError> {
        let s = self.current()?;
        if !s.done.load(Ordering::SeqCst) {
            return Err(CmdError::Busy);
        }
        let media = {
            let tree = lock(&s.tree);
            media_index(&tree, &self.context())
        };
        let wanted = feature_candidates(&media, LOCAL_FEATURE_LIMIT, CLOUD_FEATURE_LIMIT);
        let total = wanted.iter().filter(|w| **w).count();
        let done = std::sync::atomic::AtomicUsize::new(0);
        progress(0, total);
        let features: Vec<Features> = {
            use rayon::prelude::*;
            media
                .par_iter()
                .zip(wanted.par_iter())
                .map(|(m, wanted)| {
                    if !*wanted {
                        return Features::default();
                    }
                    let f = self.media.features(
                        &m.path,
                        m.parsed.video,
                        m.in_sync_root || m.cloud_only,
                    );
                    let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if n.is_multiple_of(PHOTO_PROGRESS_EVERY) || n == total {
                        progress(n, total);
                    }
                    f
                })
                .collect()
        };
        let mut groups = exact_duplicates_local(&*self.fs, &media);
        groups.extend(cloud_duplicates(&media, &features));
        // A copy found by size, type and date or name is listed once: not again when its
        // thumbnails already proved it.
        let copies: Vec<_> = cloud_copies(&media)
            .into_iter()
            .filter(|c| {
                !groups
                    .iter()
                    .any(|g| c.items.iter().all(|i| g.items.contains(i)))
            })
            .collect();
        groups.extend(copies);
        groups.extend(video_duplicates(&media, &features));
        let near = near_duplicates(&media, &features, &groups);
        groups.extend(near);
        groups.extend(bursts(&media));
        groups.extend(by_month(&media));
        suggest_keepers(&mut groups, &media, &features);
        let state = PhotoState {
            generation: s.generation,
            media,
            groups,
        };
        *lock(&self.photos) = Some(state.clone());
        Ok(state)
    }

    /// The path of a photo from the last analysis, and whether it is in a sync root.
    pub fn photo_path(&self, generation: u64, index: u32) -> Result<(String, bool), CmdError> {
        let guard = lock(&self.photos);
        let state = guard.as_ref().ok_or(CmdError::NoScan)?;
        if state.generation != generation {
            return Err(CmdError::StaleScan);
        }
        let m = state
            .media
            .get(index as usize)
            .ok_or(CmdError::UnknownItem { id: index })?;
        Ok((m.path.clone(), m.in_sync_root || m.cloud_only))
    }

    /// Items of a batch still in the quarantine, for the UI.
    pub fn quarantined_count(state: &QuarantineState) -> usize {
        state
            .batches
            .iter()
            .flat_map(|b| b.manifest.items.iter())
            .filter(|i| i.status == ItemStatus::Quarantined)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use unhoard_core::fake::FakeFs;
    use unhoard_core::roots::Roots;
    use unhoard_core::rules::Rules;

    use super::*;

    struct NoMedia;
    impl MediaSource for NoMedia {
        fn features(&self, _: &str, _: bool, _: bool) -> Features {
            Features::default()
        }
    }

    fn service() -> (Arc<FakeFs>, Arc<Service>) {
        let fs = Arc::new(FakeFs::new());
        fs.create_dir_all(r"C:\Users\u\Downloads").unwrap();
        fs.create_dir_all(r"C:\Users\u\AppData\Local\Unhoard")
            .unwrap();
        fs.add_file(r"C:\Users\u\Downloads\a.iso", 5000);
        fs.add_file(r"C:\Users\u\Downloads\b.jpg", 7000);
        let env: BTreeMap<String, String> =
            [("USERPROFILE".to_string(), r"C:\Users\u".to_string())].into();
        let ctx = ScanContext {
            rules: Rules::empty(),
            roots: Roots::new(env, None, vec![]),
            now: fs.now(),
            knowledge: Default::default(),
        };
        let svc = Service::new(
            fs.clone(),
            ctx,
            Arc::new(NoMedia),
            r"C:\Users\u\AppData\Local\Unhoard\activity.jsonl".into(),
            vec![r"C:\Unhoard Quarantine".into()],
            |_| "readme".into(),
        );
        (fs, Arc::new(svc))
    }

    fn scanned(svc: &Arc<Service>) -> Arc<Session> {
        let s = svc.start_scan(r"C:\Users\u", false).unwrap();
        for _ in 0..500 {
            if s.done.load(Ordering::SeqCst) {
                return s;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("scan did not finish");
    }

    fn id_of(s: &Session, path: &str) -> u32 {
        lock(&s.tree).find(path).unwrap().0
    }

    #[test]
    fn s4_destructive_commands_reject_unknown_and_stale_ids() {
        let (_, svc) = service();
        assert_eq!(
            svc.quarantine_preview(1, &[0]).unwrap_err(),
            CmdError::NoScan
        );
        let s = scanned(&svc);
        assert_eq!(
            svc.quarantine_move(s.generation, &[999_999]).unwrap_err(),
            CmdError::UnknownItem { id: 999_999 }
        );
        assert_eq!(
            svc.quarantine_move(s.generation + 1, &[0]).unwrap_err(),
            CmdError::StaleScan
        );
        assert_eq!(
            svc.quarantine_move(s.generation, &[]).unwrap_err(),
            CmdError::InvalidArgument { what: "ids".into() }
        );
        let s2 = scanned(&svc);
        let old = id_of(&s, r"C:\Users\u\Downloads\a.iso");
        assert_eq!(
            svc.quarantine_move(s.generation, &[old]).unwrap_err(),
            CmdError::StaleScan
        );
        assert!(s2.generation > s.generation);
    }

    #[test]
    fn f5_a_move_detaches_the_item_and_keeps_totals_right() {
        let (fs, svc) = service();
        let s = scanned(&svc);
        let iso = id_of(&s, r"C:\Users\u\Downloads\a.iso");
        let before = svc
            .view(s.generation, 0, 10, true)
            .unwrap()
            .view
            .node
            .logical;
        let report = svc.quarantine_move(s.generation, &[iso]).unwrap();
        assert!(matches!(report.results[0].outcome, Outcome::Moved { .. }));
        assert!(!fs.has(r"C:\Users\u\Downloads\a.iso"));
        let after = svc
            .view(s.generation, 0, 10, true)
            .unwrap()
            .view
            .node
            .logical;
        assert_eq!(before - after, 5000);
        assert_eq!(
            svc.details(s.generation, iso).unwrap_err(),
            CmdError::UnknownItem { id: iso }
        );
        let q = svc.quarantine_state();
        assert_eq!(Service::quarantined_count(&q), 1);
        let item = q.batches[0].manifest.items[0].id.clone();
        let restored = svc
            .restore(&q.batches[0].manifest.batch_id, &[item])
            .unwrap();
        assert!(matches!(restored[0].outcome, Outcome::Restored { .. }));
        assert!(fs.has(r"C:\Users\u\Downloads\a.iso"));
    }

    #[test]
    fn f5_warning_days_are_validated() {
        let (_, svc) = service();
        assert_eq!(
            svc.warning_shown("x", 4).unwrap_err(),
            CmdError::InvalidArgument {
                what: "daysLeft".into()
            }
        );
    }

    #[test]
    fn f3_s7_admin_reading_fills_only_unreadable_folders_once() {
        let (fs, svc) = service();
        fs.create_dir_all(r"C:\Users\u\Locked").unwrap();
        fs.deny(r"C:\Users\u\Locked");
        let s = scanned(&svc);
        let g = s.generation;
        let paths = svc.unscanned_paths(g, None).unwrap();
        assert_eq!(paths, [r"C:\Users\u\Locked"]);
        let readable = svc
            .view(g, 0, 10, false)
            .unwrap()
            .view
            .children
            .iter()
            .find(|c| c.name == "Downloads")
            .unwrap()
            .id
            .0;
        assert!(matches!(
            svc.unscanned_paths(g, Some(&[readable])),
            Err(CmdError::InvalidArgument { .. })
        ));
        let before = svc.view(g, 0, 10, false).unwrap().view.node.on_disk;
        let m = Measured {
            path: r"C:\Users\u\Locked".into(),
            logical: 9000,
            on_disk: 12_288,
            files: 3,
            children: vec![("a".into(), 12_288)],
        };
        let sum = svc.apply_measured(g, std::slice::from_ref(&m)).unwrap();
        assert_eq!((sum.folders, sum.on_disk), (1, 12_288));
        assert_eq!(
            svc.apply_measured(g, std::slice::from_ref(&m))
                .unwrap()
                .folders,
            0,
            "never counted twice"
        );
        assert_eq!(
            svc.view(g, 0, 10, false).unwrap().view.node.on_disk,
            before + 12_288
        );
        assert!(svc.unscanned_paths(g, None).unwrap().is_empty());
        assert!(matches!(
            svc.apply_measured(g + 1, &[m]),
            Err(CmdError::StaleScan)
        ));
    }

    fn wait(s: &Session) {
        for _ in 0..500 {
            if s.done.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("scan did not finish");
    }

    #[test]
    fn f17_a_finished_drive_scan_writes_a_snapshot_and_growth_compares_two() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(HistoryStore::new(dir.path().join("history.jsonl")));
        let (fs, plain) = service();
        let svc = Arc::new(
            Arc::try_unwrap(plain)
                .ok()
                .unwrap()
                .with_history(store.clone()),
        );
        // A folder scan writes nothing.
        wait(&svc.start_scan(r"C:\Users\u", false).unwrap());
        assert!(store.load().0.is_empty());
        wait(&svc.start_scan(r"C:\", true).unwrap());
        let (list, _) = store.load();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, SnapshotKind::Scan);
        assert!(
            list[0]
                .folders
                .iter()
                .any(|f| f.path == r"C:\Users\u\Downloads")
        );
        // A second snapshot an hour later sees the new big file.
        fs.add_file(r"C:\Users\u\Downloads\big.iso", 300 << 20);
        let mut later = list[0].clone();
        later.at += 3600;
        for f in &mut later.folders {
            if [r"C:\Users", r"C:\Users\u", r"C:\Users\u\Downloads"].contains(&f.path.as_str()) {
                f.on_disk += 300 << 20;
            }
        }
        store.add(later, list[0].at + 3600).unwrap();
        let g = svc.history_growth(r"C:\", 1);
        assert_eq!(g.items[0].path, r"C:\Users\u\Downloads");
        assert_eq!(g.items[0].delta, 300 << 20);
        assert_eq!(svc.history_timeline(r"C:\").len(), 2);
    }

    #[test]
    fn f17_a_cancelled_scan_writes_no_snapshot() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(HistoryStore::new(dir.path().join("history.jsonl")));
        let (_fs, plain) = service();
        let svc = Arc::new(
            Arc::try_unwrap(plain)
                .ok()
                .unwrap()
                .with_history(store.clone()),
        );
        let done = svc.measure(r"C:\", &|| true).unwrap();
        assert!(!done);
        assert!(store.load().0.is_empty());
    }
}

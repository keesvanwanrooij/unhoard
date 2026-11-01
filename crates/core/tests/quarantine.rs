//! F5 quarantine and the safety invariants S1, S3, S4, S5, S6 and S10 against the fake
//! file system (T-5.1, T-5.3 to T-5.5, T-5.7 to T-5.12, T-5.14, T-5.15, T-6.5).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::fake::{FakeFs, FileOpts};
use unhoard_core::fs::{FileSystem, FsError, Identity};
use unhoard_core::quarantine::{
    self, Batch, ItemStatus, MANIFEST_NAME, Outcome, SkipReason, execute_move, execute_purge,
    load_batches, plan_move, plan_purge, reconcile, record_warning, restore, warnings_due,
};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::tree::Tree;

const AUDIT: &str = r"C:\Users\u\AppData\Local\Unhoard\activity.jsonl";
const QROOT: &str = r"C:\Unhoard Quarantine";

fn roots() -> Roots {
    let env: BTreeMap<String, String> = [
        ("WINDIR", r"C:\Windows"),
        ("PROGRAMFILES", r"C:\Program Files"),
        ("PROGRAMDATA", r"C:\ProgramData"),
        ("USERPROFILE", r"C:\Users\u"),
        ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    Roots::new(
        env,
        Some(r"C:\Users\u\Downloads".into()),
        vec![r"C:\Users\u\OneDrive".into()],
    )
}

fn ctx(fs: &FakeFs) -> ScanContext {
    ScanContext {
        rules: Rules::empty(),
        roots: roots(),
        now: fs.now(),
        knowledge: Default::default(),
    }
}

fn readme(_: &quarantine::Manifest) -> String {
    "where these came from".into()
}

/// A home folder with a few files, scanned.
fn setup() -> (FakeFs, Tree, ScanContext) {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\Downloads").unwrap();
    fs.create_dir_all(r"C:\Users\u\AppData\Local\Unhoard")
        .unwrap();
    fs.add_file_with(
        r"C:\Users\u\Downloads\big.iso",
        FileOpts {
            data: b"SECRET-CONTENT".to_vec(),
            attributes: 0x20,
            ..FileOpts::default()
        },
    );
    fs.add_file(r"C:\Users\u\Downloads\setup.exe", 5000);
    fs.add_dir(r"C:\Users\u\Downloads\project");
    fs.add_file(r"C:\Users\u\Downloads\project\a.txt", 10);
    fs.create_dir_all(r"C:\Users\u\OneDrive\Pictures").unwrap();
    fs.add_placeholder(r"C:\Users\u\OneDrive\Pictures\cloud.jpg", 3_000_000, true);
    let c = ctx(&fs);
    let (t, _) = scan(&fs, r"C:\Users\u", &c, &ScanControl::default()).unwrap();
    (fs, t, c)
}

fn id(t: &Tree, p: &str) -> unhoard_core::tree::NodeId {
    t.find(p).unwrap_or_else(|| panic!("{p} not scanned"))
}

fn batches(fs: &FakeFs) -> Vec<Batch> {
    load_batches(
        fs,
        &[
            QROOT.to_string(),
            r"C:\Users\u\OneDrive\Unhoard Quarantine".to_string(),
        ],
    )
}

fn move_items(fs: &FakeFs, t: &Tree, c: &ScanContext, paths: &[&str]) -> quarantine::MoveReport {
    let ids: Vec<_> = paths.iter().map(|p| id(t, p)).collect();
    let plan = plan_move(fs, t, &ids, c);
    execute_move(fs, &plan, &readme, AUDIT, fs.now())
}

#[test]
fn f5_dry_run_of_a_move_and_of_the_purge_writes_nothing() {
    let (fs, t, c) = setup();
    let before = fs.snapshot();
    let plan = plan_move(&fs, &t, &[id(&t, r"C:\Users\u\Downloads\big.iso")], &c);
    assert_eq!(plan.moves.len(), 1);
    assert_eq!(plan.total_logical, 14);
    assert!(plan.moves[0].dest_root.eq_ignore_ascii_case(QROOT));
    let _ = plan_purge(&fs, &batches(&fs), fs.now());
    assert_eq!(fs.snapshot(), before, "a dry run changed the disk");
}

#[test]
fn s5_manifest_entry_is_written_before_the_move() {
    struct Spy(FakeFs);
    impl FileSystem for Spy {
        fn read_dir(&self, p: &str) -> Result<Vec<unhoard_core::fs::DirEntry>, FsError> {
            self.0.read_dir(p)
        }
        fn volume(&self, p: &str) -> Result<unhoard_core::fs::VolumeInfo, FsError> {
            self.0.volume(p)
        }
        fn identity(&self, p: &str) -> Result<Identity, FsError> {
            self.0.identity(p)
        }
        fn exists(&self, p: &str) -> bool {
            self.0.exists(p)
        }
        fn move_verified(&self, from: &str, e: &Identity, to: &str) -> Result<(), FsError> {
            let dir = unhoard_core::roots::parent_of(to).unwrap();
            let m = String::from_utf8(
                self.0
                    .read_small(&unhoard_core::roots::join(&dir, MANIFEST_NAME), 1 << 20)
                    .unwrap(),
            )
            .unwrap();
            assert!(
                m.contains("\"moving\""),
                "no `moving` entry before the move: {m}"
            );
            assert!(
                m.contains(&from.replace('\\', "\\\\")),
                "the entry does not name the original path"
            );
            self.0.move_verified(from, e, to)
        }
        fn delete_verified(&self, p: &str, e: &Identity) -> Result<(), FsError> {
            self.0.delete_verified(p, e)
        }
        fn delete_tree(&self, p: &str) -> Result<(), FsError> {
            self.0.delete_tree(p)
        }
        fn create_dir_all(&self, p: &str) -> Result<(), FsError> {
            self.0.create_dir_all(p)
        }
        fn create_private_dir(&self, p: &str) -> Result<(), FsError> {
            self.0.create_private_dir(p)
        }
        fn write_durable(&self, p: &str, b: &[u8]) -> Result<(), FsError> {
            self.0.write_durable(p, b)
        }
        fn read_small(&self, p: &str, m: usize) -> Result<Vec<u8>, FsError> {
            self.0.read_small(p, m)
        }
        fn append_line(&self, p: &str, l: &str) -> Result<(), FsError> {
            self.0.append_line(p, l)
        }
        fn read_range(&self, p: &str, o: u64, l: usize) -> Result<Vec<u8>, FsError> {
            self.0.read_range(p, o, l)
        }
        fn placeholder_in_sync(&self, p: &str) -> Result<bool, FsError> {
            self.0.placeholder_in_sync(p)
        }
        fn set_unpinned(&self, p: &str) -> Result<(), FsError> {
            self.0.set_unpinned(p)
        }
        fn now(&self) -> i64 {
            self.0.now()
        }
    }
    let (fs, t, c) = setup();
    let spy = Spy(fs);
    let plan = plan_move(&spy, &t, &[id(&t, r"C:\Users\u\Downloads\setup.exe")], &c);
    let report = execute_move(&spy, &plan, &readme, AUDIT, spy.now());
    assert!(
        matches!(report.results[0].outcome, Outcome::Moved { .. }),
        "{:?}",
        report.results
    );
    let b = batches(&spy.0);
    assert_eq!(b[0].manifest.items[0].status, ItemStatus::Quarantined);
}

#[test]
fn f5_move_and_restore_round_trip_keeps_name_and_attributes() {
    let (fs, t, c) = setup();
    let attrs = fs.attributes(r"C:\Users\u\Downloads\big.iso").unwrap();
    let r = move_items(
        &fs,
        &t,
        &c,
        &[
            r"C:\Users\u\Downloads\big.iso",
            r"C:\Users\u\Downloads\project",
        ],
    );
    assert_eq!(
        r.results
            .iter()
            .filter(|x| matches!(x.outcome, Outcome::Moved { .. }))
            .count(),
        2
    );
    assert!(!fs.has(r"C:\Users\u\Downloads\big.iso"));
    let mut b = batches(&fs);
    assert_eq!(b.len(), 1);
    assert!(b[0].dir.contains("purged after"));
    assert!(fs.has(&unhoard_core::roots::join(
        &b[0].dir,
        "READ ME - where these came from.txt"
    )));
    let ids: Vec<String> = b[0].manifest.items.iter().map(|i| i.id.clone()).collect();
    let res = restore(&fs, &mut b[0], &ids, AUDIT, fs.now());
    assert!(
        res.iter()
            .all(|x| matches!(x.outcome, Outcome::Restored { .. })),
        "{res:?}"
    );
    assert_eq!(fs.attributes(r"C:\Users\u\Downloads\big.iso"), Some(attrs));
    assert!(fs.has(r"C:\Users\u\Downloads\project\a.txt"));
}

#[test]
fn f5_restore_never_overwrites_and_recreates_missing_parents() {
    let (fs, t, c) = setup();
    move_items(
        &fs,
        &t,
        &c,
        &[
            r"C:\Users\u\Downloads\setup.exe",
            r"C:\Users\u\Downloads\project\a.txt",
        ],
    );
    fs.add_file(r"C:\Users\u\Downloads\setup.exe", 1);
    fs.delete_tree(r"C:\Users\u\Downloads\project").unwrap();
    let mut b = batches(&fs);
    let ids: Vec<String> = b[0].manifest.items.iter().map(|i| i.id.clone()).collect();
    let res = restore(&fs, &mut b[0], &ids, AUDIT, fs.now());
    assert_eq!(
        fs.logical(r"C:\Users\u\Downloads\setup.exe"),
        Some(1),
        "the newer file was overwritten"
    );
    assert_eq!(
        fs.logical(r"C:\Users\u\Downloads\setup (restored).exe"),
        Some(5000)
    );
    assert!(fs.has(r"C:\Users\u\Downloads\project\a.txt"));
    assert!(
        res.iter()
            .any(|x| matches!(&x.outcome, Outcome::Restored { renamed: true, .. }))
    );
}

#[test]
fn s5_entries_left_moving_are_reconciled_at_start() {
    let (fs, t, c) = setup();
    move_items(
        &fs,
        &t,
        &c,
        &[
            r"C:\Users\u\Downloads\setup.exe",
            r"C:\Users\u\Downloads\big.iso",
        ],
    );
    let mut b = batches(&fs);
    // Simulate a crash: one entry moved but still marked `moving`; one never moved.
    b[0].manifest.items[0].status = ItemStatus::Moving;
    let back = b[0].manifest.items[1].clone();
    fs.move_verified(&back.quarantine_path, &back.identity, &back.original_path)
        .unwrap();
    b[0].manifest.items[1].status = ItemStatus::Moving;
    quarantine::save(&fs, &b[0]).unwrap();
    let mut b = batches(&fs);
    let fixed = reconcile(&fs, &mut b[0], AUDIT, fs.now());
    assert_eq!(fixed, 2);
    assert_eq!(b[0].manifest.items[0].status, ItemStatus::Quarantined);
    assert_eq!(b[0].manifest.items[1].status, ItemStatus::Restored);
    assert_eq!(
        batches(&fs)[0].manifest.items[0].status,
        ItemStatus::Quarantined,
        "reconciliation was not saved"
    );
}

#[test]
fn s1_purge_waits_until_a_warning_was_shown_three_days_earlier() {
    let (fs, t, c) = setup();
    move_items(&fs, &t, &c, &[r"C:\Users\u\Downloads\setup.exe"]);
    fs.advance_days(31);
    let mut b = batches(&fs);
    let r = execute_purge(&fs, &mut b, fs.now(), AUDIT);
    assert_eq!(r.purged, 0, "purged without a warning");
    let due = warnings_due(&b, fs.now());
    assert_eq!(due.len(), 1);
    record_warning(&fs, &mut b[0], due[0].days_left, fs.now()).unwrap();
    fs.advance_days(2);
    let mut b = batches(&fs);
    assert_eq!(
        execute_purge(&fs, &mut b, fs.now(), AUDIT).purged,
        0,
        "purged within 3 days of the warning"
    );
    fs.advance_days(1);
    let mut b = batches(&fs);
    assert_eq!(execute_purge(&fs, &mut b, fs.now(), AUDIT).purged, 1);
    assert!(batches(&fs).is_empty(), "the emptied batch folder stays");
}

#[test]
fn f5_warnings_show_at_five_three_and_one_day() {
    let (fs, t, c) = setup();
    move_items(&fs, &t, &c, &[r"C:\Users\u\Downloads\setup.exe"]);
    fs.advance_days(24);
    assert!(warnings_due(&batches(&fs), fs.now()).is_empty());
    fs.advance_days(1);
    let mut b = batches(&fs);
    let due = warnings_due(&b, fs.now());
    assert_eq!(due[0].days_left, 5);
    record_warning(&fs, &mut b[0], 5, fs.now()).unwrap();
    assert!(warnings_due(&batches(&fs), fs.now()).is_empty());
    fs.advance_days(2);
    assert_eq!(warnings_due(&batches(&fs), fs.now())[0].days_left, 3);
}

#[test]
fn s10_a_quarantine_move_does_not_change_the_reported_free_space() {
    let (fs, t, c) = setup();
    let free = fs.volume("C:\\").unwrap().free_bytes;
    move_items(&fs, &t, &c, &[r"C:\Users\u\Downloads\setup.exe"]);
    let space = quarantine::drive_space(&fs.volume("C:\\").unwrap(), &batches(&fs), "C:");
    assert_eq!(space.free_bytes, free);
    assert_eq!(space.not_freed_bytes, 8192);
}

#[test]
fn f5_an_item_changed_since_the_scan_is_skipped_and_reported() {
    let (fs, t, c) = setup();
    fs.touch(r"C:\Users\u\Downloads\setup.exe", 9999);
    let plan = plan_move(&fs, &t, &[id(&t, r"C:\Users\u\Downloads\setup.exe")], &c);
    assert!(plan.moves.is_empty());
    assert_eq!(plan.skipped[0].reason, SkipReason::Changed);
    // Changed between the preview and the confirmation: the handle check catches it.
    let plan = plan_move(&fs, &t, &[id(&t, r"C:\Users\u\Downloads\big.iso")], &c);
    fs.touch(r"C:\Users\u\Downloads\big.iso", 1);
    let r = execute_move(&fs, &plan, &readme, AUDIT, fs.now());
    assert_eq!(r.results[0].outcome, Outcome::Failed(FsError::Changed));
    assert!(fs.has(r"C:\Users\u\Downloads\big.iso"));
}

#[test]
fn s6_purge_skips_an_item_whose_manifest_entry_no_longer_matches() {
    let (fs, t, c) = setup();
    move_items(&fs, &t, &c, &[r"C:\Users\u\Downloads\setup.exe"]);
    let mut b = batches(&fs);
    let q = b[0].manifest.items[0].quarantine_path.clone();
    record_warning(&fs, &mut b[0], 5, fs.now()).unwrap();
    fs.touch(&q, 1);
    fs.advance_days(31);
    let mut b = batches(&fs);
    let r = execute_purge(&fs, &mut b, fs.now(), AUDIT);
    assert_eq!(r.purged, 0);
    assert_eq!(r.mismatched, 1);
    assert!(fs.has(&q));
}

#[test]
fn f5_a_full_disk_stops_the_move_before_it_starts() {
    let (fs, t, c) = setup();
    let plan = plan_move(&fs, &t, &[id(&t, r"C:\Users\u\Downloads\setup.exe")], &c);
    fs.set_full_disk(true);
    let r = execute_move(&fs, &plan, &readme, AUDIT, fs.now());
    assert_eq!(r.results[0].outcome, Outcome::Failed(FsError::DiskFull));
    assert!(fs.has(r"C:\Users\u\Downloads\setup.exe"));
}

#[test]
fn f5_locked_or_vanished_items_fail_alone_and_the_batch_continues() {
    let (fs, t, c) = setup();
    let plan = plan_move(
        &fs,
        &t,
        &[
            id(&t, r"C:\Users\u\Downloads\setup.exe"),
            id(&t, r"C:\Users\u\Downloads\big.iso"),
            id(&t, r"C:\Users\u\Downloads\project"),
        ],
        &c,
    );
    fs.lock(r"C:\Users\u\Downloads\setup.exe");
    fs.vanish_on_move(r"C:\Users\u\Downloads\big.iso");
    let r = execute_move(&fs, &plan, &readme, AUDIT, fs.now());
    let by = |p: &str| {
        r.results
            .iter()
            .find(|x| x.path.ends_with(p))
            .unwrap()
            .outcome
            .clone()
    };
    assert_eq!(by("setup.exe"), Outcome::Failed(FsError::Locked));
    assert_eq!(by("big.iso"), Outcome::Failed(FsError::NotFound));
    assert!(matches!(by("project"), Outcome::Moved { .. }));
    let b = batches(&fs);
    assert_eq!(
        b[0].manifest.items.len(),
        1,
        "failed items must not stay in the manifest"
    );
}

#[test]
fn f5_every_operation_appends_one_audit_line_without_file_contents() {
    let (fs, t, c) = setup();
    move_items(
        &fs,
        &t,
        &c,
        &[
            r"C:\Users\u\Downloads\setup.exe",
            r"C:\Users\u\Downloads\big.iso",
        ],
    );
    let mut b = batches(&fs);
    let first = b[0].manifest.items[0].id.clone();
    restore(&fs, &mut b[0], &[first], AUDIT, fs.now());
    record_warning(&fs, &mut b[0], 5, fs.now()).unwrap();
    fs.advance_days(31);
    let mut b = batches(&fs);
    execute_purge(&fs, &mut b, fs.now(), AUDIT);
    let log = String::from_utf8(fs.content(AUDIT).unwrap()).unwrap();
    let ops: Vec<String> = log
        .lines()
        .map(|l| {
            serde_json::from_str::<serde_json::Value>(l).unwrap()["op"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(ops, ["move", "move", "restore", "purge"]);
    assert!(!log.contains("SECRET"), "file contents in the audit log");
    assert!(!log.contains("Downloads"), "paths in the audit log");
}

#[test]
fn s4_s3_protected_paths_and_links_are_refused_and_cloud_items_stay_in_the_cloud() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\OneDrive\Pictures").unwrap();
    fs.add_placeholder(r"C:\Users\u\OneDrive\Pictures\cloud.jpg", 3_000_000, true);
    fs.create_dir_all(r"C:\Windows\Temp").unwrap();
    let c = ctx(&fs);
    let (t, _) = scan(&fs, r"C:\", &c, &ScanControl::default()).unwrap();
    let plan = plan_move(
        &fs,
        &t,
        &[
            id(&t, r"C:\Windows\Temp"),
            id(&t, r"C:\Users\u"),
            id(&t, r"C:\Users\u\OneDrive\Pictures\cloud.jpg"),
        ],
        &c,
    );
    assert_eq!(plan.skipped.len(), 2);
    assert!(
        plan.skipped
            .iter()
            .all(|s| matches!(s.reason, SkipReason::Protected(_)))
    );
    assert_eq!(plan.moves.len(), 1);
    assert!(plan.moves[0].cloud);
    assert!(
        plan.moves[0]
            .dest_root
            .eq_ignore_ascii_case(r"C:\Users\u\OneDrive\Unhoard Quarantine")
    );
    let r = execute_move(&fs, &plan, &readme, AUDIT, fs.now());
    assert!(matches!(r.results[0].outcome, Outcome::Moved { .. }));
    assert!(fs.has(r"C:\Users\u\OneDrive\Unhoard Quarantine"));
    assert!(!fs.has(QROOT), "a cloud item reached the local quarantine");
    assert!(
        fs.data_opens().is_empty(),
        "s2: the cloud move opened file data"
    );
}

#[test]
fn f5_a_failed_outcome_keeps_its_reason_in_json() {
    let json = serde_json::to_value(Outcome::Failed(FsError::Locked)).unwrap();
    assert_eq!(json["outcome"], "failed");
    assert_eq!(json["kind"], "locked");
}

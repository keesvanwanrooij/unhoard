//! F1 scan and F3 hidden space, against the fake file system (T-1.x, T-3.1).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::sync::Mutex;
use std::sync::atomic::Ordering;

use unhoard_core::fake::{FakeFs, FileOpts};
use unhoard_core::fs::{ATTR_RECALL_ON_DATA_ACCESS, FsError, VolumeInfo};
use unhoard_core::hidden::hidden_bytes;
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, rescan, scan};
use unhoard_core::tree::{FLAG_HARD_LINK_DUP, NodeKind};

fn ctx() -> ScanContext {
    ScanContext {
        rules: Rules::empty(),
        roots: Roots::default(),
        now: 1_790_000_000,
        knowledge: Default::default(),
    }
}

fn tree_of(fs: &FakeFs, root: &str) -> unhoard_core::tree::Tree {
    scan(fs, root, &ctx(), &ScanControl::default()).unwrap().0
}

#[test]
fn f1_sizes_aggregate_bottom_up() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\a");
    fs.add_file(r"C:\a\x.bin", 100);
    fs.add_dir(r"C:\a\b");
    fs.add_file(r"C:\a\b\y.bin", 50);
    fs.add_file(r"C:\a\b\z.bin", 25);
    let t = tree_of(&fs, r"C:\a");
    let root = t.get(t.root()).unwrap();
    assert_eq!(root.logical, 175);
    assert_eq!(root.files, 3);
    let b = t.find(r"C:\a\b").unwrap();
    assert_eq!(t.get(b).unwrap().logical, 75);
    let sum: u64 = t
        .children(t.root())
        .iter()
        .map(|c| t.get(*c).unwrap().logical)
        .sum();
    assert_eq!(sum, root.logical);
}

#[test]
fn f1_hard_link_counts_once() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\h");
    fs.add_file(r"C:\h\one.bin", 10 << 20);
    fs.add_hard_link(r"C:\h\two.bin", r"C:\h\one.bin");
    let t = tree_of(&fs, r"C:\h");
    assert_eq!(t.get(t.root()).unwrap().logical, 10 << 20);
    let dups = t
        .iter()
        .filter(|(_, n)| n.flags & FLAG_HARD_LINK_DUP != 0)
        .count();
    assert_eq!(dups, 1);
}

#[test]
fn f1_junction_is_recorded_and_not_followed() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\j");
    fs.add_junction(r"C:\j\loop");
    fs.add_file(r"C:\j\loop\inside.bin", 999);
    let t = tree_of(&fs, r"C:\j");
    let link = t.find(r"C:\j\loop").unwrap();
    assert_eq!(t.get(link).unwrap().kind, NodeKind::Link);
    assert!(t.children(link).is_empty());
    assert_eq!(t.get(t.root()).unwrap().logical, 0);
}

#[test]
fn f1_access_denied_is_listed_as_unscanned_and_the_scan_continues() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\d");
    fs.add_dir(r"C:\d\locked");
    fs.add_file(r"C:\d\locked\secret.bin", 5);
    fs.deny(r"C:\d\locked");
    fs.add_file(r"C:\d\ok.bin", 7);
    let t = tree_of(&fs, r"C:\d");
    assert_eq!(t.get(t.root()).unwrap().logical, 7);
    assert_eq!(t.unscanned.len(), 1);
    assert_eq!(t.path(t.unscanned[0].0), r"C:\d\locked");
    assert_eq!(t.unscanned[0].1, FsError::AccessDenied);
}

#[test]
fn f1_online_only_file_reports_logical_size_and_zero_on_disk() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\od");
    fs.add_placeholder(r"C:\od\photo.jpg", 4 << 20, true);
    let t = tree_of(&fs, r"C:\od");
    let n = t.get(t.find(r"C:\od\photo.jpg").unwrap()).unwrap();
    assert_eq!(n.logical, 4 << 20);
    assert_eq!(n.on_disk, 0);
    assert!(n.attributes & ATTR_RECALL_ON_DATA_ACCESS != 0);
    assert!(fs.data_opens().is_empty(), "s2: scanning opened file data");
}

#[test]
fn f1_cancel_returns_a_partial_result() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\c");
    for i in 0..50 {
        fs.add_dir(&format!(r"C:\c\d{i}"));
        fs.add_file(&format!(r"C:\c\d{i}\f.bin"), 1);
    }
    let control = ScanControl::default();
    control.cancel.store(true, Ordering::SeqCst);
    let start = std::time::Instant::now();
    let (t, outcome) = scan(&fs, r"C:\c", &ctx(), &control).unwrap();
    assert!(outcome.partial);
    assert!(t.get(t.root()).unwrap().files < 50);
    assert!(start.elapsed().as_secs() < 1);
}

#[test]
fn f1_rescan_replaces_only_the_subtree() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\r");
    fs.add_dir(r"C:\r\a");
    fs.add_file(r"C:\r\a\f.bin", 10);
    fs.add_dir(r"C:\r\b");
    fs.add_file(r"C:\r\b\g.bin", 20);
    let t = Mutex::new(tree_of(&fs, r"C:\r"));
    fs.add_file(r"C:\r\a\new.bin", 5);
    fs.add_file(r"C:\r\b\ignored.bin", 1000);
    let a = t.lock().unwrap().find(r"C:\r\a").unwrap();
    let outcome = rescan(&fs, &t, a, &ctx(), &ScanControl::default());
    assert!(!outcome.partial);
    let t = t.into_inner().unwrap();
    assert_eq!(t.get(t.find(r"C:\r\a").unwrap()).unwrap().logical, 15);
    assert_eq!(t.get(t.find(r"C:\r\b").unwrap()).unwrap().logical, 20);
    assert_eq!(t.get(t.root()).unwrap().logical, 35);
}

#[test]
fn f1_progress_counts_files_and_bytes() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\p");
    fs.add_file_with(
        r"C:\p\a.bin",
        FileOpts {
            logical: Some(100),
            on_disk: Some(4096),
            ..FileOpts::default()
        },
    );
    let control = ScanControl::default();
    scan(&fs, r"C:\p", &ctx(), &control).unwrap();
    assert_eq!(control.files.load(Ordering::SeqCst), 1);
    assert_eq!(control.on_disk.load(Ordering::SeqCst), 4096);
}

#[test]
fn f3_hidden_space_is_used_minus_scanned_and_never_negative() {
    let v = VolumeInfo {
        total_bytes: 1000,
        free_bytes: 100,
        serial: 1,
        file_system: "NTFS".into(),
    };
    assert_eq!(hidden_bytes(&v, 700, 0), 200);
    assert_eq!(hidden_bytes(&v, 700, 50), 150);
    assert_eq!(hidden_bytes(&v, 950, 0), 0);
}

#[test]
fn f5_a_moved_item_is_detached_and_its_size_leaves_every_ancestor() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\m");
    fs.add_dir(r"C:\m\a");
    fs.add_file(r"C:\m\a\big.bin", 1000);
    fs.add_file(r"C:\m\a\small.bin", 10);
    let mut t = tree_of(&fs, r"C:\m");
    let big = t.find(r"C:\m\a\big.bin").unwrap();
    t.detach(big);
    assert!(t.get(big).is_none());
    assert_eq!(t.get(t.find(r"C:\m\a").unwrap()).unwrap().logical, 10);
    assert_eq!(t.get(t.root()).unwrap().logical, 10);
    assert_eq!(t.get(t.root()).unwrap().files, 1);
    assert_eq!(t.children(t.find(r"C:\m\a").unwrap()).len(), 1);
}

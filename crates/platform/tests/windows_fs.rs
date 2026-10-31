//! Windows integration tests of the file-system layer, on generated trees in temp folders
//! (T-1.2, T-1.3, T-1.6, T-5.2, T-5.6, T-5.7, T-5.9, T-5.10, T-5.13, T-S2).
#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;
use std::process::Command;

use unhoard_core::fs::{FileSystem, FsError};
use unhoard_core::quarantine::{load_batches, plan_move, plan_purge};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::tree::{FLAG_HARD_LINK_DUP, NodeKind};
use unhoard_platform::WindowsFs;

fn ctx() -> ScanContext {
    ScanContext {
        rules: Rules::empty(),
        roots: Roots::default(),
        now: WindowsFs.now(),
    }
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// Every file and folder below `root` with size, attributes and last write time.
fn snapshot(root: &Path) -> BTreeMap<String, (u64, u32, u64)> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let e = e.unwrap();
            let m = std::fs::symlink_metadata(e.path()).unwrap();
            out.insert(
                s(&e.path()),
                (m.file_size(), m.file_attributes(), m.last_write_time()),
            );
            if m.is_dir() && !m.file_type().is_symlink() {
                stack.push(e.path());
            }
        }
    }
    out
}

fn junction(link: &Path, target: &Path) {
    let ok = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()
        .unwrap();
    assert!(
        ok.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
}

#[test]
fn f1_hard_link_counts_once() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("one.bin"), vec![1u8; 100_000]).unwrap();
    std::fs::hard_link(dir.path().join("one.bin"), dir.path().join("two.bin")).unwrap();
    let (t, _) = scan(&WindowsFs, &s(dir.path()), &ctx(), &ScanControl::default()).unwrap();
    assert_eq!(t.get(t.root()).unwrap().logical, 100_000);
    assert_eq!(
        t.iter()
            .filter(|(_, n)| n.flags & FLAG_HARD_LINK_DUP != 0)
            .count(),
        1
    );
}

#[test]
fn f1_junctions_are_recorded_and_a_loop_terminates() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("a")).unwrap();
    std::fs::write(dir.path().join("a").join("f.bin"), [0u8; 10]).unwrap();
    junction(&dir.path().join("a").join("loop"), dir.path());
    let (t, _) = scan(&WindowsFs, &s(dir.path()), &ctx(), &ScanControl::default()).unwrap();
    let link = t.find(&s(&dir.path().join("a").join("loop"))).unwrap();
    assert_eq!(t.get(link).unwrap().kind, NodeKind::Link);
    assert!(t.children(link).is_empty());
    assert_eq!(t.get(t.root()).unwrap().logical, 10);
}

#[test]
fn f1_paths_longer_than_260_characters_are_scanned() {
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().to_path_buf();
    for i in 0..12 {
        deep.push(format!("folder-with-a-rather-long-name-{i:02}"));
    }
    assert!(s(&deep).len() > 300);
    let verbatim = format!(r"\\?\{}", s(&deep));
    std::fs::create_dir_all(&verbatim).unwrap();
    std::fs::write(format!(r"{verbatim}\deep.bin"), [7u8; 42]).unwrap();
    let (t, _) = scan(&WindowsFs, &s(dir.path()), &ctx(), &ScanControl::default()).unwrap();
    assert_eq!(t.get(t.root()).unwrap().logical, 42);
    assert!(t.unscanned.is_empty());
}

#[test]
fn s2_scanning_changes_no_file_timestamp_or_attribute() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub").join("a.txt"), b"hello").unwrap();
    std::fs::write(dir.path().join("b.jpg"), [1u8; 2000]).unwrap();
    let before = snapshot(dir.path());
    scan(&WindowsFs, &s(dir.path()), &ctx(), &ScanControl::default()).unwrap();
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn f5_move_and_move_back_keep_name_timestamps_and_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("photo.jpg");
    std::fs::write(&src, [3u8; 5000]).unwrap();
    let mut perms = std::fs::metadata(&src).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&src, perms).unwrap();
    let before = std::fs::symlink_metadata(&src).unwrap();
    std::fs::create_dir(dir.path().join("q")).unwrap();
    let dest = dir.path().join("q").join("photo.jpg");
    let id = WindowsFs.identity(&s(&src)).unwrap();
    WindowsFs.move_verified(&s(&src), &id, &s(&dest)).unwrap();
    assert!(!src.exists());
    let id2 = WindowsFs.identity(&s(&dest)).unwrap();
    assert_eq!(
        id2.file_id, id.file_id,
        "a same-volume move must be a rename"
    );
    WindowsFs.move_verified(&s(&dest), &id2, &s(&src)).unwrap();
    let after = std::fs::symlink_metadata(&src).unwrap();
    assert_eq!(after.last_write_time(), before.last_write_time());
    assert_eq!(after.creation_time(), before.creation_time());
    assert_eq!(after.file_attributes(), before.file_attributes());
}

#[test]
fn f5_a_changed_or_swapped_item_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("a.bin");
    std::fs::write(&src, [1u8; 100]).unwrap();
    let id = WindowsFs.identity(&s(&src)).unwrap();
    std::fs::remove_file(&src).unwrap();
    std::fs::write(&src, [1u8; 100]).unwrap();
    let r = WindowsFs.move_verified(&s(&src), &id, &s(&dir.path().join("b.bin")));
    assert_eq!(r, Err(FsError::Changed));
    assert!(src.exists());
}

#[test]
fn f5_never_overwrites_the_destination() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a.bin"), dir.path().join("b.bin"));
    std::fs::write(&a, [1u8; 10]).unwrap();
    std::fs::write(&b, [2u8; 20]).unwrap();
    let id = WindowsFs.identity(&s(&a)).unwrap();
    assert_eq!(
        WindowsFs.move_verified(&s(&a), &id, &s(&b)),
        Err(FsError::AlreadyExists)
    );
    assert_eq!(std::fs::read(&b).unwrap(), vec![2u8; 20]);
}

#[test]
fn f5_a_locked_file_fails_with_locked() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("open.bin");
    std::fs::write(&src, [1u8; 10]).unwrap();
    let id = WindowsFs.identity(&s(&src)).unwrap();
    let _held = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&src)
        .unwrap();
    let r = WindowsFs.move_verified(&s(&src), &id, &s(&dir.path().join("moved.bin")));
    assert_eq!(r, Err(FsError::Locked));
}

#[test]
fn f5_unicode_names_and_trailing_dots_or_spaces_survive_a_move_and_back() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("q")).unwrap();
    for name in ["naïve 文件 😀.txt", "trailing dot.", "trailing space "] {
        let src = format!(r"\\?\{}\{name}", s(dir.path()));
        std::fs::write(&src, name.as_bytes()).unwrap();
        let plain = format!(r"{}\{name}", s(dir.path()));
        let dest = format!(r"{}\q\{name}", s(dir.path()));
        let id = WindowsFs.identity(&plain).unwrap();
        WindowsFs.move_verified(&plain, &id, &dest).unwrap();
        let id2 = WindowsFs.identity(&dest).unwrap();
        WindowsFs.move_verified(&dest, &id2, &plain).unwrap();
        assert_eq!(std::fs::read(&src).unwrap(), name.as_bytes());
    }
    let listed: Vec<String> = WindowsFs
        .read_dir(&s(dir.path()))
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert!(
        listed.contains(&"trailing dot.".to_string())
            && listed.contains(&"trailing space ".to_string())
    );
}

#[test]
fn s6_delete_removes_a_link_as_a_link_and_never_its_target() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("precious");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("keep.txt"), b"keep").unwrap();
    let batch = dir.path().join("batch");
    std::fs::create_dir_all(batch.join("inner")).unwrap();
    std::fs::write(batch.join("inner").join("x.bin"), [1u8; 10]).unwrap();
    std::fs::write(batch.join("readonly.bin"), [1u8; 10]).unwrap();
    let mut p = std::fs::metadata(batch.join("readonly.bin"))
        .unwrap()
        .permissions();
    p.set_readonly(true);
    std::fs::set_permissions(batch.join("readonly.bin"), p).unwrap();
    junction(&batch.join("link"), &target);
    let id = WindowsFs.identity(&s(&batch)).unwrap();
    WindowsFs.delete_verified(&s(&batch), &id).unwrap();
    assert!(!batch.exists());
    assert_eq!(std::fs::read(target.join("keep.txt")).unwrap(), b"keep");
}

#[test]
fn f5_dry_runs_of_a_move_and_of_the_purge_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("Downloads")).unwrap();
    std::fs::write(dir.path().join("Downloads").join("big.iso"), [1u8; 4096]).unwrap();
    let c = ctx();
    let (t, _) = scan(&WindowsFs, &s(dir.path()), &c, &ScanControl::default()).unwrap();
    let before = snapshot(dir.path());
    let plan = plan_move(
        &WindowsFs,
        &t,
        &[t.find(&s(&dir.path().join("Downloads").join("big.iso")))
            .unwrap()],
        &c,
    );
    assert_eq!(plan.moves.len(), 1);
    let _ = plan_purge(
        &WindowsFs,
        &load_batches(&WindowsFs, &[s(dir.path())]),
        WindowsFs.now(),
    );
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn f5_durable_write_replaces_atomically_and_reads_back() {
    let dir = tempfile::tempdir().unwrap();
    let p = s(&dir.path().join("m.json"));
    WindowsFs.write_durable(&p, b"one").unwrap();
    WindowsFs.write_durable(&p, b"two").unwrap();
    assert_eq!(WindowsFs.read_small(&p, 100).unwrap(), b"two");
    WindowsFs
        .append_line(&s(&dir.path().join("log.jsonl")), "{}")
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("log.jsonl")).unwrap(),
        "{}\n"
    );
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        2,
        "no temp file left behind"
    );
}

#[test]
fn f5_private_folder_is_created() {
    let dir = tempfile::tempdir().unwrap();
    let q = s(&dir.path().join("Unhoard Quarantine"));
    WindowsFs.create_private_dir(&q).unwrap();
    assert!(Path::new(&q).is_dir());
    std::fs::write(Path::new(&q).join("x.txt"), b"x").unwrap();
}

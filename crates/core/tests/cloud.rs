//! F6 cloud: placeholder states, free up space and the hydration guard (T-6.1 to T-6.4).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::cloud::{
    FreeUpSkip, PinState, apply_free_up, pin_state, plan_free_up, remeasure, summary,
};
use unhoard_core::fake::{FakeFs, FileOpts};
use unhoard_core::fs::{ATTR_PINNED, ATTR_UNPINNED, FileSystem};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};

const SR: &str = r"C:\Users\u\OneDrive";

fn ctx(fs: &FakeFs) -> ScanContext {
    let env: BTreeMap<String, String> =
        [("USERPROFILE".to_string(), r"C:\Users\u".to_string())].into();
    ScanContext {
        rules: Rules::empty(),
        roots: Roots::new(env, None, vec![SR.into()]),
        now: fs.now(),
        knowledge: Default::default(),
    }
}

fn library() -> FakeFs {
    let fs = FakeFs::new();
    fs.create_dir_all(&format!(r"{SR}\Pictures")).unwrap();
    fs.add_placeholder(&format!(r"{SR}\Pictures\online.jpg"), 4_000_000, true);
    fs.add_file_with(
        &format!(r"{SR}\Pictures\local.jpg"),
        FileOpts {
            logical: Some(2_000_000),
            in_sync: true,
            ..FileOpts::default()
        },
    );
    fs.add_file_with(
        &format!(r"{SR}\Pictures\pinned.jpg"),
        FileOpts {
            logical: Some(1_000_000),
            attributes: ATTR_PINNED,
            in_sync: true,
            ..FileOpts::default()
        },
    );
    fs.add_file_with(
        &format!(r"{SR}\Pictures\uploading.jpg"),
        FileOpts {
            logical: Some(500_000),
            in_sync: false,
            ..FileOpts::default()
        },
    );
    fs
}

#[test]
fn f6_placeholder_states_and_cloud_versus_local_size() {
    let fs = library();
    let c = ctx(&fs);
    let (t, _) = scan(&fs, SR, &c, &ScanControl::default()).unwrap();
    let s = summary(&t, t.find(&format!(r"{SR}\Pictures")).unwrap());
    assert_eq!(s.cloud_bytes, 7_500_000);
    assert_eq!(s.online_only, 1);
    assert_eq!(s.locally_available, 2);
    assert_eq!(s.always_keep, 1);
    assert!(s.local_bytes > 0 && s.local_bytes < s.cloud_bytes);
    assert_eq!(pin_state(ATTR_PINNED), PinState::AlwaysKeep);
}

#[test]
fn f6_free_up_space_changes_only_the_pin_state_and_remeasures() {
    let fs = library();
    let c = ctx(&fs);
    let (t, _) = scan(&fs, SR, &c, &ScanControl::default()).unwrap();
    let before = fs.logical(&format!(r"{SR}\Pictures\pinned.jpg"));
    let plan = plan_free_up(&fs, &t, &[t.find(&format!(r"{SR}\Pictures")).unwrap()], &c);
    assert_eq!(plan.files.len(), 2, "local.jpg and pinned.jpg");
    assert!(plan.bytes_to_free > 0);
    let results = apply_free_up(&fs, &plan);
    assert!(results.iter().all(|r| r.1.is_ok()));
    let a = fs
        .attributes(&format!(r"{SR}\Pictures\pinned.jpg"))
        .unwrap();
    assert_eq!(a & ATTR_PINNED, 0);
    assert_ne!(a & ATTR_UNPINNED, 0);
    assert_eq!(
        fs.logical(&format!(r"{SR}\Pictures\pinned.jpg")),
        before,
        "free up space changed the file"
    );
    let still_local = remeasure(
        &fs,
        &plan.files.iter().map(|f| f.1.clone()).collect::<Vec<_>>(),
    );
    assert_eq!(still_local, 0);
}

#[test]
fn f6_files_not_in_sync_are_not_offered_for_free_up_space() {
    let fs = library();
    let c = ctx(&fs);
    let (t, _) = scan(&fs, SR, &c, &ScanControl::default()).unwrap();
    let plan = plan_free_up(
        &fs,
        &t,
        &[t.find(&format!(r"{SR}\Pictures\uploading.jpg")).unwrap()],
        &c,
    );
    assert!(plan.files.is_empty());
    assert_eq!(plan.skipped[0].1, FreeUpSkip::NotInSync);
}

#[test]
fn s2_no_cloud_operation_opens_a_placeholder_for_its_data() {
    let fs = library();
    let c = ctx(&fs);
    let (t, _) = scan(&fs, SR, &c, &ScanControl::default()).unwrap();
    let root = t.find(&format!(r"{SR}\Pictures")).unwrap();
    let _ = summary(&t, root);
    let plan = plan_free_up(&fs, &t, &[root], &c);
    apply_free_up(&fs, &plan);
    let _ = unhoard_core::classify::describe(&t, root, &c);
    assert!(
        fs.data_opens().is_empty(),
        "a placeholder was opened for data: {:?}",
        fs.data_opens()
    );
}

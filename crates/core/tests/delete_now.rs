//! S6 "Delete now": only items whose rule has tier Safe, never in a sync root, a preview
//! that writes nothing, an identity check per item and an audit-log line.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::fake::FakeFs;
use unhoard_core::fs::FileSystem;
use unhoard_core::quarantine::{Outcome, SkipReason, execute_delete, plan_delete};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::tree::Tree;

const AUDIT: &str = r"C:\Users\u\AppData\Local\Unhoard\activity.jsonl";

/// A safe cache, a likely-safe folder and a cache-like folder inside OneDrive.
const RULES: &str = r#"
[[rule]]
id = "test-cache"
parent = "%LOCALAPPDATA%"
name = "SomeCache"
kind = "dir"
category = "developer"
tier = "safe"
action = "quarantine"

[[rule]]
id = "test-modules"
parent = '%USERPROFILE%\code'
name = "node_modules"
kind = "dir"
category = "developer"
tier = "likelySafe"
action = "quarantine"

[[rule]]
id = "test-cloud-cache"
parent = '%USERPROFILE%\OneDrive'
name = "cache"
kind = "dir"
category = "developer"
tier = "safe"
action = "quarantine"
"#;

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
    Roots::new(env, None, vec![r"C:\Users\u\OneDrive".into()])
}

fn setup() -> (FakeFs, Tree, ScanContext) {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\AppData\Local\Unhoard")
        .unwrap();
    for dir in [
        r"C:\Users\u\AppData\Local\SomeCache\sub",
        r"C:\Users\u\code\node_modules",
        r"C:\Users\u\OneDrive\cache",
    ] {
        fs.create_dir_all(dir).unwrap();
    }
    fs.add_file(r"C:\Users\u\AppData\Local\SomeCache\a.bin", 4000);
    fs.add_file(r"C:\Users\u\AppData\Local\SomeCache\sub\b.bin", 6000);
    fs.add_file(r"C:\Users\u\code\node_modules\x.js", 500);
    fs.add_file(r"C:\Users\u\OneDrive\cache\c.bin", 700);
    let roots = roots();
    let c = ScanContext {
        rules: Rules::from_toml(&[RULES], &roots).unwrap(),
        roots,
        now: fs.now(),
        knowledge: Default::default(),
    };
    let (t, _) = scan(&fs, r"C:\Users\u", &c, &ScanControl::default()).unwrap();
    (fs, t, c)
}

fn id(t: &Tree, path: &str) -> unhoard_core::tree::NodeId {
    t.find(path).unwrap_or_else(|| panic!("{path}"))
}

#[test]
fn s6_delete_now_takes_only_safe_items_outside_sync_roots() {
    let (fs, t, c) = setup();
    let ids = [
        id(&t, r"C:\Users\u\AppData\Local\SomeCache"),
        id(&t, r"C:\Users\u\code\node_modules"),
        id(&t, r"C:\Users\u\OneDrive\cache"),
    ];
    let plan = plan_delete(&fs, &t, &ids, &c);
    let paths: Vec<&str> = plan.items.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(paths, [r"C:\Users\u\AppData\Local\SomeCache"]);
    assert!(plan.total_on_disk >= 10_000);
    let reasons: Vec<(&str, &SkipReason)> = plan
        .skipped
        .iter()
        .map(|s| (s.path.as_str(), &s.reason))
        .collect();
    assert!(reasons.contains(&(r"C:\Users\u\code\node_modules", &SkipReason::NotSafe)));
    assert!(reasons.contains(&(r"C:\Users\u\OneDrive\cache", &SkipReason::Cloud)));
}

#[test]
fn s6_the_delete_preview_writes_nothing() {
    let (fs, t, c) = setup();
    let before = fs.snapshot();
    let _plan = plan_delete(
        &fs,
        &t,
        &[id(&t, r"C:\Users\u\AppData\Local\SomeCache")],
        &c,
    );
    assert_eq!(fs.snapshot(), before);
}

#[test]
fn s6_delete_now_removes_the_item_and_logs_it() {
    let (fs, t, c) = setup();
    let plan = plan_delete(
        &fs,
        &t,
        &[id(&t, r"C:\Users\u\AppData\Local\SomeCache")],
        &c,
    );
    let report = execute_delete(&fs, &plan, AUDIT, fs.now());
    assert!(matches!(report.results[0].outcome, Outcome::Deleted));
    assert!(!fs.has(r"C:\Users\u\AppData\Local\SomeCache"));
    assert!(report.deleted_on_disk >= 10_000);
    let log = String::from_utf8(fs.read_small(AUDIT, 1 << 20).unwrap()).unwrap();
    assert!(log.contains("\"op\":\"delete\""), "{log}");
    assert!(
        !log.contains("SomeCache"),
        "the audit log holds no paths: {log}"
    );
}

#[test]
fn s6_an_item_that_changed_after_the_preview_is_left_alone() {
    let (fs, t, c) = setup();
    let plan = plan_delete(
        &fs,
        &t,
        &[id(&t, r"C:\Users\u\AppData\Local\SomeCache")],
        &c,
    );
    // Another program replaces the folder between the preview and the click.
    fs.delete_tree(r"C:\Users\u\AppData\Local\SomeCache")
        .unwrap();
    fs.add_file(r"C:\Users\u\AppData\Local\SomeCache\new.bin", 10);
    let report = execute_delete(&fs, &plan, AUDIT, fs.now());
    assert!(matches!(report.results[0].outcome, Outcome::Failed(_)));
    assert!(fs.has(r"C:\Users\u\AppData\Local\SomeCache\new.bin"));
    assert_eq!(report.deleted_on_disk, 0);
}

#[test]
fn s6_delete_now_is_offered_only_on_safe_items_outside_sync_roots() {
    use unhoard_core::classify::describe;
    use unhoard_core::rules::Action;
    let (_, t, c) = setup();
    let offered = |p: &str| {
        describe(&t, id(&t, p), &c)
            .actions
            .contains(&Action::DeleteNow)
    };
    assert!(offered(r"C:\Users\u\AppData\Local\SomeCache"));
    assert!(!offered(r"C:\Users\u\code\node_modules"));
    assert!(!offered(r"C:\Users\u\OneDrive\cache"));
}

//! F10 report output: the JSON contract, profile redaction and unknown sizes (T-10.1, T-10.3
//! at the core level).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use unhoard_core::fake::FakeFs;
use unhoard_core::fs::FileSystem;
use unhoard_core::report::{ReportOptions, redact, scan_report};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};

#[test]
fn f10_s9_profile_path_is_redacted() {
    assert_eq!(
        redact(r"C:\Users\Alice\Downloads\x.zip", Some(r"C:\Users\Alice")),
        r"%USERPROFILE%\Downloads\x.zip"
    );
    assert_eq!(
        redact(r"c:\users\alice", Some(r"C:\Users\Alice")),
        "%USERPROFILE%"
    );
    assert_eq!(
        redact(r"C:\Users\AliceB\x", Some(r"C:\Users\Alice")),
        r"C:\Users\AliceB\x"
    );
    assert_eq!(redact(r"D:\data", Some(r"C:\Users\Alice")), r"D:\data");
}

#[test]
fn f10_report_matches_the_contract_and_never_shows_the_profile() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\Alice\Downloads").unwrap();
    fs.add_file(r"C:\Users\Alice\Downloads\setup.exe", 1000);
    fs.add_dir(r"C:\Users\Alice\Locked");
    fs.deny(r"C:\Users\Alice\Locked");
    let ctx = ScanContext {
        rules: Rules::empty(),
        roots: Roots::default(),
        now: 1_790_000_000,
    };
    let (t, outcome) = scan(&fs, r"C:\Users\Alice", &ctx, &ScanControl::default()).unwrap();
    let opts = ReportOptions {
        top: 20,
        depth: 1,
        logical: false,
        profile: Some(r"C:\Users\Alice".into()),
    };
    let r = scan_report(&t, None, outcome.partial, 12, &ctx, &opts);
    let json = serde_json::to_string(&r).unwrap();
    assert!(!json.contains("Alice"), "the profile path leaked: {json}");
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["schemaVersion"], 1);
    assert_eq!(v["root"], "%USERPROFILE%");
    assert_eq!(v["partial"], false);
    assert!(
        v["hiddenBytes"].is_null(),
        "a folder scan has no hidden space"
    );
    assert_eq!(v["totals"]["files"], 1);
    assert_eq!(v["unscanned"][0]["path"], r"%USERPROFILE%\Locked");
    assert_eq!(v["unscanned"][0]["reason"], "accessDenied");
    let locked = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["path"] == r"%USERPROFILE%\Locked")
        .unwrap();
    assert!(
        locked["logicalBytes"].is_null() && locked["onDiskBytes"].is_null(),
        "unknown size must be null, not 0"
    );
    let dl = v["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["path"] == r"%USERPROFILE%\Downloads")
        .unwrap();
    assert_eq!(dl["logicalBytes"], 1000);
    assert_eq!(dl["kind"], "folder");
    for key in [
        "schemaVersion",
        "root",
        "partial",
        "durationMs",
        "volume",
        "totals",
        "hiddenBytes",
        "unscanned",
        "items",
    ] {
        assert!(v.get(key).is_some(), "missing {key}");
    }
}

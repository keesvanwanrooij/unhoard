//! F10 CLI integration tests on generated temp trees (T-10.1 to T-10.3, AC10.3, AC10.5).
#![cfg(windows)]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::process::Command;

use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_platform::WindowsFs;

fn unhoard(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_unhoard"))
        .args(args)
        .output()
        .unwrap()
}

fn tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("photos")).unwrap();
    std::fs::write(dir.path().join("photos").join("a.jpg"), vec![1u8; 30_000]).unwrap();
    std::fs::write(dir.path().join("photos").join("b.jpg"), vec![1u8; 10_000]).unwrap();
    std::fs::write(dir.path().join("notes.txt"), b"hello").unwrap();
    dir
}

#[test]
fn f10_scan_json_matches_the_schema() {
    let dir = tree();
    let out = unhoard(&["scan", dir.path().to_str().unwrap(), "--json"]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
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
    assert_eq!(v["schemaVersion"], 1);
    assert_eq!(v["totals"]["files"], 3);
    assert_eq!(v["items"][0]["kind"], "folder");
    assert!(v["items"][0]["path"].as_str().unwrap().ends_with("photos"));
}

#[test]
fn f10_cli_and_core_report_the_same_totals() {
    let dir = tree();
    let out = unhoard(&["scan", dir.path().to_str().unwrap(), "--json", "--logical"]);
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let ctx = ScanContext {
        rules: Rules::empty(),
        roots: Roots::default(),
        now: 0,
        knowledge: Default::default(),
    };
    let (t, _) = scan(
        &WindowsFs,
        dir.path().to_str().unwrap(),
        &ctx,
        &ScanControl::default(),
    )
    .unwrap();
    let root = t.get(t.root()).unwrap();
    assert_eq!(v["totals"]["logicalBytes"], root.logical);
    assert_eq!(v["totals"]["onDiskBytes"], root.on_disk);
}

#[test]
fn f10_s9_output_replaces_the_profile_path() {
    // The CLI runs with the temp folder's parent as its profile, so the test works where
    // the temp folder lies outside the real profile (as on CI) and never touches a profile.
    let dir = tree();
    let profile = dir.path().parent().unwrap().to_str().unwrap().to_string();
    for args in [
        vec!["scan", dir.path().to_str().unwrap(), "--json"],
        vec!["scan", dir.path().to_str().unwrap()],
    ] {
        let out = Command::new(env!("CARGO_BIN_EXE_unhoard"))
            .args(&args)
            .env("USERPROFILE", &profile)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_lowercase();
        assert!(
            !text.contains(&profile.to_lowercase()),
            "profile path in output"
        );
        assert!(text.contains("%userprofile%"));
    }
}

#[test]
fn f10_exit_codes_and_no_writes() {
    let dir = tree();
    let before: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(unhoard(&["scan"]).status.code(), Some(2));
    assert_eq!(
        unhoard(&["scan", "x", "--top", "-3"]).status.code(),
        Some(2)
    );
    assert_eq!(
        unhoard(&["scan", dir.path().join("missing").to_str().unwrap()])
            .status
            .code(),
        Some(1)
    );
    let ok = unhoard(&[
        "scan",
        dir.path().to_str().unwrap(),
        "--top",
        "1",
        "--depth",
        "2",
    ]);
    assert_eq!(ok.status.code(), Some(0));
    let after: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after, "the CLI wrote to disk");
}

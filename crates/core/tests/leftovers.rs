//! F4 leftovers of uninstalled apps (AC4.14 to AC4.16, T-4.9 to T-4.11).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::classify::{Action, describe};
use unhoard_core::fake::{FakeFs, FileOpts};
use unhoard_core::fs::FileSystem;
use unhoard_core::knowledge::{Activity, Knowledge, Program, UseSource};
use unhoard_core::leftovers::{Leftover, same_app};
use unhoard_core::roots::Roots;
use unhoard_core::rules::{Rules, Tier};
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::time::unix_to_filetime;
use unhoard_core::view::suggestions;

const NOW: i64 = 1_790_000_000;
const DAY: i64 = 86_400;
const LOCAL: &str = r"C:\Users\u\AppData\Local";

fn roots() -> Roots {
    let env: BTreeMap<String, String> = [
        ("USERPROFILE", r"C:\Users\u"),
        ("LOCALAPPDATA", LOCAL),
        ("APPDATA", r"C:\Users\u\AppData\Roaming"),
        ("PROGRAMDATA", r"C:\ProgramData"),
        ("PROGRAMFILES", r"C:\Program Files"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    Roots::new(env, None, vec![])
}

fn program(name: &str, publisher: &str, location: &str) -> Program {
    Program {
        name: name.into(),
        publisher: Some(publisher.into()),
        install_location: location.into(),
        install_date: None,
        estimated_bytes: None,
    }
}

const KEEP: &str = r#"
[[rule]]
id = "def-local-microsoft"
define = true
parent = "%LOCALAPPDATA%"
name = "Microsoft"
kind = "dir"
tier = "keep"
"#;

fn old_folder(fs: &FakeFs, path: &str, days: i64) {
    fs.create_dir_all(path).unwrap();
    fs.add_file_with(
        &format!(r"{path}\data.bin"),
        FileOpts {
            logical: Some(5 << 20),
            modified: unix_to_filetime(NOW - days * DAY),
            ..FileOpts::default()
        },
    );
}

#[test]
fn f4_names_match_without_case_spaces_punctuation_and_versions() {
    assert!(same_app("Some App 3", "SomeApp"));
    assert!(same_app("some-app", "Some_App v2.1"));
    assert!(same_app("Tool 2024", "TOOL"));
    assert!(!same_app("Tool", "Toolbox"));
    assert!(!same_app("", "x"));
}

#[test]
fn f4_gone_apps_are_likely_safe_unclaimed_old_folders_review_and_the_rest_nothing() {
    let fs = FakeFs::new();
    old_folder(&fs, &format!(r"{LOCAL}\OldEditor"), 10);
    old_folder(&fs, &format!(r"{LOCAL}\Mystery"), 400);
    old_folder(&fs, &format!(r"{LOCAL}\Fresh"), 30);
    old_folder(&fs, &format!(r"{LOCAL}\Installed Tool"), 400);
    old_folder(&fs, &format!(r"{LOCAL}\Microsoft"), 400);
    old_folder(&fs, &format!(r"{LOCAL}\Packages"), 400);
    old_folder(&fs, r"C:\ProgramData\Vendor Suite", 400);
    let ctx = ScanContext {
        rules: Rules::from_toml(&[KEEP], &roots()).unwrap(),
        roots: roots(),
        now: NOW,
        knowledge: Knowledge {
            programs: vec![program(
                "Installed Tool 5",
                "Tool Corp",
                r"C:\Program Files\Installed Tool",
            )],
            activity: vec![],
            gone: vec![Activity {
                path: r"C:\Program Files\Old Editor\editor.exe".into(),
                at: NOW - 200 * DAY,
                source: UseSource::Started,
            }],
            steam: vec![],
        },
    };
    let (t, _) = scan(&fs, r"C:\", &ctx, &ScanControl::default()).unwrap();
    let d = |p: &str| describe(&t, t.find(p).unwrap(), &ctx);

    let editor = d(&format!(r"{LOCAL}\OldEditor"));
    assert_eq!(
        editor.leftover,
        Some(Leftover::GoneApp {
            name: "Old Editor".into(),
            last_run: NOW - 200 * DAY
        })
    );
    assert_eq!(editor.tier, Some(Tier::LikelySafe));
    assert_eq!(
        editor.actions,
        vec![Action::Quarantine, Action::ShowInExplorer]
    );

    let mystery = d(&format!(r"{LOCAL}\Mystery"));
    assert!(
        matches!(mystery.leftover, Some(Leftover::Unclaimed { unchanged_days }) if unchanged_days >= 399)
    );
    assert_eq!(mystery.tier, Some(Tier::Review));
    let suite = d(r"C:\ProgramData\Vendor Suite");
    assert!(matches!(suite.leftover, Some(Leftover::Unclaimed { .. })));

    for keep in ["Fresh", "Installed Tool", "Microsoft", "Packages"] {
        assert_eq!(d(&format!(r"{LOCAL}\{keep}")).leftover, None, "{keep}");
    }

    let listed: Vec<String> = suggestions(&t, &ctx, 100)
        .into_iter()
        .map(|s| s.path)
        .collect();
    assert!(listed.contains(&format!(r"{LOCAL}\OldEditor")));
    assert!(listed.contains(&format!(r"{LOCAL}\Mystery")));
    assert!(!listed.contains(&format!(r"{LOCAL}\Fresh")));
    assert!(!listed.iter().any(|p| p.ends_with("Installed Tool")));
}

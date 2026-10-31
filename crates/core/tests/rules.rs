//! F4 known locations: fixtures, text, precedence, the default case and protection
//! (T-4.1 to T-4.5).
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
use unhoard_core::protect::{ProtectReason, protection};
use unhoard_core::roots::{Roots, parent_of};
use unhoard_core::rules::{RuleKind, Rules, Tier};
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::time::unix_to_filetime;

const NOW: i64 = 1_790_000_000;

fn roots() -> Roots {
    let env: BTreeMap<String, String> = [
        ("WINDIR", r"C:\Windows"),
        ("SYSTEMDRIVE", "C:"),
        ("PROGRAMFILES", r"C:\Program Files"),
        ("PROGRAMFILES(X86)", r"C:\Program Files (x86)"),
        ("PROGRAMDATA", r"C:\ProgramData"),
        ("USERPROFILE", r"C:\Users\u"),
        ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
        ("APPDATA", r"C:\Users\u\AppData\Roaming"),
        ("TEMP", r"C:\Users\u\AppData\Local\Temp"),
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

/// Creates `path` as the rule expects it: a folder or a file, old and large enough.
fn materialise(fs: &FakeFs, path: &str, rule: &unhoard_core::rules::Rule) {
    let parent = parent_of(path).unwrap();
    fs.create_dir_all(&parent).unwrap();
    let old = unix_to_filetime(NOW - (rule.min_age_days.unwrap_or(0) as i64 + 2) * 86_400);
    match rule.kind {
        RuleKind::Dir => {
            fs.add_dir(path);
        }
        _ => {
            fs.add_file_with(
                path,
                FileOpts {
                    logical: Some(rule.min_bytes.unwrap_or(4096).max(4096)),
                    modified: old,
                    ..FileOpts::default()
                },
            );
        }
    }
}

fn best_rule_id(rules: &Rules, fs: &FakeFs, path: &str) -> Option<String> {
    let parent = parent_of(path).unwrap();
    let entries = fs.read_dir(&parent).unwrap();
    let name = unhoard_core::roots::file_name(path).to_lowercase();
    let matches = rules.match_listing(&parent, &entries, NOW);
    let i = entries.iter().position(|e| e.name.to_lowercase() == name)?;
    matches[i].and_then(|r| rules.get(r)).map(|r| r.id.clone())
}

#[test]
fn f4_every_rule_matches_its_fixture_and_not_its_near_miss() {
    let rules = Rules::builtin(&roots()).unwrap();
    assert!(rules.all().len() >= 40, "the MVP library has its entries");
    for rule in rules.all() {
        let example = rule
            .example
            .clone()
            .unwrap_or_else(|| panic!("{} has no example", rule.id));
        let near = rule
            .near_miss
            .clone()
            .unwrap_or_else(|| panic!("{} has no nearMiss", rule.id));
        let fs = FakeFs::new();
        materialise(&fs, &example, rule);
        let parent = parent_of(&example).unwrap();
        if let Some(sib) = &rule.sibling {
            fs.add_file(
                &unhoard_core::roots::join(&parent, &sib.replace('*', "project")),
                1,
            );
        }
        if rule.stem_sibling {
            let (stem, _) =
                unhoard_core::roots::split_ext(unhoard_core::roots::file_name(&example));
            fs.add_dir(&unhoard_core::roots::join(&parent, stem));
        }
        assert_eq!(
            best_rule_id(&rules, &fs, &example).as_deref(),
            Some(rule.id.as_str()),
            "fixture of {}",
            rule.id
        );
        let fs = FakeFs::new();
        materialise(&fs, &near, rule);
        assert_ne!(
            best_rule_id(&rules, &fs, &near).as_deref(),
            Some(rule.id.as_str()),
            "near miss of {}",
            rule.id
        );
    }
}

#[test]
fn f4_s4_project_rules_never_match_inside_installed_apps_or_dependencies() {
    let rules = Rules::builtin(&roots()).unwrap();
    // Dependencies an app or extension runs from: moving them breaks the app.
    let app_owned = [
        r"C:\Users\u\AppData\Local\Programs\editor\resources\app\node_modules",
        r"C:\Program Files\nodejs\node_modules\npm\node_modules",
        r"C:\Users\u\AppData\Roaming\npm\node_modules\tool\node_modules",
        r"C:\Users\u\.vscode\extensions\ext-1.0.0\node_modules",
        r"C:\Users\u\.agent\extensions\chat\node_modules",
        r"C:\ProgramData\Vendor\Plugin\node_modules",
        r"C:\code\mono\node_modules\pkg\node_modules",
        r"C:\Users\u\AppData\Roaming\app\runtime\python\.venv",
        r"C:\Users\u\AppData\Local\app\target",
    ];
    for path in app_owned {
        let fs = FakeFs::new();
        fs.create_dir_all(&parent_of(path).unwrap()).unwrap();
        fs.add_dir(path);
        let parent = parent_of(path).unwrap();
        for sib in ["package.json", "Cargo.toml"] {
            fs.add_file(&unhoard_core::roots::join(&parent, sib), 1);
        }
        assert_eq!(best_rule_id(&rules, &fs, path), None, "{path}");
    }
    // The user's own projects still match.
    for (path, id) in [
        (r"C:\code\app\node_modules", "node-modules"),
        (
            r"C:\Users\u\OneDrive\archive\app\node_modules",
            "node-modules",
        ),
        (r"C:\Users\u\.venv", "python-venv"),
        (r"C:\Users\u\Documents\tool\target", "rust-target"),
    ] {
        let fs = FakeFs::new();
        fs.create_dir_all(&parent_of(path).unwrap()).unwrap();
        fs.add_dir(path);
        let parent = parent_of(path).unwrap();
        for sib in ["package.json", "Cargo.toml"] {
            fs.add_file(&unhoard_core::roots::join(&parent, sib), 1);
        }
        assert_eq!(
            best_rule_id(&rules, &fs, path).as_deref(),
            Some(id),
            "{path}"
        );
    }
}

#[test]
fn f4_every_rule_has_english_and_dutch_text() {
    let en: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/i18n/en.json")).unwrap();
    let nl: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/i18n/nl.json")).unwrap();
    let rules = Rules::builtin(&roots()).unwrap();
    for rule in rules.all() {
        for (lang, doc) in [("en", &en), ("nl", &nl)] {
            for field in ["title", "explain", "reason", "action"] {
                let v = doc["knownLocations"][&rule.id][field]
                    .as_str()
                    .unwrap_or("");
                assert!(
                    !v.trim().is_empty(),
                    "{lang}: knownLocations.{}.{field} is missing",
                    rule.id
                );
            }
        }
    }
}

const PRECEDENCE: &str = r#"
[[rule]]
id = "broad-cache"
parent = "**"
name = "cache"
kind = "dir"
category = "other"
tier = "safe"
action = "quarantine"

[[rule]]
id = "specific-cache"
parent = "%LOCALAPPDATA%\\Vendor"
name = "cache"
kind = "dir"
category = "developer"
tier = "review"
action = "quarantine"

[[rule]]
id = "tie-safe"
parent = "C:\\tie"
name = "thing"
kind = "dir"
category = "other"
tier = "safe"
action = "quarantine"

[[rule]]
id = "tie-keep"
parent = "C:\\tie"
name = "thing"
kind = "dir"
category = "system"
tier = "keep"
action = "explainOnly"

[[rule]]
id = "inside-windows"
parent = "%WINDIR%"
name = "leftovers"
kind = "dir"
category = "system"
tier = "safe"
action = "quarantine"
"#;

#[test]
fn f4_most_specific_rule_wins_and_strictest_tier_on_a_tie() {
    let rules = Rules::from_toml(&[PRECEDENCE], &roots()).unwrap();
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\AppData\Local\Vendor\cache")
        .unwrap();
    fs.create_dir_all(r"C:\somewhere\cache").unwrap();
    fs.create_dir_all(r"C:\tie\thing").unwrap();
    assert_eq!(
        best_rule_id(&rules, &fs, r"C:\Users\u\AppData\Local\Vendor\cache").as_deref(),
        Some("specific-cache")
    );
    assert_eq!(
        best_rule_id(&rules, &fs, r"C:\somewhere\cache").as_deref(),
        Some("broad-cache")
    );
    assert_eq!(
        best_rule_id(&rules, &fs, r"C:\tie\thing").as_deref(),
        Some("tie-keep")
    );
    assert!(
        Tier::Keep > Tier::Review
            && Tier::Review > Tier::LikelySafe
            && Tier::LikelySafe > Tier::Safe
    );
}

#[test]
fn f4_unmatched_large_item_gets_the_generic_explanation_and_tier_review() {
    let fs = FakeFs::new();
    fs.add_dir(r"C:\data");
    fs.add_file_with(
        r"C:\data\movie.mkv",
        FileOpts {
            logical: Some(12 << 30),
            modified: unix_to_filetime(NOW - 3 * 365 * 86_400),
            ..FileOpts::default()
        },
    );
    fs.add_file(r"C:\data\small.txt", 10);
    let ctx = ScanContext {
        rules: Rules::from_toml(&[], &roots()).unwrap(),
        roots: roots(),
        now: NOW,
    };
    let (t, _) = scan(&fs, r"C:\data", &ctx, &ScanControl::default()).unwrap();
    let d = describe(&t, t.find(r"C:\data\movie.mkv").unwrap(), &ctx);
    assert!(d.generic);
    assert_eq!(d.rule_id, None);
    assert_eq!(d.tier, Some(Tier::Review));
    assert_eq!(d.actions, vec![Action::ShowInExplorer, Action::Quarantine]);
    let small = describe(&t, t.find(r"C:\data\small.txt").unwrap(), &ctx);
    assert!(!small.generic);
    assert_eq!(small.tier, None);
}

#[test]
fn f4_s4_protected_paths_never_get_a_move_action() {
    let r = roots();
    for p in [
        r"C:\Windows",
        r"C:\Windows\System32\drivers",
        r"C:\Program Files\App",
        r"C:\Program Files (x86)\Steam",
        r"C:\ProgramData\Microsoft\Windows",
        r"C:\ProgramData\Package Cache\{x}",
        r"C:\Users",
        r"C:\Users\u",
        r"C:\Users\someone-else",
        r"C:\",
        r"D:\",
        r"C:\System Volume Information",
        r"C:\$Recycle.Bin\S-1-5",
        r"C:\Unhoard Quarantine",
        r"C:\Unhoard Quarantine\2026-10-31\x",
        r"C:\Users\u\OneDrive\Unhoard Quarantine\y",
    ] {
        assert!(protection(p, &r).is_some(), "{p} must be protected");
    }
    for p in [
        r"C:\Users\u\Downloads\a.zip",
        r"C:\ProgramData\SomeApp",
        r"C:\Program Files Extra\x",
        r"C:\Users\u\AppData",
    ] {
        assert_eq!(protection(p, &r), None, "{p} must not be protected");
    }
    assert_eq!(
        protection(r"C:\Windows\Temp", &r),
        Some(ProtectReason::Windows)
    );

    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Windows\leftovers").unwrap();
    fs.add_file(r"C:\Windows\leftovers\f.bin", 1);
    let ctx = ScanContext {
        rules: Rules::from_toml(&[PRECEDENCE], &r).unwrap(),
        roots: r,
        now: NOW,
    };
    let (t, _) = scan(&fs, r"C:\Windows", &ctx, &ScanControl::default()).unwrap();
    let d = describe(&t, t.find(r"C:\Windows\leftovers").unwrap(), &ctx);
    assert_eq!(d.rule_id.as_deref(), Some("inside-windows"));
    assert!(d.protected.is_some());
    assert!(!d.actions.contains(&Action::Quarantine));
}

#[test]
fn f6_s3_cloud_items_never_offer_the_local_quarantine() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\OneDrive\Pictures").unwrap();
    fs.add_placeholder(r"C:\Users\u\OneDrive\Pictures\big.mov", 3 << 30, true);
    let ctx = ScanContext {
        rules: Rules::from_toml(&[], &roots()).unwrap(),
        roots: roots(),
        now: NOW,
    };
    let (t, _) = scan(&fs, r"C:\Users\u\OneDrive", &ctx, &ScanControl::default()).unwrap();
    let d = describe(
        &t,
        t.find(r"C:\Users\u\OneDrive\Pictures\big.mov").unwrap(),
        &ctx,
    );
    assert!(d.cloud);
    let q = d.quarantine_root.unwrap();
    assert!(
        unhoard_core::roots::is_within(&q, r"C:\Users\u\OneDrive"),
        "cloud quarantine stays in the sync root: {q}"
    );
}

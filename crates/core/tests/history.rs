//! F17 storage history: snapshots, retention, growth attribution and the history file
//! (T-17.1 to T-17.5).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::fake::FakeFs;
use unhoard_core::fs::FileSystem;
use unhoard_core::history::{
    DAY, Folder, MAX_FOLDERS, MIN_GROWTH, Snapshot, SnapshotKind, Totals, baseline, growth, parse,
    retain, snapshot, to_line,
};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};

const NOW: i64 = 1_790_000_000;
const MB: u64 = 1 << 20;
const GB: u64 = 1 << 30;

fn ctx(rules: &[&str]) -> ScanContext {
    let env: BTreeMap<String, String> = [("LOCALAPPDATA", r"C:\Users\u\AppData\Local")]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let roots = Roots::new(env, None, vec![]);
    ScanContext {
        rules: Rules::from_toml(rules, &roots).unwrap(),
        roots,
        now: NOW,
        knowledge: Default::default(),
    }
}

fn snap(at: i64, folders: &[(&str, u64)]) -> Snapshot {
    Snapshot {
        at,
        kind: SnapshotKind::Daily,
        root: r"C:\".into(),
        totals: Totals::default(),
        folders: folders
            .iter()
            .map(|(p, s)| Folder {
                path: (*p).to_string(),
                on_disk: *s,
                files: 1,
            })
            .collect(),
    }
}

const DEEP_RULE: &str = r#"
[[rule]]
id = "deep-cache"
parent = "C:\\a\\b\\c"
name = "cache"
kind = "dir"
category = "other"
tier = "safe"
action = "quarantine"
"#;

#[test]
fn f17_a_snapshot_holds_the_top_folders_and_rule_matches_but_no_deeper_names() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\a\b\c\cache").unwrap();
    fs.add_file(r"C:\a\b\c\cache\blob.bin", 3 * MB);
    fs.add_file(r"C:\a\b\c\d.bin", 2 * MB);
    fs.add_file(r"C:\a\x.bin", MB);
    fs.create_dir_all(r"C:\e").unwrap();
    fs.add_file(r"C:\e\f.bin", 4 * MB);
    let ctx = ctx(&[DEEP_RULE]);
    let (tree, _) = scan(&fs, r"C:\", &ctx, &ScanControl::default()).unwrap();
    let totals = Totals {
        used: Some(100 * GB),
        free: Some(20 * GB),
        hidden: Some(5 * GB),
    };
    let s = snapshot(&tree, &ctx, NOW, SnapshotKind::Scan, totals);
    let paths: Vec<&str> = s.folders.iter().map(|f| f.path.as_str()).collect();
    for want in [r"C:\a", r"C:\a\b", r"C:\a\b\c", r"C:\e"] {
        assert!(paths.contains(&want), "{want} missing in {paths:?}");
    }
    // The rule match sits four levels deep and is kept for that reason.
    assert!(paths.contains(&r"C:\a\b\c\cache"));
    // Files are never listed, only folders.
    assert!(!paths.iter().any(|p| p.ends_with(".bin")));
    assert_eq!(s.totals, totals);
    assert!(s.folders.len() <= MAX_FOLDERS);
    let a = s.folders.iter().find(|f| f.path == r"C:\a").unwrap();
    assert!(a.on_disk >= 6 * MB);
}

#[test]
fn f17_retention_keeps_days_for_90_days_then_weeks_for_two_years_and_caps_the_size() {
    let mut all = vec![];
    for d in 0..900 {
        all.push(snap(NOW - d * DAY, &[(r"C:\a", 1)]));
    }
    let kept = retain(all, NOW, usize::MAX);
    let ages: Vec<i64> = kept.iter().map(|s| (NOW - s.at) / DAY).collect();
    for d in 0..90 {
        assert!(ages.contains(&d), "day {d} must stay");
    }
    let old: Vec<i64> = ages.iter().copied().filter(|a| *a >= 90).collect();
    assert!(
        old.iter().all(|a| *a <= 730),
        "nothing older than two years"
    );
    assert!(
        (90..=110).contains(&old.len()),
        "about one per week: {}",
        old.len()
    );
    let mut sorted = kept.clone();
    sorted.sort_by_key(|s| s.at);
    assert_eq!(kept, sorted, "oldest first");
    // The size cap drops the oldest first.
    let line = to_line(&kept[0]).len() + 1;
    let capped = retain(kept.clone(), NOW, line * 10);
    assert_eq!(capped.len(), 10);
    assert_eq!(capped.last(), kept.last());
}

#[test]
fn f17_growth_is_attributed_to_the_deepest_folder_holding_80_percent() {
    let temp = r"C:\Users\u\AppData\Local\Temp";
    let traces = r"C:\Users\u\AppData\Local\Temp\DiagOutputDir\RdClientAutoTrace";
    let diag = r"C:\Users\u\AppData\Local\Temp\DiagOutputDir";
    let old = snap(
        NOW - 7 * DAY,
        &[
            (temp, 2 * GB),
            (diag, 0),
            (traces, 0),
            (r"C:\Games", 40 * GB),
            (r"C:\Quarantine", 31 * GB),
        ],
    );
    let new = snap(
        NOW,
        &[
            (temp, 56 * GB),
            (diag, 54 * GB),
            (traces, 54 * GB),
            (r"C:\Games", 40 * GB + 50 * MB),
            (r"C:\New", 3 * GB),
        ],
    );
    let g = growth(&old, &new);
    let paths: Vec<&str> = g.iter().map(|x| x.path.as_str()).collect();
    assert_eq!(g[0].path, traces, "the cause, not its parents: {paths:?}");
    assert_eq!(g[0].delta, 54 * GB as i64);
    assert!(!paths.contains(&temp) && !paths.contains(&diag));
    assert!(
        !paths.contains(&r"C:\Games"),
        "under {MIN_GROWTH} bytes is noise"
    );
    let gone = g.iter().find(|x| x.path == r"C:\Quarantine").unwrap();
    assert_eq!(gone.delta, -(31 * GB as i64));
    assert_eq!(gone.after, 0);
    let fresh = g.iter().find(|x| x.path == r"C:\New").unwrap();
    assert_eq!(fresh.before, 0);
    // Largest change first, whatever its sign.
    assert!(g.windows(2).all(|w| w[0].delta.abs() >= w[1].delta.abs()));
}

#[test]
fn f17_a_parent_that_grew_in_several_places_keeps_its_own_line() {
    let old = snap(NOW - DAY, &[(r"C:\p", 0), (r"C:\p\x", 0), (r"C:\p\y", 0)]);
    let new = snap(
        NOW,
        &[(r"C:\p", 10 * GB), (r"C:\p\x", 5 * GB), (r"C:\p\y", 5 * GB)],
    );
    let g = growth(&old, &new);
    assert_eq!(g[0].path, r"C:\p");
}

#[test]
fn f17_a_broken_line_is_skipped_and_counted() {
    let a = snap(NOW - DAY, &[(r"C:\a", 1)]);
    let b = snap(NOW, &[(r"C:\a", 2)]);
    let text = format!("{}\nnot json\n{}\n\n", to_line(&a), to_line(&b));
    let (list, skipped) = parse(&text);
    assert_eq!(list, vec![a, b]);
    assert_eq!(skipped, 1);
}

#[test]
fn f17_the_baseline_is_the_nearest_snapshot_at_or_before_the_moment() {
    let list = vec![
        snap(NOW - 10 * DAY, &[]),
        snap(NOW - 6 * DAY, &[]),
        snap(NOW - DAY, &[]),
        snap(NOW, &[]),
    ];
    assert_eq!(
        baseline(&list, NOW - 7 * DAY).map(|s| s.at),
        Some(NOW - 10 * DAY)
    );
    assert_eq!(
        baseline(&list, NOW - 6 * DAY).map(|s| s.at),
        Some(NOW - 6 * DAY)
    );
    assert_eq!(baseline(&list, NOW - 20 * DAY).map(|s| s.at), None);
}

#[test]
fn f17_a_stored_snapshot_is_compact_and_reads_back_the_same() {
    let mut folders = vec![];
    for a in 0..10 {
        let top = format!(r"C:\Users\someone\Folder number {a}");
        folders.push((top.clone(), 10 * GB));
        for b in 0..99 {
            folders.push((format!(r"{top}\A rather long sub folder name {b}"), GB));
        }
    }
    folders.push((r"C:\Users".to_string(), 100 * GB));
    folders.push((r"C:\Users\someone".to_string(), 100 * GB));
    let refs: Vec<(&str, u64)> = folders.iter().map(|(p, s)| (p.as_str(), *s)).collect();
    let mut s = snap(NOW, &refs);
    s.folders.sort_by(|a, b| a.path.cmp(&b.path));
    let line = to_line(&s);
    let (back, skipped) = parse(&line);
    assert_eq!(skipped, 0);
    assert_eq!(back, vec![s.clone()]);
    let plain: usize = s.folders.iter().map(|f| f.path.len()).sum();
    // The stored line is smaller than the folder paths alone would be.
    assert!(
        line.len() < plain,
        "{} bytes for {plain} bytes of paths",
        line.len()
    );
    // A folder whose parent is not kept still reads back with its full path.
    let lone = snap(NOW, &[(r"C:\a\b\c", 1)]);
    assert_eq!(parse(&to_line(&lone)).0, vec![lone]);
}

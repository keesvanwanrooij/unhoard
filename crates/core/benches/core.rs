//! Core benchmarks for the performance budgets (T-P1): a view query and a subtree rescan on
//! a generated tree of one million entries, and rule matching of a directory listing.
#![allow(
    missing_docs,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "benchmark harness"
)]

use std::collections::BTreeMap;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use unhoard_core::fs::{DirEntry, FileId};
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::ScanContext;
use unhoard_core::tree::{Category, ChildSpec, NodeId, NodeKind, Tree};
use unhoard_core::view::view;

fn spec(name: String, kind: NodeKind, size: u64, id: u128) -> ChildSpec {
    ChildSpec {
        name,
        kind,
        logical: size,
        on_disk: size.div_ceil(4096) * 4096,
        attributes: 0,
        file_id: FileId(id),
        modified: 1,
        category: Category::Other,
        rule: None,
        flags: 0,
    }
}

/// A tree of about `target` entries: folders of 40 files, 20 subfolders deep in places.
fn generated(target: usize) -> (Tree, NodeId) {
    let mut t = Tree::new(r"C:\", 1);
    let mut next = 1u128;
    let mut folders = vec![t.root()];
    let mut count = 0;
    let mut i = 0;
    let mut sample = t.root();
    while count < target {
        let parent = folders[i % folders.len()];
        i += 1;
        for k in 0..8 {
            let f = t
                .add_child(
                    parent,
                    spec(format!("dir{count}-{k}"), NodeKind::Folder, 0, next),
                )
                .unwrap();
            next += 1;
            count += 1;
            folders.push(f);
            for j in 0..40 {
                t.add_child(
                    f,
                    spec(
                        format!("f{j}.bin"),
                        NodeKind::File,
                        (j as u64 + 1) * 37_000,
                        next,
                    ),
                );
                next += 1;
                count += 1;
            }
            if count > target / 2 && sample == t.root() {
                sample = f;
            }
        }
    }
    (t, sample)
}

fn benches(c: &mut Criterion) {
    let ctx = ScanContext {
        rules: Rules::empty(),
        roots: Roots::default(),
        now: 0,
    };
    let (mut tree, sample) = generated(1_000_000);
    c.bench_function("core.viewQuery", |b| {
        b.iter(|| black_box(view(&tree, tree.root(), 120, false, &ctx)))
    });
    c.bench_function("core.viewQueryDeep", |b| {
        b.iter(|| black_box(view(&tree, sample, 120, false, &ctx)))
    });
    c.bench_function("core.rescan10k", |b| {
        b.iter(|| {
            tree.clear_children(sample);
            for j in 0..10_000u128 {
                tree.add_child(
                    sample,
                    spec(format!("r{j}"), NodeKind::File, 4096, 10_000_000_000 + j),
                );
            }
        })
    });
    let env: BTreeMap<String, String> = [
        ("WINDIR", r"C:\Windows"),
        ("LOCALAPPDATA", r"C:\Users\u\AppData\Local"),
        ("APPDATA", r"C:\Users\u\AppData\Roaming"),
        ("USERPROFILE", r"C:\Users\u"),
        ("PROGRAMDATA", r"C:\ProgramData"),
        ("PROGRAMFILES", r"C:\Program Files"),
        ("PROGRAMFILES(X86)", r"C:\Program Files (x86)"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let rules = Rules::builtin(&Roots::new(
        env,
        Some(r"C:\Users\u\Downloads".into()),
        vec![],
    ))
    .unwrap();
    let listing: Vec<DirEntry> = (0..200)
        .map(|i| DirEntry {
            name: if i % 50 == 0 {
                "node_modules".into()
            } else {
                format!("entry{i}")
            },
            lossy_name: false,
            is_dir: i % 3 == 0,
            logical: 1000,
            on_disk: 4096,
            attributes: 0,
            reparse_tag: 0,
            file_id: FileId(i),
            modified: 1,
            created: 1,
        })
        .collect();
    c.bench_function("core.ruleMatch200", |b| {
        b.iter(|| black_box(rules.match_listing(r"C:\code\project", &listing, 0)))
    });
}

criterion_group! {
    name = core;
    config = Criterion::default().sample_size(20).warm_up_time(std::time::Duration::from_millis(500)).measurement_time(std::time::Duration::from_secs(3));
    targets = benches
}
criterion_main!(core);

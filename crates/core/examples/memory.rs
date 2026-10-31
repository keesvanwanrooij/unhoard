//! Memory check of the scan tree (architecture doc, "Memory budget"): builds a generated
//! tree of N entries with realistic name lengths, then holds it for a few seconds so the
//! peak working set of the process can be read.
//!
//! `cargo run --release -p unhoard-core --example memory -- 1000000`

use std::hint::black_box;
use std::time::Duration;

use unhoard_core::fs::FileId;
use unhoard_core::tree::{Category, ChildSpec, NodeKind, Tree};

/// One entry of the generated tree.
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

/// Builds about `target` entries: every folder holds 40 files and up to 4 subfolders, so
/// 1 million entries are about 8 levels deep. File names are 29 characters and folder
/// names 21, near the average of a real profile.
fn build(target: usize) -> Tree {
    let mut tree = Tree::new(r"C:\", 1);
    let mut folders = vec![tree.root()];
    let mut next = 1u128;
    let mut count = 0usize;
    while count < target {
        // The k-th new folder goes into folder (k - 1) / 4: a fan-out of 4.
        let parent = folders[(folders.len() - 1) / 4];
        let Some(folder) = tree.add_child(
            parent,
            spec(
                format!("Project folder {count:06}"),
                NodeKind::Folder,
                0,
                next,
            ),
        ) else {
            break;
        };
        next += 1;
        count += 1;
        folders.push(folder);
        for j in 0..40u64 {
            tree.add_child(
                folder,
                spec(
                    format!("IMG_20190612_{count:08}_{j:03}.jpg"),
                    NodeKind::File,
                    (j + 1) * 37_000,
                    next,
                ),
            );
            next += 1;
            count += 1;
        }
    }
    tree
}

fn main() {
    let target = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(1_000_000);
    let tree = build(target);
    println!("entries: {}", tree.len());
    black_box(&tree);
    std::thread::sleep(Duration::from_secs(3));
    black_box(&tree);
}

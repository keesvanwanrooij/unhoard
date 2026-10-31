//! F2 view queries and F4 suggestions: what the UI asks the core for.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::fake::FakeFs;
use unhoard_core::fs::FileSystem;
use unhoard_core::roots::Roots;
use unhoard_core::rules::{Rules, Tier};
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::view::{suggestions, view};

const RULES: &str = r#"
[[rule]]
id = "node-modules"
parent = "**"
name = "node_modules"
kind = "dir"
sibling = "package.json"
category = "developer"
tier = "likelySafe"
action = "quarantine"
"#;

fn setup() -> (FakeFs, unhoard_core::tree::Tree, ScanContext) {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\code\app\node_modules\dep\node_modules")
        .unwrap();
    fs.add_file(r"C:\code\app\package.json", 1);
    fs.add_file(r"C:\code\app\node_modules\dep\package.json", 1);
    fs.add_file(r"C:\code\app\node_modules\dep\node_modules\x.js", 3000);
    fs.add_file(r"C:\code\app\node_modules\big.js", 9000);
    for i in 0..30 {
        fs.add_file(&format!(r"C:\code\f{i}.txt"), 100 + i);
    }
    let env: BTreeMap<String, String> = BTreeMap::new();
    let ctx = ScanContext {
        rules: Rules::from_toml(&[RULES], &Roots::new(env.clone(), None, vec![])).unwrap(),
        roots: Roots::new(env, None, vec![]),
        now: fs.now(),
    };
    let (t, _) = scan(&fs, r"C:\code", &ctx, &ScanControl::default()).unwrap();
    (fs, t, ctx)
}

#[test]
fn f2_view_returns_the_largest_children_and_the_rest_as_others() {
    let (_, t, ctx) = setup();
    let v = view(&t, t.root(), 10, false, &ctx).unwrap();
    assert_eq!(v.children.len(), 10);
    assert_eq!(v.children[0].name, "app");
    assert!(v.children.windows(2).all(|w| w[0].on_disk >= w[1].on_disk));
    assert_eq!(v.others.count, 21);
    assert_eq!(v.breadcrumb.len(), 1);
    let app = v.children[0].id;
    let inner = view(&t, app, 10, false, &ctx).unwrap();
    assert_eq!(
        inner
            .breadcrumb
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>(),
        [r"C:\code", "app"]
    );
    let nm = inner
        .children
        .iter()
        .find(|c| c.name == "node_modules")
        .unwrap();
    assert_eq!(nm.tier, Some(Tier::LikelySafe));
    assert!(nm.has_children);
}

#[test]
fn f4_suggestions_list_matches_by_size_without_nested_matches() {
    let (_, t, ctx) = setup();
    let s = suggestions(&t, &ctx, 100);
    let paths: Vec<&str> = s.iter().map(|x| x.path.as_str()).collect();
    assert_eq!(
        paths,
        [r"C:\code\app\node_modules"],
        "nested node_modules must not be listed twice"
    );
    assert_eq!(s[0].tier, Tier::LikelySafe);
}

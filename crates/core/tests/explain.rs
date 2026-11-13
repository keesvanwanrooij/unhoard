//! F4 explanations for every item: the place an item lies in, the app it belongs to, the
//! type of a file, what a folder mostly holds, and definitions that explain a known folder
//! without suggesting it.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::classify::{Action, describe};
use unhoard_core::explain::{FileType, Place, file_type, place_of};
use unhoard_core::fake::{FakeFs, FileOpts};
use unhoard_core::fs::FileSystem;
use unhoard_core::roots::Roots;
use unhoard_core::rules::{Rules, Tier};
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::time::unix_to_filetime;
use unhoard_core::tree::Category;
use unhoard_core::view::suggestions;

const NOW: i64 = 1_790_000_000;

fn roots() -> Roots {
    let env: BTreeMap<String, String> = [
        ("WINDIR", r"C:\Windows"),
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

fn ctx(rules: &[&str]) -> ScanContext {
    ScanContext {
        rules: Rules::from_toml(rules, &roots()).unwrap(),
        roots: roots(),
        now: NOW,
        knowledge: Default::default(),
    }
}

#[test]
fn f4_every_path_lies_in_a_place_and_names_the_app_it_belongs_to() {
    let r = roots();
    let cases: [(&str, Place, Option<&str>); 22] = [
        (r"C:\Users\u\Downloads\a.zip", Place::Downloads, None),
        (r"C:\Users\u\Desktop\notes.txt", Place::Desktop, None),
        (r"C:\Users\u\Documents\Tax\2019.pdf", Place::Documents, None),
        (r"C:\Users\u\Pictures\Trip", Place::Pictures, None),
        (r"C:\Users\u\Videos\clip.mp4", Place::Videos, None),
        (r"C:\Users\u\Music\Album", Place::Music, None),
        (
            r"C:\Users\u\OneDrive\Afbeeldingen\2012",
            Place::Pictures,
            None,
        ),
        (
            r"C:\Users\u\OneDrive\Documenten\x.docx",
            Place::Documents,
            None,
        ),
        (r"C:\Users\u\OneDrive\Archive\old", Place::Cloud, None),
        (
            r"C:\Users\u\AppData\Local\Vendor\Cache\f.bin",
            Place::AppData,
            Some("Vendor"),
        ),
        (
            r"C:\Users\u\AppData\Roaming\Tool",
            Place::AppData,
            Some("Tool"),
        ),
        (
            r"C:\Users\u\AppData\LocalLow\Studio\Game",
            Place::AppData,
            Some("Studio"),
        ),
        (
            r"C:\Users\u\AppData\Local\Programs\Editor\bin",
            Place::UserPrograms,
            Some("Editor"),
        ),
        (
            r"C:\Users\u\AppData\Local\Packages\Vendor.App_8wekyb3d8bbwe\LocalState",
            Place::StoreApp,
            Some("Vendor.App"),
        ),
        (r"C:\Users\u\AppData\Local\Temp\x.tmp", Place::Temp, None),
        (
            r"C:\Program Files\Suite\app.exe",
            Place::ProgramFiles,
            Some("Suite"),
        ),
        (
            r"C:\Program Files (x86)\Old Tool",
            Place::ProgramFiles,
            Some("Old Tool"),
        ),
        (
            r"C:\ProgramData\Vendor\db",
            Place::ProgramData,
            Some("Vendor"),
        ),
        (r"C:\Windows\System32\drivers", Place::Windows, None),
        (r"C:\Users\u\projects\thing", Place::Profile, None),
        (r"C:\Users\other\Desktop", Place::OtherUser, Some("other")),
        (r"D:\Backups\2020", Place::Drive, None),
    ];
    for (path, place, owner) in cases {
        let (p, o) = place_of(path, &r);
        assert_eq!(p, place, "{path}");
        assert_eq!(o.as_deref(), owner, "{path}");
    }
    // Similar names are not the same folder.
    assert_eq!(place_of(r"C:\Program Files Extra\x", &r).0, Place::Drive);
    assert_eq!(place_of(r"C:\Users\u\Downloads2\x", &r).0, Place::Profile);
}

#[test]
fn f4_a_file_type_comes_from_the_extension() {
    let cases = [
        ("movie.MKV", Some(FileType::Video)),
        ("IMG_0001.HEIC", Some(FileType::Photo)),
        ("shot.cr3", Some(FileType::RawPhoto)),
        ("song.flac", Some(FileType::Audio)),
        ("backup.7z", Some(FileType::Archive)),
        ("linux.iso", Some(FileType::DiskImage)),
        ("ext4.vhdx", Some(FileType::VirtualDisk)),
        ("setup.msi", Some(FileType::Installer)),
        ("tool.exe", Some(FileType::Program)),
        ("lib.dll", Some(FileType::Library)),
        ("report.docx", Some(FileType::Document)),
        ("sheet.xlsx", Some(FileType::Spreadsheet)),
        ("deck.pptx", Some(FileType::Presentation)),
        ("scan.pdf", Some(FileType::Pdf)),
        ("book.epub", Some(FileType::Ebook)),
        ("notes.txt", Some(FileType::Text)),
        ("main.rs", Some(FileType::Code)),
        ("data.json", Some(FileType::Data)),
        ("app.sqlite", Some(FileType::Database)),
        ("run.log", Some(FileType::Log)),
        ("old.bak", Some(FileType::Backup)),
        ("crash.dmp", Some(FileType::Dump)),
        ("model.gguf", Some(FileType::AiModel)),
        ("textures.pak", Some(FileType::GameData)),
        ("font.ttf", Some(FileType::Font)),
        ("mail.pst", Some(FileType::Mail)),
        ("poster.psd", Some(FileType::Design)),
        ("film.crdownload", Some(FileType::Partial)),
        ("x.tmp", Some(FileType::Temporary)),
        ("link.lnk", Some(FileType::Shortcut)),
        ("driver.sys", Some(FileType::System)),
        ("noextension", None),
        ("weird.qqq", None),
    ];
    for (name, want) in cases {
        assert_eq!(file_type(name), want, "{name}");
    }
}

#[test]
fn f4_a_large_unmatched_file_is_explained_by_its_type_and_place() {
    let fs = FakeFs::new();
    fs.add_dir(r"D:\data");
    fs.add_file_with(
        r"D:\data\movie.mkv",
        FileOpts {
            logical: Some(12 << 30),
            modified: unix_to_filetime(NOW - 3 * 365 * 86_400),
            ..FileOpts::default()
        },
    );
    fs.add_file(r"D:\data\small.txt", 10);
    let ctx = ctx(&[]);
    let (t, _) = scan(&fs, r"D:\data", &ctx, &ScanControl::default()).unwrap();
    let d = describe(&t, t.find(r"D:\data\movie.mkv").unwrap(), &ctx);
    assert!(d.large);
    assert_eq!(d.rule_id, None);
    assert_eq!(d.tier, Some(Tier::Review));
    assert_eq!(d.actions, vec![Action::ShowInExplorer, Action::Quarantine]);
    assert_eq!(d.explain.file_type, Some(FileType::Video));
    assert_eq!(d.explain.place, Place::Drive);
    let small = describe(&t, t.find(r"D:\data\small.txt").unwrap(), &ctx);
    assert!(!small.large);
    assert_eq!(small.tier, None);
    assert_eq!(small.explain.file_type, Some(FileType::Text));
}

#[test]
fn f4_a_folder_is_explained_by_what_it_mostly_holds() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\stuff\Trip").unwrap();
    for i in 0..9 {
        fs.add_file(&format!(r"C:\Users\u\stuff\Trip\IMG_{i}.jpg"), 4 << 20);
    }
    fs.add_file(r"C:\Users\u\stuff\Trip\route.gpx", 1 << 20);
    fs.create_dir_all(r"C:\Users\u\AppData\Local\Vendor\Data")
        .unwrap();
    fs.add_file(r"C:\Users\u\AppData\Local\Vendor\Data\state.bin", 1 << 20);
    let ctx = ctx(&[]);
    let (t, _) = scan(&fs, r"C:\Users\u", &ctx, &ScanControl::default()).unwrap();
    let trip = describe(&t, t.find(r"C:\Users\u\stuff\Trip").unwrap(), &ctx);
    assert_eq!(trip.explain.place, Place::Profile);
    assert_eq!(trip.explain.file_type, None);
    let mix = trip.explain.mix.expect("mostly photos");
    assert_eq!(mix.category, Category::Media);
    assert!(mix.percent >= 90, "{}", mix.percent);
    let vendor = describe(
        &t,
        t.find(r"C:\Users\u\AppData\Local\Vendor").unwrap(),
        &ctx,
    );
    assert_eq!(vendor.explain.place, Place::AppData);
    assert_eq!(vendor.explain.owner.as_deref(), Some("Vendor"));
    assert!(
        vendor.explain.owner_root,
        "the folder is the app's own folder"
    );
    let data = describe(
        &t,
        t.find(r"C:\Users\u\AppData\Local\Vendor\Data").unwrap(),
        &ctx,
    );
    assert!(!data.explain.owner_root);
    // A folder of mixed content names no single kind.
    assert_eq!(vendor.explain.mix, None);
}

const DEFINITIONS: &str = r#"
[[rule]]
id = "def-desktop"
define = true
parent = "%USERPROFILE%"
name = "Desktop"
kind = "dir"
category = "documents"

[[rule]]
id = "def-roaming-vendor"
define = true
parent = "%APPDATA%"
name = "Vendor"
kind = "dir"
category = "apps"
tier = "keep"

[[rule]]
id = "desktop-old-installers"
parent = "%USERPROFILE%\\Desktop"
name = "*.exe"
kind = "file"
category = "downloads"
tier = "likelySafe"
action = "quarantine"
"#;

#[test]
fn f4_a_definition_explains_a_folder_without_suggesting_it() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\Desktop").unwrap();
    fs.add_file(r"C:\Users\u\Desktop\setup.exe", 8 << 20);
    fs.add_file(r"C:\Users\u\Desktop\big.bin", 40 << 20);
    fs.create_dir_all(r"C:\Users\u\AppData\Roaming\Vendor")
        .unwrap();
    fs.add_file(r"C:\Users\u\AppData\Roaming\Vendor\settings.dat", 30 << 20);
    let ctx = ctx(&[DEFINITIONS]);
    let (t, _) = scan(&fs, r"C:\Users\u", &ctx, &ScanControl::default()).unwrap();

    let desktop = describe(&t, t.find(r"C:\Users\u\Desktop").unwrap(), &ctx);
    assert_eq!(desktop.rule_id.as_deref(), Some("def-desktop"));
    assert!(desktop.definition);
    assert_eq!(desktop.tier, None, "a definition has no opinion by itself");
    assert_eq!(
        desktop.actions,
        vec![Action::ShowInExplorer, Action::Quarantine]
    );

    let vendor = describe(
        &t,
        t.find(r"C:\Users\u\AppData\Roaming\Vendor").unwrap(),
        &ctx,
    );
    assert_eq!(vendor.tier, Some(Tier::Keep));
    assert_eq!(
        vendor.actions,
        vec![Action::ExplainOnly, Action::ShowInExplorer],
        "a folder to keep is never offered for a move"
    );

    let listed: Vec<String> = suggestions(&t, &ctx, 100)
        .into_iter()
        .map(|s| s.path)
        .collect();
    assert_eq!(listed, vec![r"C:\Users\u\Desktop\setup.exe".to_string()]);
}

#[test]
fn f4_a_full_rule_wins_over_a_definition_for_the_same_item() {
    let both = r#"
[[rule]]
id = "def-cache"
define = true
parent = "%LOCALAPPDATA%\\Vendor"
name = "Cache"
kind = "dir"
category = "other"

[[rule]]
id = "vendor-cache"
parent = "%LOCALAPPDATA%\\Vendor"
name = "Cache"
kind = "dir"
category = "other"
tier = "safe"
action = "quarantine"
"#;
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\Users\u\AppData\Local\Vendor\Cache")
        .unwrap();
    fs.add_file(r"C:\Users\u\AppData\Local\Vendor\Cache\a", 2 << 20);
    let ctx = ctx(&[both]);
    let (t, _) = scan(&fs, r"C:\Users\u", &ctx, &ScanControl::default()).unwrap();
    let d = describe(
        &t,
        t.find(r"C:\Users\u\AppData\Local\Vendor\Cache").unwrap(),
        &ctx,
    );
    assert_eq!(d.rule_id.as_deref(), Some("vendor-cache"));
    assert_eq!(d.tier, Some(Tier::Safe));
}

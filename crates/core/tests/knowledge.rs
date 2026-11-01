//! F2 and F4 knowledge beyond the file tree: app adapters (Steam, Ollama, Docker), the
//! Windows program history, installed programs, last use, effort and the priority matrix
//! (AC2.10, AC4.11 to AC4.13), and items read with admin rights (AC3.7).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use std::collections::BTreeMap;

use unhoard_core::adapters::{docker, ollama, steam, userassist};
use unhoard_core::classify::{Action, describe};
use unhoard_core::fake::{FakeFs, FileOpts};
use unhoard_core::fs::FileSystem;
use unhoard_core::knowledge::{Activity, Knowledge, Program, SteamInfo, SteamLibrary, UseSource};
use unhoard_core::roots::Roots;
use unhoard_core::rules::{Rules, Tier};
use unhoard_core::scan::{ScanContext, ScanControl, scan};
use unhoard_core::time::unix_to_filetime;
use unhoard_core::tree::{FLAG_ELEVATED, Tree};
use unhoard_core::view::{matrix, suggestions};

const DAY: i64 = 86_400;
const NOW: i64 = 1_790_000_000;
const MIB: u64 = 1 << 20;

const RULES: &str = r#"
[[rule]]
id = "steam-games"
parent = "**\\steamapps"
name = "common"
kind = "dir"
category = "apps"
tier = "review"
action = "uninstaller"

[[rule]]
id = "npm-cache"
parent = "**"
name = "npm-cache"
kind = "dir"
category = "developer"
tier = "safe"
action = "quarantine"

[[rule]]
id = "pagefile"
parent = "**"
name = "pagefile.sys"
kind = "file"
category = "system"
tier = "keep"
action = "explainOnly"
"#;

fn ctx(knowledge: Knowledge) -> ScanContext {
    let env: BTreeMap<String, String> = BTreeMap::new();
    let roots = Roots::new(env, None, vec![]);
    ScanContext {
        rules: Rules::from_toml(&[RULES], &roots).unwrap(),
        roots,
        now: NOW,
        knowledge,
    }
}

fn file(fs: &FakeFs, path: &str, bytes: u64, age_days: i64) {
    fs.add_file_with(
        path,
        FileOpts {
            logical: Some(bytes),
            modified: unix_to_filetime(NOW - age_days * DAY),
            ..FileOpts::default()
        },
    );
}

fn steam_knowledge() -> Knowledge {
    let manifest = r#""AppState"
{
	"appid"		"1449850"
	"name"		"Card Game"
	"installdir"		"Card Game"
	"SizeOnDisk"		"3000000"
	"LastPlayed"		"1785000000"
}"#;
    let game = steam::parse_appmanifest(manifest, r"D:\SteamLibrary").unwrap();
    Knowledge {
        steam: vec![SteamLibrary {
            common: r"D:\SteamLibrary\steamapps\common".into(),
            games: vec![game],
        }],
        ..Knowledge::default()
    }
}

fn steam_tree(fs: &FakeFs, ctx: &ScanContext) -> Tree {
    fs.create_dir_all(r"D:\SteamLibrary\steamapps\common\Card Game")
        .unwrap();
    fs.create_dir_all(r"D:\SteamLibrary\steamapps\common\Old Game")
        .unwrap();
    file(
        fs,
        r"D:\SteamLibrary\steamapps\common\Card Game\game.pak",
        3 * MIB,
        10,
    );
    file(
        fs,
        r"D:\SteamLibrary\steamapps\common\Old Game\data.pak",
        2 * MIB,
        900,
    );
    scan(fs, r"D:\SteamLibrary", ctx, &ScanControl::default())
        .unwrap()
        .0
}

#[test]
fn f4_steam_library_folders_and_app_manifests_parse() {
    let vdf = r#""libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "1000" }
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;
    assert_eq!(
        steam::library_paths(vdf),
        [r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]
    );
    let k = steam_knowledge();
    let game = &k.steam[0].games[0];
    assert_eq!(game.app_id, 1_449_850);
    assert_eq!(game.name, "Card Game");
    assert_eq!(
        game.install_dir,
        r"D:\SteamLibrary\steamapps\common\Card Game"
    );
    assert_eq!(game.last_played, Some(1_785_000_000));
    assert!(steam::parse_appmanifest("not a manifest", r"D:\x").is_none());
}

#[test]
fn f4_steam_games_and_leftovers_replace_the_common_folder_in_suggestions() {
    let fs = FakeFs::new();
    let ctx = ctx(steam_knowledge());
    let t = steam_tree(&fs, &ctx);
    let s = suggestions(&t, &ctx, 100);
    let paths: Vec<&str> = s.iter().map(|x| x.path.as_str()).collect();
    assert!(paths.contains(&r"D:\SteamLibrary\steamapps\common\Card Game"));
    assert!(paths.contains(&r"D:\SteamLibrary\steamapps\common\Old Game"));
    assert!(!paths.contains(&r"D:\SteamLibrary\steamapps\common"));

    let game = s.iter().find(|x| x.path.ends_with("Card Game")).unwrap();
    assert_eq!(game.tier, Tier::Review);
    assert!(game.description.actions.contains(&Action::AppUninstall));
    assert!(!game.description.actions.contains(&Action::Quarantine));
    assert!(matches!(
        game.description.steam,
        Some(SteamInfo::Game {
            app_id: 1_449_850,
            ..
        })
    ));
    assert_eq!(
        game.description.last_use.map(|u| (u.at, u.source)),
        Some((1_785_000_000, UseSource::Played))
    );

    let left = s.iter().find(|x| x.path.ends_with("Old Game")).unwrap();
    assert_eq!(left.tier, Tier::LikelySafe);
    assert_eq!(left.description.steam, Some(SteamInfo::Leftover));
    assert!(left.description.actions.contains(&Action::Quarantine));
}

#[test]
fn f4_ollama_models_come_from_manifests_with_shared_blobs() {
    let digest = |c: char| format!("sha256:{}", c.to_string().repeat(64));
    let manifest = |layers: &[(char, u64)]| {
        let l: Vec<String> = layers
            .iter()
            .map(|(c, s)| {
                format!(
                    r#"{{"mediaType":"x","digest":"{}","size":{s}}}"#,
                    digest(*c)
                )
            })
            .collect();
        format!(
            r#"{{"schemaVersion":2,"config":{{"digest":"{}","size":10}},"layers":[{}]}}"#,
            digest('c'),
            l.join(",")
        )
    };
    let dir = r"C:\Users\u\.ollama\models";
    let files = [
        ollama::ManifestFile {
            relative: "registry.ollama.ai/library/llama3/latest".into(),
            modified: NOW - 40 * DAY,
            json: manifest(&[('a', 4000), ('b', 100)]),
        },
        ollama::ManifestFile {
            relative: "registry.ollama.ai/someone/coder/7b".into(),
            modified: NOW - 2 * DAY,
            json: manifest(&[('a', 4000), ('d', 900)]),
        },
        ollama::ManifestFile {
            relative: "registry.ollama.ai/library/bad/latest".into(),
            modified: 0,
            json: r#"{"layers":[{"digest":"sha256:..\\..\\x","size":1}]}"#.into(),
        },
    ];
    let models = ollama::models(dir, &files);
    assert_eq!(
        models.len(),
        2,
        "a manifest with an invalid digest is skipped"
    );
    let llama = models.iter().find(|m| m.name == "llama3:latest").unwrap();
    assert_eq!(llama.bytes, 4000 + 100 + 10);
    assert_eq!(
        llama.unique_bytes, 100,
        "the shared layer and config are not unique"
    );
    assert_eq!(
        llama.manifest,
        r"C:\Users\u\.ollama\models\manifests\registry.ollama.ai\library\llama3\latest"
    );
    assert!(
        llama
            .unique_blobs
            .contains(&format!(r"{dir}\blobs\sha256-{}", "b".repeat(64)))
    );
    assert!(models.iter().any(|m| m.name == "someone/coder:7b"));
}

#[test]
fn f4_docker_system_df_output_parses_to_bytes() {
    let out = concat!(
        r#"{"Active":"2","Reclaimable":"1.2GB (52%)","Size":"2.3GB","TotalCount":"5","Type":"Images"}"#,
        "\n",
        r#"{"Active":"0","Reclaimable":"496.3kB (100%)","Size":"496.3kB","TotalCount":"3","Type":"Containers"}"#,
        "\n",
        "garbage line\n",
        r#"{"Active":"1","Reclaimable":"0B (0%)","Size":"0B","TotalCount":"1","Type":"Local Volumes"}"#,
    );
    let rows = docker::parse_system_df(out);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind, "Images");
    assert_eq!(rows[0].total, 5);
    assert_eq!(rows[0].bytes, 2_300_000_000);
    assert_eq!(rows[0].reclaimable, 1_200_000_000);
    assert_eq!(rows[1].bytes, 496_300);
    assert_eq!(docker::parse_size("12.5MB"), Some(12_500_000));
    assert_eq!(docker::parse_size("huge"), None);
}

#[test]
fn f2_program_history_names_and_times_decode() {
    assert_eq!(userassist::rot13("Pbqr.rkr"), "Code.exe");
    let mut known = BTreeMap::new();
    known.insert(
        "{6D809377-6AF0-444B-8957-A3773F02200E}".to_string(),
        r"C:\Program Files".to_string(),
    );
    let name = userassist::rot13(r"{6Q809377-6NS0-444O-8957-N3773S02200R}\Ncc\ncc.rkr");
    assert_eq!(
        userassist::resolve(&name, &known).as_deref(),
        Some(r"C:\Program Files\App\app.exe")
    );
    assert_eq!(userassist::resolve(r"C:\x\shortcut.lnk", &known), None);
    let mut data = vec![0u8; 72];
    let ft = unix_to_filetime(NOW - DAY);
    data[60..68].copy_from_slice(&ft.to_le_bytes());
    assert_eq!(userassist::last_run(&data), Some(NOW - DAY));
    assert_eq!(userassist::last_run(&[0u8; 16]), None);
    assert_eq!(userassist::last_run(&[0u8; 72]), None);
}

#[test]
fn f4_installed_program_folder_names_the_program_and_its_uninstall() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"D:\Apps\Editor").unwrap();
    file(&fs, r"D:\Apps\Editor\editor.exe", 300 * MIB, 5);
    let ctx = ctx(Knowledge {
        programs: vec![Program {
            name: "Editor".into(),
            publisher: Some("Example Ltd".into()),
            install_location: r"d:\apps\editor\".into(),
            install_date: Some("20240115".into()),
            estimated_bytes: Some(300 * MIB),
        }],
        activity: vec![Activity {
            path: r"D:\Apps\Editor\editor.exe".into(),
            at: NOW - 3 * DAY,
            source: UseSource::Started,
        }],
        ..Knowledge::default()
    });
    let (t, _) = scan(&fs, r"D:\Apps", &ctx, &ScanControl::default()).unwrap();
    let id = t.find(r"D:\Apps\Editor").unwrap();
    let d = describe(&t, id, &ctx);
    assert_eq!(d.program.as_ref().map(|p| p.name.as_str()), Some("Editor"));
    assert_eq!(d.tier, Some(Tier::Review));
    assert_eq!(d.actions.first(), Some(&Action::Uninstaller));
    assert_eq!(
        d.last_use.map(|u| (u.at, u.source)),
        Some((NOW - 3 * DAY, UseSource::Started))
    );
    assert_eq!(d.effort, Some(3));
}

#[test]
fn f2_last_use_falls_back_to_the_newest_change_inside() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\data\old").unwrap();
    file(&fs, r"C:\data\old\a.bin", MIB, 400);
    file(&fs, r"C:\data\old\b.bin", MIB, 30);
    let ctx = ctx(Knowledge::default());
    let (t, _) = scan(&fs, r"C:\data", &ctx, &ScanControl::default()).unwrap();
    let id = t.find(r"C:\data\old").unwrap();
    let d = describe(&t, id, &ctx);
    let u = d.last_use.unwrap();
    assert_eq!(u.source, UseSource::Changed);
    assert!((u.at - (NOW - 30 * DAY)).abs() <= 1, "{}", u.at);
}

#[test]
fn f2_matrix_plots_suggestions_with_effort_and_leaves_out_keep_items() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\x\npm-cache").unwrap();
    file(&fs, r"C:\x\npm-cache\blob", 50 * MIB, 3);
    file(&fs, r"C:\x\pagefile.sys", 900 * MIB, 1);
    file(&fs, r"C:\x\movie.mkv", 2 << 30, 700);
    let ctx = ctx(Knowledge::default());
    let (t, _) = scan(&fs, r"C:\x", &ctx, &ScanControl::default()).unwrap();
    let points = matrix(&t, &ctx, 100);
    let by_name = |n: &str| points.iter().find(|p| p.name == n);
    assert_eq!(
        by_name("npm-cache").map(|p| p.effort),
        Some(1),
        "safe, one click"
    );
    assert_eq!(
        by_name("movie.mkv").map(|p| p.effort),
        Some(2),
        "review first"
    );
    assert!(
        by_name("pagefile.sys").is_none(),
        "keep items are not plotted"
    );
    assert!(points.windows(2).all(|w| w[0].on_disk >= w[1].on_disk));
}

#[test]
fn f3_revealed_folders_are_grouped_by_kind() {
    use unhoard_core::hidden::{HiddenKind, hidden_kind};
    let env: BTreeMap<String, String> = [
        ("USERPROFILE", r"C:\Users\me"),
        ("PROGRAMFILES", r"C:\Program Files"),
        ("PROGRAMDATA", r"C:\ProgramData"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    let roots = Roots::new(env, None, vec![]);
    for (path, kind) in [
        (r"C:\System Volume Information", HiddenKind::RestorePoints),
        (r"C:\Program Files\WindowsApps", HiddenKind::StoreApps),
        (r"C:\Recovery", HiddenKind::Recovery),
        (r"C:\$WinREAgent", HiddenKind::Recovery),
        (
            r"C:\ProgramData\Microsoft\Windows Defender\Scans",
            HiddenKind::Defender,
        ),
        (
            r"C:\ProgramData\Microsoft\Windows\WER\ReportQueue\x",
            HiddenKind::ErrorReports,
        ),
        (r"C:\Users\other", HiddenKind::OtherAccounts),
        (r"C:\$Recycle.Bin\S-1-5-18", HiddenKind::RecycleBin),
        (r"C:\temp\vendor", HiddenKind::Other),
        (r"C:\Users\me\AppData\x", HiddenKind::Other),
    ] {
        assert_eq!(hidden_kind(path, &roots), kind, "{path}");
    }
}

#[test]
fn f3_s7_folders_read_with_admin_fill_the_tree_and_never_offer_a_move() {
    let fs = FakeFs::new();
    fs.create_dir_all(r"C:\data\locked").unwrap();
    file(&fs, r"C:\data\a.bin", MIB, 1);
    fs.deny(r"C:\data\locked");
    let ctx = ctx(Knowledge::default());
    let (mut t, _) = scan(&fs, r"C:\data", &ctx, &ScanControl::default()).unwrap();
    let id = t.find(r"C:\data\locked").unwrap();
    let before = t.get(t.root()).unwrap().on_disk;
    assert!(t.set_measured(id, 7 * MIB, 8 * MIB, 12));
    let n = t.get(id).unwrap();
    assert_eq!(n.on_disk, 8 * MIB);
    assert!(n.flags & FLAG_ELEVATED != 0);
    assert_eq!(t.get(t.root()).unwrap().on_disk, before + 8 * MIB);
    assert!(
        !t.set_measured(id, 1, 1, 1),
        "an item is measured only once"
    );
    let d = describe(&t, id, &ctx);
    assert!(d.elevated);
    assert!(!d.actions.contains(&Action::Quarantine));
}

//! F16 Optimise: each finding's state follows from its measurement, startup entries keep
//! their exact old value for undo, and a broken startup entry is flagged (T-16.1, T-16.3).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use unhoard_core::optimise::{
    Disk, Group, Memory, OptimiseData, Partition, PartitionKind, ProcessUse, StartupEntry,
    StartupScope, State, Updates, approved, disabled_value, enabled_value, findings,
    partition_kind,
};
use unhoard_core::processes;

const GB: u64 = 1 << 30;

fn data() -> OptimiseData {
    OptimiseData {
        memory: Some(Memory {
            load_percent: 50,
            total: 32 * GB,
            available: 16 * GB,
            commit: 20 * GB,
            commit_limit: 40 * GB,
        }),
        processes: vec![],
        disks: vec![Disk {
            name: "SSD".into(),
            media: "SSD".into(),
            health: "Healthy".into(),
            size: Some(953 * GB),
            partitions: vec![
                Partition {
                    number: 1,
                    kind: PartitionKind::Efi,
                    size: GB / 4,
                    ..Partition::default()
                },
                Partition {
                    number: 3,
                    letter: Some("C".into()),
                    kind: PartitionKind::Data,
                    size: 950 * GB,
                    file_system: Some("NTFS".into()),
                    label: Some("Windows".into()),
                    free: Some(120 * GB),
                },
            ],
        }],
        system_free: Some(120 * GB),
        system_total: Some(950 * GB),
        startup: vec![],
        updates: Some(Updates {
            pending: 0,
            failed_last_30_days: 0,
        }),
        page_file_on: Some(true),
        power_mode: Some("balanced".into()),
    }
}

fn state(d: &OptimiseData, id: &str) -> State {
    findings(d)
        .into_iter()
        .find(|f| f.id == id)
        .unwrap_or_else(|| panic!("no finding {id}"))
        .state
}

#[test]
fn f16_states_follow_the_measurements() {
    let ok = data();
    for id in [
        "memory",
        "disk:SSD",
        "freeSpace",
        "updates",
        "pageFile",
        "power",
    ] {
        assert_eq!(state(&ok, id), State::Ok, "{id}");
    }
    let mut d = data();
    d.memory.as_mut().unwrap().load_percent = 92;
    assert_eq!(state(&d, "memory"), State::Look);
    d.disks[0].health = "Warning".into();
    assert_eq!(state(&d, "disk:SSD"), State::Look);
    d.system_free = Some(15 * GB);
    assert_eq!(state(&d, "freeSpace"), State::Look);
    d.system_free = Some(5 * GB);
    assert_eq!(state(&d, "freeSpace"), State::Action);
    d.updates = Some(Updates {
        pending: 3,
        failed_last_30_days: 0,
    });
    assert_eq!(state(&d, "updates"), State::Look);
    d.page_file_on = Some(false);
    assert_eq!(state(&d, "pageFile"), State::Look);
    // Every finding sits in one of the six groups and names its source.
    for f in findings(&d) {
        assert!(!f.source.is_empty(), "{}", f.id);
        assert!(matches!(
            f.group,
            Group::Performance
                | Group::Storage
                | Group::Startup
                | Group::Space
                | Group::Health
                | Group::Config
        ));
    }
}

#[test]
fn f16_unknown_measurements_are_left_out_never_shown_as_zero() {
    let mut d = data();
    d.memory = None;
    d.updates = None;
    d.system_free = None;
    let ids: Vec<String> = findings(&d).into_iter().map(|f| f.id).collect();
    for gone in ["memory", "updates", "freeSpace"] {
        assert!(!ids.iter().any(|i| i == gone), "{gone}");
    }
}

#[test]
fn f16_startup_entries_read_their_approved_state_and_a_missing_file_is_flagged() {
    assert!(approved(None));
    assert!(approved(Some(&enabled_value())));
    let off = disabled_value(133_000_000_000_000_000);
    assert!(!approved(Some(&off)));
    assert_eq!(off[0], 3);
    assert_eq!(&off[4..12], &133_000_000_000_000_000u64.to_le_bytes());
    // Windows also writes 6 and 7 for some entries.
    assert!(approved(Some(&[6, 0, 0, 0])));
    assert!(!approved(Some(&[7, 0, 0, 0])));

    let mut d = data();
    d.startup = vec![
        StartupEntry {
            name: "Chat".into(),
            command: r#""C:\Apps\chat.exe" --tray"#.into(),
            file: Some(r"C:\Apps\chat.exe".into()),
            file_exists: true,
            enabled: true,
            scope: StartupScope::UserRun,
            publisher: None,
        },
        StartupEntry {
            name: "Gone".into(),
            command: r"C:\Old\gone.exe".into(),
            file: Some(r"C:\Old\gone.exe".into()),
            file_exists: false,
            enabled: true,
            scope: StartupScope::MachineRun,
            publisher: None,
        },
    ];
    let f = findings(&d);
    let chat = f.iter().find(|x| x.id == "startup:UserRun:Chat").unwrap();
    assert_eq!(
        chat.state,
        State::Action,
        "a user entry can be switched off"
    );
    let gone = f
        .iter()
        .find(|x| x.id == "startup:MachineRun:Gone")
        .unwrap();
    assert_eq!(
        gone.state,
        State::Look,
        "flagged, never deleted, no toggle for machine entries"
    );
    assert!(
        gone.values
            .iter()
            .any(|(k, v)| k == "missing" && v == "true")
    );
}

/// The value `key` of the finding `id`.
fn value(d: &OptimiseData, id: &str, key: &str) -> Option<String> {
    findings(d)
        .into_iter()
        .find(|f| f.id == id)?
        .values
        .into_iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v)
}

#[test]
fn f16_partitions_are_listed_under_their_disk() {
    let d = data();
    let ids: Vec<String> = findings(&d).into_iter().map(|f| f.id).collect();
    let at = |id: &str| {
        ids.iter()
            .position(|i| i == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    assert!(at("disk:SSD") < at("partition:SSD:1"));
    assert!(at("partition:SSD:1") < at("partition:SSD:3"));
    assert_eq!(
        value(&d, "disk:SSD", "size").as_deref(),
        Some("1023275958272")
    );
    assert_eq!(value(&d, "partition:SSD:3", "letter").as_deref(), Some("C"));
    assert_eq!(
        value(&d, "partition:SSD:3", "kind").as_deref(),
        Some("data")
    );
    assert_eq!(
        value(&d, "partition:SSD:3", "fileSystem").as_deref(),
        Some("NTFS")
    );
    assert_eq!(value(&d, "partition:SSD:1", "kind").as_deref(), Some("efi"));
    // What Windows did not report is left out, never shown as 0 or empty.
    assert_eq!(value(&d, "partition:SSD:1", "free"), None);
    assert_eq!(value(&d, "partition:SSD:1", "letter"), None);
    assert_eq!(state(&d, "partition:SSD:1"), State::Ok);
}

#[test]
fn f16_partition_kinds_follow_the_gpt_type() {
    let kind = |g: &str| partition_kind(Some(g), None);
    assert_eq!(
        kind("{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}"),
        PartitionKind::Efi
    );
    assert_eq!(
        kind("{E3C9E316-0B5C-4DB8-817D-F92DF00215AE}"),
        PartitionKind::Reserved
    );
    assert_eq!(
        kind("{de94bba4-06d1-4d40-a16a-bfd50179d6ac}"),
        PartitionKind::Recovery
    );
    assert_eq!(
        kind("{ebd0a0a2-b9e5-4433-87c0-68b6b72699c7}"),
        PartitionKind::Data
    );
    assert_eq!(
        kind("{00000000-0000-0000-0000-000000000000}"),
        PartitionKind::Other
    );
    // MBR disks: 0x07 is NTFS data, 0x27 the recovery partition, 0xEF the EFI partition.
    assert_eq!(partition_kind(None, Some(0x07)), PartitionKind::Data);
    assert_eq!(partition_kind(None, Some(0x27)), PartitionKind::Recovery);
    assert_eq!(partition_kind(None, Some(0xEF)), PartitionKind::Efi);
    assert_eq!(partition_kind(None, None), PartitionKind::Other);
}

#[test]
fn f16_known_processes_are_found_by_executable_name() {
    let id = |exe: &str| processes::known(exe).map(|p| p.id.as_str());
    assert_eq!(id("chrome.exe"), Some("chrome"));
    assert_eq!(id("CHROME.EXE"), Some("chrome"));
    assert_eq!(
        id(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
        Some("chrome")
    );
    assert_eq!(id("Memory Compression"), Some("memoryCompression"));
    assert_eq!(id("something-nobody-knows.exe"), None);
    assert_eq!(id(""), None);
}

#[test]
fn f16_every_known_process_has_english_and_dutch_text() {
    let en: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/i18n/en.json")).unwrap();
    let nl: serde_json::Value =
        serde_json::from_str(include_str!("../../../src/i18n/nl.json")).unwrap();
    assert!(processes::all().len() >= 100);
    let mut exes = std::collections::BTreeSet::new();
    for p in processes::all() {
        for (lang, texts) in [("en", &en), ("nl", &nl)] {
            for key in ["name", "what"] {
                let text = texts["processes"][&p.id][key].as_str().unwrap_or("");
                assert!(!text.is_empty(), "{lang}: processes.{}.{key}", p.id);
            }
        }
        assert!(!p.exe.is_empty(), "{}", p.id);
        for e in &p.exe {
            assert_eq!(*e, e.to_lowercase(), "{e}");
            assert!(exes.insert(e.clone()), "{e} is listed twice");
        }
    }
    // No text without a process.
    for texts in [&en, &nl] {
        for id in texts["processes"].as_object().unwrap().keys() {
            assert!(processes::all().iter().any(|p| &p.id == id), "{id}");
        }
    }
}

#[test]
fn f16_process_and_startup_findings_name_the_known_program() {
    let mut d = data();
    d.processes = vec![
        ProcessUse {
            name: "chrome.exe".into(),
            cpu_percent: 12.0,
            memory: GB,
        },
        ProcessUse {
            name: "mystery.exe".into(),
            cpu_percent: 1.0,
            memory: GB,
        },
    ];
    d.startup = vec![StartupEntry {
        name: "Sync".into(),
        command: r#""C:\Users\u\AppData\Local\Microsoft\OneDrive\OneDrive.exe" /background"#.into(),
        file: Some(r"C:\Users\u\AppData\Local\Microsoft\OneDrive\OneDrive.exe".into()),
        file_exists: true,
        enabled: true,
        scope: StartupScope::UserRun,
        publisher: None,
    }];
    assert_eq!(value(&d, "process:0", "known").as_deref(), Some("chrome"));
    assert_eq!(value(&d, "process:1", "known"), None);
    assert_eq!(
        value(&d, "startup:UserRun:Sync", "known").as_deref(),
        Some("onedrive")
    );
}

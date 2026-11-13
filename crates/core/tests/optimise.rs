//! F16 Optimise: each finding's state follows from its measurement, startup entries keep
//! their exact old value for undo, and a broken startup entry is flagged (T-16.1, T-16.3).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "test helpers fail loudly on purpose"
)]

use unhoard_core::optimise::{
    Disk, Group, Memory, OptimiseData, StartupEntry, StartupScope, State, Updates, approved,
    disabled_value, enabled_value, findings,
};

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

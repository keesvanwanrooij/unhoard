//! Optimise (F16): the honest check-up. The platform layer measures; this module turns the
//! measurements into findings with a state, the values that explain them and their source.
//!
//! Every finding only reads, except the startup toggle, whose registry values are encoded
//! here. A measurement Windows did not give is left out, never shown as 0 (rule 4).

use serde::{Deserialize, Serialize};

/// The six groups of the check-up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    /// CPU, memory and the page file.
    Performance,
    /// Disk health.
    Storage,
    /// Startup apps.
    Startup,
    /// Free space.
    Space,
    /// Windows Update and system files.
    Health,
    /// Power mode and the page file setting.
    Config,
}

/// What a finding asks of the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// In order.
    Ok,
    /// Worth a look.
    Look,
    /// An action is possible.
    Action,
}

/// One finding: what was measured, from where, and what it means.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// A stable ID; it also picks the finding's text.
    pub id: String,
    /// Its group.
    pub group: Group,
    /// Its state.
    pub state: State,
    /// The measured values, by name, as text for the finding's sentence.
    pub values: Vec<(String, String)>,
    /// Where the numbers come from (a text key).
    pub source: String,
}

/// Memory as Windows reports it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Memory {
    /// Memory in use, in percent.
    pub load_percent: u32,
    /// Physical memory.
    pub total: u64,
    /// Available physical memory.
    pub available: u64,
    /// Committed memory (RAM plus page file in use).
    pub commit: u64,
    /// The commit limit.
    pub commit_limit: u64,
}

/// A running process and what it uses.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessUse {
    /// Its executable's name.
    pub name: String,
    /// CPU over the sampled second, in percent of the whole machine.
    pub cpu_percent: f64,
    /// Working set.
    pub memory: u64,
}

/// A physical disk.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Disk {
    /// Its name.
    pub name: String,
    /// `SSD`, `HDD` or `Unspecified`.
    pub media: String,
    /// `Healthy`, `Warning`, `Unhealthy` or `Unknown`, as Windows reports it.
    pub health: String,
    /// Its capacity, when Windows reported it.
    pub size: Option<u64>,
    /// Its partitions, in disk order.
    pub partitions: Vec<Partition>,
}

/// What a partition is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PartitionKind {
    /// The EFI system partition: the boot loader.
    Efi,
    /// The Microsoft reserved partition: empty space Windows keeps for itself.
    Reserved,
    /// The recovery partition: the Windows Recovery Environment.
    Recovery,
    /// A partition with your files, usually with a drive letter.
    Data,
    /// Anything else.
    #[default]
    Other,
}

/// A partition of a disk.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Partition {
    /// Its number on the disk.
    pub number: u32,
    /// Its drive letter, without the colon.
    pub letter: Option<String>,
    /// What it is for.
    pub kind: PartitionKind,
    /// Its size.
    pub size: u64,
    /// The file system of its volume, for example `NTFS` or `FAT32`.
    pub file_system: Option<String>,
    /// The volume's label.
    pub label: Option<String>,
    /// Free space on its volume.
    pub free: Option<u64>,
}

/// The kind of a partition from its GPT type GUID or, on an MBR disk, its type byte.
pub fn partition_kind(gpt_type: Option<&str>, mbr_type: Option<u16>) -> PartitionKind {
    if let Some(g) = gpt_type {
        return match g
            .trim_matches(|c| c == '{' || c == '}')
            .to_ascii_lowercase()
            .as_str()
        {
            "c12a7328-f81f-11d2-ba4b-00a0c93ec93b" => PartitionKind::Efi,
            "e3c9e316-0b5c-4db8-817d-f92df00215ae" => PartitionKind::Reserved,
            "de94bba4-06d1-4d40-a16a-bfd50179d6ac" => PartitionKind::Recovery,
            "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7" => PartitionKind::Data,
            _ => PartitionKind::Other,
        };
    }
    match mbr_type {
        Some(0x01 | 0x04 | 0x06 | 0x07 | 0x0B | 0x0C | 0x0E) => PartitionKind::Data,
        Some(0x27) => PartitionKind::Recovery,
        Some(0xEF) => PartitionKind::Efi,
        _ => PartitionKind::Other,
    }
}

/// Where a startup entry lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartupScope {
    /// The user's `Run` key; the user may switch it.
    UserRun,
    /// The machine's `Run` key; switching needs admin.
    MachineRun,
    /// The user's Startup folder; the user may switch it.
    UserFolder,
    /// The Startup folder for all users; switching needs admin.
    CommonFolder,
}

impl StartupScope {
    /// True when the current user may switch the entry without admin.
    pub fn user_may_switch(self) -> bool {
        matches!(self, StartupScope::UserRun | StartupScope::UserFolder)
    }
}

/// A program that starts when the user signs in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntry {
    /// The value or shortcut name.
    pub name: String,
    /// The command it runs.
    pub command: String,
    /// The program file the command starts, when it could be read.
    pub file: Option<String>,
    /// False when that file no longer exists.
    pub file_exists: bool,
    /// Whether Windows starts it (its `StartupApproved` state).
    pub enabled: bool,
    /// Where it lives.
    pub scope: StartupScope,
    /// The program file's company, when known.
    pub publisher: Option<String>,
}

/// Windows Update as the Update Agent reports it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Updates {
    /// Updates waiting to be installed.
    pub pending: u32,
    /// Updates that failed to install in the last 30 days.
    pub failed_last_30_days: u32,
}

/// Everything the check-up measured; `None` and empty mean "not available".
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptimiseData {
    /// Memory.
    pub memory: Option<Memory>,
    /// The top processes by CPU and memory.
    pub processes: Vec<ProcessUse>,
    /// Physical disks.
    pub disks: Vec<Disk>,
    /// Free space on the system drive.
    pub system_free: Option<u64>,
    /// Size of the system drive.
    pub system_total: Option<u64>,
    /// Startup entries.
    pub startup: Vec<StartupEntry>,
    /// Windows Update.
    pub updates: Option<Updates>,
    /// Whether a page file is configured.
    pub page_file_on: Option<bool>,
    /// `bestEfficiency`, `balanced` or `bestPerformance`.
    pub power_mode: Option<String>,
}

/// Free space below this is worth a look: big updates need room (20 GiB).
const FREE_LOOK: u64 = 20 << 30;
/// Free space below this calls for action (10 GiB).
const FREE_ACTION: u64 = 10 << 30;
/// Memory load above this is worth a look.
const MEMORY_LOOK: u32 = 85;
/// Commit above this share of the limit is worth a look.
const COMMIT_LOOK: f64 = 0.9;

fn finding(
    id: &str,
    group: Group,
    state: State,
    values: Vec<(&str, String)>,
    source: &str,
) -> Finding {
    Finding {
        id: id.to_string(),
        group,
        state,
        values: values
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        source: source.to_string(),
    }
}

/// The values with only those Windows reported; a missing one is left out, never empty.
fn some_values(values: Vec<(&str, Option<String>)>) -> Vec<(&str, String)> {
    values
        .into_iter()
        .filter_map(|(k, v)| v.map(|v| (k, v)))
        .collect()
}

/// The values plus `known`, the ID of the known program `exe` runs, when there is one.
fn known_value<'a>(exe: &str, mut values: Vec<(&'a str, String)>) -> Vec<(&'a str, String)> {
    if let Some(p) = crate::processes::known(exe) {
        values.push(("known", p.id.clone()));
    }
    values
}

/// The findings of a check-up, in group order.
pub fn findings(d: &OptimiseData) -> Vec<Finding> {
    let mut out = vec![];
    if let Some(m) = &d.memory {
        let commit = if m.commit_limit > 0 {
            m.commit as f64 / m.commit_limit as f64
        } else {
            0.0
        };
        let state = if m.load_percent > MEMORY_LOOK || commit > COMMIT_LOOK {
            State::Look
        } else {
            State::Ok
        };
        out.push(finding(
            "memory",
            Group::Performance,
            state,
            vec![
                ("load", m.load_percent.to_string()),
                ("total", m.total.to_string()),
                ("available", m.available.to_string()),
                ("commit", m.commit.to_string()),
                ("commitLimit", m.commit_limit.to_string()),
            ],
            "memoryStatus",
        ));
    }
    for (i, p) in d.processes.iter().enumerate() {
        out.push(finding(
            &format!("process:{i}"),
            Group::Performance,
            State::Ok,
            known_value(
                &p.name,
                vec![
                    ("name", p.name.clone()),
                    ("cpu", format!("{:.0}", p.cpu_percent)),
                    ("memory", p.memory.to_string()),
                ],
            ),
            "processList",
        ));
    }
    for disk in &d.disks {
        let state = if disk.health.eq_ignore_ascii_case("healthy") {
            State::Ok
        } else {
            State::Look
        };
        out.push(finding(
            &format!("disk:{}", disk.name),
            Group::Storage,
            state,
            some_values(vec![
                ("name", Some(disk.name.clone())),
                ("media", Some(disk.media.clone())),
                ("health", Some(disk.health.clone())),
                ("size", disk.size.map(|s| s.to_string())),
                ("partitions", Some(disk.partitions.len().to_string())),
            ]),
            "storageManagement",
        ));
        for part in &disk.partitions {
            let kind = serde_json::to_value(part.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| "other".into());
            out.push(finding(
                &format!("partition:{}:{}", disk.name, part.number),
                Group::Storage,
                State::Ok,
                some_values(vec![
                    ("disk", Some(disk.name.clone())),
                    ("number", Some(part.number.to_string())),
                    ("kind", Some(kind)),
                    ("size", Some(part.size.to_string())),
                    ("letter", part.letter.clone()),
                    ("fileSystem", part.file_system.clone()),
                    ("label", part.label.clone()),
                    ("free", part.free.map(|f| f.to_string())),
                ]),
                "storageManagement",
            ));
        }
    }
    for e in &d.startup {
        let state = if !e.file_exists {
            State::Look
        } else if e.scope.user_may_switch() {
            State::Action
        } else {
            State::Ok
        };
        out.push(finding(
            &format!("startup:{:?}:{}", e.scope, e.name),
            Group::Startup,
            state,
            known_value(
                e.file.as_deref().unwrap_or(&e.name),
                vec![
                    ("name", e.name.clone()),
                    ("command", e.command.clone()),
                    ("enabled", e.enabled.to_string()),
                    ("missing", (!e.file_exists).to_string()),
                    ("publisher", e.publisher.clone().unwrap_or_default()),
                ],
            ),
            "startupApproved",
        ));
    }
    if let (Some(free), Some(total)) = (d.system_free, d.system_total) {
        let state = if free < FREE_ACTION {
            State::Action
        } else if free < FREE_LOOK {
            State::Look
        } else {
            State::Ok
        };
        out.push(finding(
            "freeSpace",
            Group::Space,
            state,
            vec![("free", free.to_string()), ("total", total.to_string())],
            "volume",
        ));
    }
    if let Some(u) = &d.updates {
        let state = if u.pending > 0 || u.failed_last_30_days > 0 {
            State::Look
        } else {
            State::Ok
        };
        out.push(finding(
            "updates",
            Group::Health,
            state,
            vec![
                ("pending", u.pending.to_string()),
                ("failed", u.failed_last_30_days.to_string()),
            ],
            "windowsUpdate",
        ));
    }
    out.push(finding(
        "systemFiles",
        Group::Health,
        State::Ok,
        vec![],
        "explainOnly",
    ));
    if let Some(on) = d.page_file_on {
        out.push(finding(
            "pageFile",
            Group::Config,
            if on { State::Ok } else { State::Look },
            vec![("on", on.to_string())],
            "memoryManagement",
        ));
    }
    if let Some(mode) = &d.power_mode {
        out.push(finding(
            "power",
            Group::Config,
            State::Ok,
            vec![("mode", mode.clone())],
            "powerMode",
        ));
    }
    out
}

/// Whether a `StartupApproved` value means "enabled". A missing value is enabled; Windows
/// writes 2 or 6 in the first byte for enabled and 3 or 7 for disabled.
pub fn approved(value: Option<&[u8]>) -> bool {
    match value.and_then(|v| v.first()) {
        None => true,
        Some(b) => b & 1 == 0,
    }
}

/// The value Task Manager writes to enable an entry.
pub fn enabled_value() -> [u8; 12] {
    [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// The value Task Manager writes to disable an entry: 3, then the time as a file time.
pub fn disabled_value(filetime: u64) -> [u8; 12] {
    let mut v = [0u8; 12];
    v[0] = 3;
    v[4..12].copy_from_slice(&filetime.to_le_bytes());
    v
}

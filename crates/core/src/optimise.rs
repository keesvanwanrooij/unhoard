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
            vec![
                ("name", p.name.clone()),
                ("cpu", format!("{:.0}", p.cpu_percent)),
                ("memory", p.memory.to_string()),
            ],
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
            vec![
                ("name", disk.name.clone()),
                ("media", disk.media.clone()),
                ("health", disk.health.clone()),
            ],
            "storageManagement",
        ));
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
            vec![
                ("name", e.name.clone()),
                ("command", e.command.clone()),
                ("enabled", e.enabled.to_string()),
                ("missing", (!e.file_exists).to_string()),
                ("publisher", e.publisher.clone().unwrap_or_default()),
            ],
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

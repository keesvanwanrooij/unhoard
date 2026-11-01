//! Hidden space: what is in use on a drive but was not seen by the scan, and what kind of
//! folder a revealed one is, so the UI can group and explain it (AC3.9).

use serde::{Deserialize, Serialize};

use crate::fs::VolumeInfo;
use crate::roots::{Roots, drive_of};

/// Hidden bytes = (total − free) − scanned size on disk − parts measured another way (for
/// example the Recycle Bin). Never negative.
pub fn hidden_bytes(volume: &VolumeInfo, scanned_on_disk: u64, measured_elsewhere: u64) -> u64 {
    volume
        .total_bytes
        .saturating_sub(volume.free_bytes)
        .saturating_sub(scanned_on_disk)
        .saturating_sub(measured_elsewhere)
}

/// What a folder found by "Reveal hidden space" is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HiddenKind {
    /// System Volume Information: restore points and shadow copies.
    RestorePoints,
    /// `Program Files\WindowsApps`: apps from the Microsoft Store.
    StoreApps,
    /// The recovery environment (`Recovery`, `$WinREAgent`).
    Recovery,
    /// Windows Defender's scans, updates and quarantine.
    Defender,
    /// Windows Error Reporting's reports.
    ErrorReports,
    /// Other accounts' profiles.
    OtherAccounts,
    /// The Recycle Bin of other accounts.
    RecycleBin,
    /// Anything else.
    Other,
}

fn key(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

fn within(p: &str, dir: &str) -> bool {
    p == dir || (p.starts_with(dir) && p.as_bytes().get(dir.len()) == Some(&b'\\'))
}

/// The kind of a revealed folder.
pub fn hidden_kind(path: &str, roots: &Roots) -> HiddenKind {
    let p = key(path);
    let Some(drive) = drive_of(path).map(|d| d.to_lowercase()) else {
        return HiddenKind::Other;
    };
    let at = |rel: &str| within(&p, &format!("{drive}\\{rel}"));
    let under = |var: Option<&str>, rel: &str| {
        var.is_some_and(|v| within(&p, &key(&format!("{v}\\{rel}"))))
    };
    if at("system volume information") {
        return HiddenKind::RestorePoints;
    }
    if at("$recycle.bin") {
        return HiddenKind::RecycleBin;
    }
    if at("recovery") || at("$winreagent") {
        return HiddenKind::Recovery;
    }
    if roots
        .program_files()
        .iter()
        .any(|pf| under(Some(pf), "WindowsApps"))
    {
        return HiddenKind::StoreApps;
    }
    let pd = roots.program_data();
    if under(pd, r"Microsoft\Windows Defender") {
        return HiddenKind::Defender;
    }
    if under(pd, r"Microsoft\Windows\WER") {
        return HiddenKind::ErrorReports;
    }
    let me = roots.profile().map(key);
    let users = format!("{drive}\\users");
    if within(&p, &users) && p != users && !me.as_deref().is_some_and(|m| within(&p, m)) {
        return HiddenKind::OtherAccounts;
    }
    HiddenKind::Other
}

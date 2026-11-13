//! Protected paths: never moved, renamed or deleted, whatever a rule or the UI asks (S4).
//!
//! The check runs in the core every time an item is resolved from its ID, after the path
//! is normalised; the UI never decides it.

use serde::{Deserialize, Serialize};

use crate::roots::{Roots, drive_of, is_within, parent_of, same_path};

/// Why a path is protected.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtectReason {
    /// The Windows folder or something inside it.
    Windows,
    /// `Program Files` or `Program Files (x86)`.
    ProgramFiles,
    /// `ProgramData\Microsoft` or `ProgramData\Package Cache`.
    ProgramData,
    /// The folder that holds all user profiles.
    UsersFolder,
    /// A user profile root itself.
    ProfileRoot,
    /// A whole drive.
    DriveRoot,
    /// System Volume Information.
    SystemVolumeInformation,
    /// The Recycle Bin folder.
    RecycleBin,
    /// A quarantine folder or something already in it.
    Quarantine,
}

/// Returns why `path` is protected, or `None` if Unhoard may move it.
pub fn protection(path: &str, roots: &Roots) -> Option<ProtectReason> {
    let p = path.replace('/', "\\");
    let p = p.trim_end_matches('\\');
    let drive = drive_of(p)?;
    if p.len() <= 2 {
        return Some(ProtectReason::DriveRoot);
    }
    if roots.in_quarantine(p) {
        return Some(ProtectReason::Quarantine);
    }
    let first = p[3..].split('\\').next().unwrap_or("").to_lowercase();
    if first == "system volume information" {
        return Some(ProtectReason::SystemVolumeInformation);
    }
    if first == "$recycle.bin" {
        return Some(ProtectReason::RecycleBin);
    }
    let windows = roots
        .windows()
        .map(str::to_string)
        .unwrap_or_else(|| format!("{drive}\\Windows"));
    if is_within(p, &windows) {
        return Some(ProtectReason::Windows);
    }
    let mut program_files: Vec<String> = roots
        .program_files()
        .iter()
        .map(|s| s.to_string())
        .collect();
    if program_files.is_empty() {
        program_files = vec![
            format!("{drive}\\Program Files"),
            format!("{drive}\\Program Files (x86)"),
        ];
    }
    if program_files.iter().any(|pf| is_within(p, pf)) {
        return Some(ProtectReason::ProgramFiles);
    }
    let program_data = roots
        .program_data()
        .map(str::to_string)
        .unwrap_or_else(|| format!("{drive}\\ProgramData"));
    if ["Microsoft", "Package Cache"]
        .iter()
        .any(|s| is_within(p, &format!("{program_data}\\{s}")))
    {
        return Some(ProtectReason::ProgramData);
    }
    let users = roots.users().unwrap_or_else(|| format!("{drive}\\Users"));
    if same_path(p, &users) {
        return Some(ProtectReason::UsersFolder);
    }
    if parent_of(p).is_some_and(|parent| same_path(&parent, &users)) {
        return Some(ProtectReason::ProfileRoot);
    }
    None
}

//! What Unhoard needs to know about this PC: the well-known locations, the drives, the
//! Recycle Bin, and opening Explorer or a Windows tool on an explicit user click.

use std::collections::BTreeMap;
use std::process::Command;

use unhoard_core::roots::{QUARANTINE_DIR, Roots, join};
use unhoard_core::rules::WindowsTool;

use crate::win::{files, registry, security, shell};

/// Reads the environment, the Downloads Known Folder and the cloud sync roots.
pub fn system_roots() -> Roots {
    let env: BTreeMap<String, String> = std::env::vars().collect();
    let sync = security::current_user_sid()
        .map(|sid| registry::sync_roots(&sid))
        .unwrap_or_default();
    Roots::new(env, shell::downloads(), sync)
}

/// The fixed drives, as `C:`.
pub fn fixed_drives() -> Vec<String> {
    files::fixed_drives()
}

/// Every quarantine root that may hold batches: one per fixed drive and one per sync root.
pub fn quarantine_roots(roots: &Roots) -> Vec<String> {
    fixed_drives()
        .iter()
        .map(|d| format!("{d}\\{QUARANTINE_DIR}"))
        .chain(roots.sync_roots.iter().map(|s| join(s, QUARANTINE_DIR)))
        .collect()
}

/// The bytes in the Recycle Bin of a drive.
pub fn recycle_bin_bytes(drive: &str) -> Option<u64> {
    shell::recycle_bin_bytes(drive)
}

/// Opens Explorer with `path` selected. Only on an explicit user click.
pub fn show_in_explorer(path: &str) -> std::io::Result<()> {
    Command::new("explorer.exe")
        .arg(format!("/select,{path}"))
        .spawn()
        .map(drop)
}

/// Opens a folder in Explorer. Only on an explicit user click.
pub fn open_folder(path: &str) -> std::io::Result<()> {
    Command::new("explorer.exe").arg(path).spawn().map(drop)
}

/// Opens a Windows tool from the allowlist. Only on an explicit user click.
pub fn open_tool(tool: WindowsTool) -> std::io::Result<()> {
    let (program, arg) = match tool {
        WindowsTool::StorageSettings => ("explorer.exe", "ms-settings:storagesense"),
        WindowsTool::StorageSense => ("explorer.exe", "ms-settings:storagepolicies"),
        WindowsTool::Apps => ("explorer.exe", "ms-settings:appsfeatures"),
        WindowsTool::DiskCleanup => ("cleanmgr.exe", ""),
        WindowsTool::SystemProtection => ("SystemPropertiesProtection.exe", ""),
        WindowsTool::RecycleBin => ("explorer.exe", "shell:RecycleBinFolder"),
    };
    let mut c = Command::new(program);
    if !arg.is_empty() {
        c.arg(arg);
    }
    c.spawn().map(drop)
}

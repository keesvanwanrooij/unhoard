//! "Reveal hidden space": the elevated, read-only helper (F3, S7). Risk class High.
//!
//! The app never runs elevated. To look into folders the user may not read, it starts its
//! own executable once more through the UAC prompt with `--reveal-helper`, a pipe name and
//! the folders to list. The helper enables SeBackupPrivilege, lists those folders with
//! the same read-only scanner, sends a summary over the pipe and exits. It has no code
//! path that writes, moves or deletes. The app accepts the data only from the process it
//! started (checked by process ID) and validates every folder before the helper runs.

use std::io::Write;

use serde::{Deserialize, Serialize};
use unhoard_core::fs::FsError;
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};

use crate::WindowsFs;
use crate::win::{elevate, security};

/// The argument that makes the executable run as the helper.
pub const HELPER_ARG: &str = "--reveal-helper";
const MAX_FOLDERS: usize = 64;
const MAX_REPLY: usize = 8 << 20;
const TOP: usize = 20;

/// What the helper found in one folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealedFolder {
    /// The folder.
    pub path: String,
    /// Logical size.
    pub logical: u64,
    /// Size on disk.
    pub on_disk: u64,
    /// Number of files.
    pub files: u64,
    /// The largest children: name and size on disk.
    pub children: Vec<(String, u64)>,
    /// Why it could not be listed, even elevated.
    pub error: Option<FsError>,
}

/// Why the reveal did not happen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum RevealError {
    /// The user declined the UAC prompt.
    Declined,
    /// A folder failed validation.
    InvalidFolder(String),
    /// The helper failed.
    Failed(FsError),
}

/// A folder the helper may list: an absolute local path, no `..`, a sane length.
pub fn valid_folder(path: &str) -> bool {
    let b = path.as_bytes();
    path.len() <= 32_000
        && b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && !path.contains('/')
        && !path.contains('"')
        && !path.split('\\').any(|c| c == ".." || c == ".")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

/// Asks the user (UAC) to list `folders` elevated and returns what the helper found.
/// Blocks until the helper ends; call it off the UI thread.
pub fn request_reveal(folders: &[String]) -> Result<Vec<RevealedFolder>, RevealError> {
    if folders.len() > MAX_FOLDERS {
        return Err(RevealError::InvalidFolder("too many folders".into()));
    }
    if let Some(bad) = folders.iter().find(|f| !valid_folder(f)) {
        return Err(RevealError::InvalidFolder(bad.clone()));
    }
    let exe = std::env::current_exe().map_err(|_| RevealError::Failed(FsError::NotFound))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pipe = format!("unhoard-reveal-{}-{nonce:x}", std::process::id());
    let server = elevate::create_pipe(&pipe).map_err(RevealError::Failed)?;
    let payload = hex(&serde_json::to_vec(folders)
        .map_err(|_| RevealError::Failed(FsError::Other { code: 0 }))?);
    let args = format!("{HELPER_ARG} {pipe} {payload}");
    let process =
        match elevate::run_elevated(&exe.to_string_lossy(), &args).map_err(RevealError::Failed)? {
            Some(p) => p,
            None => return Err(RevealError::Declined),
        };
    let pid = process.id();
    let reader = std::thread::spawn(move || server.receive_from(pid, MAX_REPLY));
    if !process.wait(30 * 60 * 1000) {
        return Err(RevealError::Failed(FsError::Other { code: 1460 }));
    }
    if !reader.is_finished() {
        // The helper ended without connecting: connect once ourselves so the reader stops.
        // Its process-ID check rejects this connection.
        let _unblock = std::fs::OpenOptions::new()
            .write(true)
            .open(format!(r"\\.\pipe\{pipe}"));
    }
    let bytes = reader
        .join()
        .map_err(|_| RevealError::Failed(FsError::Other { code: 0 }))?
        .map_err(RevealError::Failed)?;
    serde_json::from_slice(&bytes).map_err(|_| RevealError::Failed(FsError::Other { code: 0 }))
}

/// Runs the helper: `args` are everything after [`HELPER_ARG`]. Returns the exit code.
pub fn run_helper(args: &[String]) -> i32 {
    let (Some(pipe), Some(payload)) = (args.first(), args.get(1)) else {
        return 2;
    };
    if !pipe.starts_with("unhoard-reveal-") || pipe.contains('\\') {
        return 2;
    }
    let Some(folders) = unhex(payload).and_then(|b| serde_json::from_slice::<Vec<String>>(&b).ok())
    else {
        return 2;
    };
    if folders.len() > MAX_FOLDERS || !folders.iter().all(|f| valid_folder(f)) {
        return 2;
    }
    if security::enable_backup_privilege().is_err() {
        return 3;
    }
    let ctx = ScanContext {
        rules: Rules::empty(),
        roots: Roots::default(),
        now: 0,
    };
    let results: Vec<RevealedFolder> = folders
        .iter()
        .map(
            |f| match scan(&WindowsFs, f, &ctx, &ScanControl::default()) {
                Ok((t, _)) => {
                    let root = t.get(t.root());
                    let children = t
                        .top_children(t.root(), TOP, false)
                        .0
                        .into_iter()
                        .filter_map(|c| t.get(c).map(|n| (n.name.to_string(), n.on_disk)))
                        .collect();
                    RevealedFolder {
                        path: f.clone(),
                        logical: root.map(|n| n.logical).unwrap_or(0),
                        on_disk: root.map(|n| n.on_disk).unwrap_or(0),
                        files: root.map(|n| n.files).unwrap_or(0),
                        children,
                        error: None,
                    }
                }
                Err(e) => RevealedFolder {
                    path: f.clone(),
                    logical: 0,
                    on_disk: 0,
                    files: 0,
                    children: vec![],
                    error: Some(e),
                },
            },
        )
        .collect();
    let Ok(json) = serde_json::to_vec(&results) else {
        return 4;
    };
    match std::fs::OpenOptions::new()
        .write(true)
        .open(format!(r"\\.\pipe\{pipe}"))
        .and_then(|mut p| p.write_all(&json))
    {
        Ok(()) => 0,
        Err(_) => 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f3_s7_helper_folders_are_validated() {
        assert!(valid_folder(r"C:\System Volume Information"));
        assert!(valid_folder(r"D:\Users\other"));
        for bad in [
            r"C:\a\..\b",
            "relative",
            r"\\server\share",
            r"C:/x",
            r#"C:\x" --evil"#,
            "",
        ] {
            assert!(!valid_folder(bad), "{bad}");
        }
        assert_eq!(unhex(&hex(b"[\"C:\\\\x\"]")).unwrap(), b"[\"C:\\\\x\"]");
        assert_eq!(run_helper(&["unhoard-reveal-1".into(), "zz".into()]), 2);
        assert_eq!(run_helper(&[r"..\evil".into(), hex(b"[]")]), 2);
    }
}

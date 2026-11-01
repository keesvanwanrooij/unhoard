//! The elevated, read-only helper behind "Reveal hidden space" and "Read protected
//! folders" (F3, S7). Risk class High.
//!
//! The app never runs elevated. To look into folders the user may not read, it starts its
//! own executable once more through the UAC prompt with `--reveal-helper` and a random pipe
//! name. The helper connects to the pipe; the app checks that the client is the process it
//! started and only then sends the folders to read. So the helper can start when a scan
//! starts and get its list when the scan ends. The helper validates every folder, enables
//! SeBackupPrivilege, lists the folders with the same read-only scanner, sends a summary
//! back and exits. It has no code path that writes, moves or deletes.

use std::io::{Read, Write};
use std::sync::mpsc::{Sender, channel};
use std::thread::JoinHandle;

use serde::{Deserialize, Serialize};
use unhoard_core::fs::FsError;
use unhoard_core::roots::Roots;
use unhoard_core::rules::Rules;
use unhoard_core::scan::{ScanContext, ScanControl, scan};

use crate::WindowsFs;
use crate::win::{elevate, security};

/// The argument that makes the executable run as the helper.
pub const HELPER_ARG: &str = "--reveal-helper";
const MAX_FOLDERS: usize = 4096;
const MAX_REQUEST: usize = 4 << 20;
const MAX_REPLY: usize = 32 << 20;
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

/// The request as sent over the pipe: a little-endian length, then the folders as JSON.
/// `None` if there are too many folders or one is invalid.
fn encode_request(folders: &[String]) -> Option<Vec<u8>> {
    if folders.len() > MAX_FOLDERS || !folders.iter().all(|f| valid_folder(f)) {
        return None;
    }
    let json = serde_json::to_vec(folders).ok()?;
    if json.len() > MAX_REQUEST {
        return None;
    }
    let mut out = u32::try_from(json.len()).ok()?.to_le_bytes().to_vec();
    out.extend(json);
    Some(out)
}

/// The folders of a request body (without its length), validated again by the helper.
fn decode_request(body: &[u8]) -> Option<Vec<String>> {
    let folders: Vec<String> = serde_json::from_slice(body).ok()?;
    (folders.len() <= MAX_FOLDERS && folders.iter().all(|f| valid_folder(f))).then_some(folders)
}

/// A started helper, waiting for its list of folders.
pub struct HelperSession {
    process: elevate::Process,
    pipe: String,
    request: Sender<Vec<u8>>,
    reply: JoinHandle<Result<Vec<u8>, FsError>>,
}

/// Starts the helper: Windows shows the UAC prompt now. Blocks only until the user answers.
pub fn start_helper() -> Result<HelperSession, RevealError> {
    let exe = std::env::current_exe().map_err(|_| RevealError::Failed(FsError::NotFound))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pipe = format!("unhoard-reveal-{}-{nonce:x}", std::process::id());
    let server = elevate::create_pipe(&pipe).map_err(RevealError::Failed)?;
    let process = elevate::run_elevated(&exe.to_string_lossy(), &format!("{HELPER_ARG} {pipe}"))
        .map_err(RevealError::Failed)?
        .ok_or(RevealError::Declined)?;
    let pid = process.id();
    let (request, rx) = channel();
    let reply = std::thread::spawn(move || server.serve(pid, &rx, MAX_REPLY));
    Ok(HelperSession {
        process,
        pipe,
        request,
        reply,
    })
}

impl HelperSession {
    /// Sends the folders to the helper and waits for what it found. Call it off the UI
    /// thread; the helper exits afterwards.
    pub fn read(self, folders: &[String]) -> Result<Vec<RevealedFolder>, RevealError> {
        let bad = || {
            folders
                .iter()
                .find(|f| !valid_folder(f))
                .cloned()
                .unwrap_or_else(|| "too many folders".into())
        };
        let body = encode_request(folders).ok_or_else(|| RevealError::InvalidFolder(bad()))?;
        // If the helper already ended, the reply thread reports that below.
        let _sent = self.request.send(body);
        if !self.process.wait(30 * 60 * 1000) {
            return Err(RevealError::Failed(FsError::Other { code: 1460 }));
        }
        if !self.reply.is_finished() {
            // The helper ended without connecting: connect once ourselves so the server
            // stops waiting. Its process-ID check rejects this connection.
            let _unblock = std::fs::OpenOptions::new()
                .write(true)
                .open(format!(r"\\.\pipe\{}", self.pipe));
        }
        let bytes = self
            .reply
            .join()
            .map_err(|_| RevealError::Failed(FsError::Other { code: 0 }))?
            .map_err(RevealError::Failed)?;
        serde_json::from_slice(&bytes).map_err(|_| RevealError::Failed(FsError::Other { code: 0 }))
    }
}

/// Asks the user (UAC) to list `folders` elevated and returns what the helper found.
/// Blocks until the helper ends; call it off the UI thread.
pub fn request_reveal(folders: &[String]) -> Result<Vec<RevealedFolder>, RevealError> {
    if encode_request(folders).is_none() {
        let bad = folders
            .iter()
            .find(|f| !valid_folder(f))
            .cloned()
            .unwrap_or_else(|| "too many folders".into());
        return Err(RevealError::InvalidFolder(bad));
    }
    start_helper()?.read(folders)
}

/// Lists one folder elevated and summarises it.
fn reveal_one(folder: &str, ctx: &ScanContext) -> RevealedFolder {
    match scan(&WindowsFs, folder, ctx, &ScanControl::default()) {
        Ok((t, _)) => {
            let root = t.get(t.root());
            let children = t
                .top_children(t.root(), TOP, false)
                .0
                .into_iter()
                .filter_map(|c| t.get(c).map(|n| (n.name.to_string(), n.on_disk)))
                .collect();
            RevealedFolder {
                path: folder.to_string(),
                logical: root.map(|n| n.logical).unwrap_or(0),
                on_disk: root.map(|n| n.on_disk).unwrap_or(0),
                files: root.map(|n| n.files).unwrap_or(0),
                children,
                error: None,
            }
        }
        Err(e) => RevealedFolder {
            path: folder.to_string(),
            logical: 0,
            on_disk: 0,
            files: 0,
            children: vec![],
            error: Some(e),
        },
    }
}

/// Runs the helper: `args` are everything after [`HELPER_ARG`]. Returns the exit code.
pub fn run_helper(args: &[String]) -> i32 {
    let Some(pipe) = args.first() else {
        return 2;
    };
    if !pipe.starts_with("unhoard-reveal-") || pipe.contains(['\\', '/']) {
        return 2;
    }
    let Ok(mut conn) = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(format!(r"\\.\pipe\{pipe}"))
    else {
        return 5;
    };
    let mut len = [0u8; 4];
    if conn.read_exact(&mut len).is_err() {
        return 6;
    }
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_REQUEST {
        return 2;
    }
    let mut body = vec![0u8; len];
    if conn.read_exact(&mut body).is_err() {
        return 6;
    }
    let Some(folders) = decode_request(&body) else {
        return 2;
    };
    if security::enable_backup_privilege().is_err() {
        return 3;
    }
    let ctx = ScanContext {
        rules: Rules::empty(),
        roots: Roots::default(),
        now: 0,
        knowledge: Default::default(),
    };
    let results: Vec<RevealedFolder> = folders.iter().map(|f| reveal_one(f, &ctx)).collect();
    let Ok(json) = serde_json::to_vec(&results) else {
        return 4;
    };
    match conn.write_all(&json) {
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
        assert_eq!(run_helper(&[r"..\evil".into()]), 2);
        assert_eq!(run_helper(&[]), 2);
    }

    #[test]
    fn f3_s7_requests_are_framed_bounded_and_validated() {
        let folders = vec![
            r"C:\System Volume Information".to_string(),
            r"D:\x".to_string(),
        ];
        let req = encode_request(&folders).unwrap();
        let len = u32::from_le_bytes(req[..4].try_into().unwrap()) as usize;
        assert_eq!(len, req.len() - 4);
        assert_eq!(decode_request(&req[4..]).unwrap(), folders);
        assert!(encode_request(&[r"C:\a\..\b".to_string()]).is_none());
        assert!(encode_request(&vec![r"C:\x".to_string(); MAX_FOLDERS + 1]).is_none());
        assert!(
            decode_request(br#"["C:\\a\\..\\b"]"#).is_none(),
            "the helper validates too"
        );
        assert!(decode_request(b"not json").is_none());
    }
}

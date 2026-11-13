//! The history file of storage history (F17): one JSON line per snapshot in
//! `%LOCALAPPDATA%\Unhoard\insights\history.jsonl`.
//!
//! Every write rewrites the whole file through a temporary file that replaces the old one
//! only when it is complete, so a crash or a full disk never loses the history (T-17.3).
//! The file is small (at most 6 MB), so a rewrite per snapshot costs nothing noticeable.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use unhoard_core::history::{MAX_BYTES, Snapshot, parse, retain, to_line};

/// The history file and a lock, so two writers never interleave.
pub struct HistoryStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl HistoryStore {
    /// A store for the file at `path`; the folder is created on the first write.
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    /// All snapshots, oldest first, and the number of lines that could not be read. A
    /// missing file is an empty history.
    pub fn load(&self) -> (Vec<Snapshot>, usize) {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => parse(&text),
            Err(_) => (vec![], 0),
        }
    }

    /// Adds `snapshot`, thins the history and writes it back atomically.
    pub fn add(&self, snapshot: Snapshot, now: i64) -> std::io::Result<()> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (mut list, _) = self.load();
        list.push(snapshot);
        let kept = retain(list, now, MAX_BYTES);
        let mut text = String::new();
        for s in &kept {
            text.push_str(&to_line(s));
            text.push('\n');
        }
        write_atomic(&self.path, text.as_bytes())
    }

    /// Removes the history file.
    pub fn clear(&self) -> std::io::Result<()> {
        let _guard = self
            .lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match std::fs::remove_file(&self.path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// Writes `bytes` to `path` through `path.tmp`, flushed to disk, then replaces `path`.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("jsonl.tmp");
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests fail loudly on purpose")]

    use super::*;
    use unhoard_core::history::{SnapshotKind, Totals};

    fn snap(at: i64) -> Snapshot {
        Snapshot {
            at,
            kind: SnapshotKind::Scan,
            root: r"C:\".into(),
            totals: Totals::default(),
            folders: vec![],
        }
    }

    #[test]
    fn f17_adding_appends_and_keeps_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = HistoryStore::new(dir.path().join("insights").join("history.jsonl"));
        store.add(snap(2_000), 3_000).unwrap();
        store.add(snap(1_000), 3_000).unwrap();
        let (list, skipped) = store.load();
        assert_eq!(
            list.iter().map(|s| s.at).collect::<Vec<_>>(),
            vec![1_000, 2_000]
        );
        assert_eq!(skipped, 0);
    }

    #[test]
    fn f17_t17_3_a_failed_write_leaves_the_old_file_intact() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.jsonl");
        let store = HistoryStore::new(path.clone());
        store.add(snap(1_000), 2_000).unwrap();
        let before = std::fs::read(&path).unwrap();
        // A folder where the temporary file should go makes the write fail midway.
        std::fs::create_dir(path.with_extension("jsonl.tmp")).unwrap();
        assert!(store.add(snap(1_500), 2_000).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

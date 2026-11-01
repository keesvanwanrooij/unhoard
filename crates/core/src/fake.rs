//! An in-memory [`FileSystem`] for tests (feature `fake`).
//!
//! It simulates what the tests of the safety invariants need: placeholders and pin states,
//! junctions, hard links, locked files, access denied, vanishing files, a full disk and a
//! clock. It records every open, so a test can prove that no cloud file was opened for its
//! data (S2), and it can take a snapshot to prove that an operation wrote nothing.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

use crate::fs::{
    ATTR_DIRECTORY, ATTR_PINNED, ATTR_RECALL_ON_DATA_ACCESS, ATTR_REPARSE_POINT, ATTR_UNPINNED,
    DirEntry, FileId, FileSystem, FileTime, FsError, Identity, TAG_MOUNT_POINT, VolumeInfo,
};

/// How a fake open accessed an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenKind {
    /// Attributes, identity or pin state only.
    Attributes,
    /// File data.
    Data,
}

/// One item in the fake file system.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Item {
    path: String,
    is_dir: bool,
    logical: u64,
    on_disk: u64,
    attributes: u32,
    reparse_tag: u32,
    file_id: FileId,
    modified: FileTime,
    data: Vec<u8>,
    denied: bool,
    locked: bool,
    in_sync: bool,
    lossy_name: bool,
}

#[derive(Default)]
struct State {
    items: BTreeMap<String, Item>,
    opens: Vec<(String, OpenKind)>,
    now: i64,
    full_disk: bool,
    next_id: u128,
    volume: Option<VolumeInfo>,
    vanish_on_move: Vec<String>,
}

/// The in-memory file system.
pub struct FakeFs {
    state: Mutex<State>,
}

/// Options for a fake file.
#[derive(Clone, Debug, Default)]
pub struct FileOpts {
    /// Logical size; defaults to the data length.
    pub logical: Option<u64>,
    /// Size on disk; defaults to the logical size rounded up to 4 KiB.
    pub on_disk: Option<u64>,
    /// Extra attributes.
    pub attributes: u32,
    /// File content.
    pub data: Vec<u8>,
    /// Last write time.
    pub modified: FileTime,
    /// Whether a cloud placeholder is in sync (uploaded).
    pub in_sync: bool,
    /// Whether the name is not valid Unicode.
    pub lossy_name: bool,
}

/// Normalises a path to the lookup key: lower case, backslashes, no trailing separator.
pub fn key(path: &str) -> String {
    let p = path.replace('/', "\\").to_lowercase();
    p.trim_end_matches('\\').to_string()
}

fn parent_key(k: &str) -> Option<&str> {
    k.rfind('\\').map(|i| &k[..i])
}

fn name_of(path: &str) -> String {
    path.trim_end_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or("")
        .to_string()
}

impl Default for FakeFs {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeFs {
    /// Creates an empty fake with a `C:` volume of 1 TiB and 100 GiB free.
    pub fn new() -> Self {
        let fs = Self {
            state: Mutex::new(State {
                now: 1_790_000_000,
                next_id: 1,
                ..State::default()
            }),
        };
        fs.add_dir("C:\\");
        fs
    }

    fn st(&self) -> MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn insert(&self, path: &str, mut item: Item) -> FileId {
        let mut st = self.st();
        st.next_id += 1;
        let id = FileId(st.next_id);
        if item.file_id == FileId(0) {
            item.file_id = id;
        }
        let fid = item.file_id;
        st.items.insert(key(path), item);
        fid
    }

    /// Adds a directory and returns its file ID.
    pub fn add_dir(&self, path: &str) -> FileId {
        self.insert(
            path,
            Item {
                path: path.trim_end_matches('\\').to_string(),
                is_dir: true,
                logical: 0,
                on_disk: 0,
                attributes: ATTR_DIRECTORY,
                reparse_tag: 0,
                file_id: FileId(0),
                modified: 1,
                data: vec![],
                denied: false,
                locked: false,
                in_sync: true,
                lossy_name: false,
            },
        )
    }

    /// Adds a file of `size` bytes with zero content.
    pub fn add_file(&self, path: &str, size: u64) -> FileId {
        self.add_file_with(
            path,
            FileOpts {
                logical: Some(size),
                ..FileOpts::default()
            },
        )
    }

    /// Adds a file with the given options.
    pub fn add_file_with(&self, path: &str, o: FileOpts) -> FileId {
        let logical = o.logical.unwrap_or(o.data.len() as u64);
        let on_disk = o.on_disk.unwrap_or(logical.div_ceil(4096) * 4096);
        self.insert(
            path,
            Item {
                path: path.to_string(),
                is_dir: false,
                logical,
                on_disk,
                attributes: o.attributes,
                reparse_tag: 0,
                file_id: FileId(0),
                modified: if o.modified == 0 { 1 } else { o.modified },
                data: o.data,
                denied: false,
                locked: false,
                in_sync: o.in_sync,
                lossy_name: o.lossy_name,
            },
        )
    }

    /// Adds an online-only cloud placeholder: logical size, nothing on disk.
    pub fn add_placeholder(&self, path: &str, size: u64, in_sync: bool) -> FileId {
        self.add_file_with(
            path,
            FileOpts {
                logical: Some(size),
                on_disk: Some(0),
                attributes: ATTR_RECALL_ON_DATA_ACCESS,
                in_sync,
                ..FileOpts::default()
            },
        )
    }

    /// Adds a second name for an existing file (a hard link).
    pub fn add_hard_link(&self, path: &str, existing: &str) {
        let mut st = self.st();
        if let Some(mut it) = st.items.get(&key(existing)).cloned() {
            it.path = path.to_string();
            st.items.insert(key(path), it);
        }
    }

    /// Adds a directory junction at `path`. Its target is never listed through it.
    pub fn add_junction(&self, path: &str) -> FileId {
        let id = self.add_dir(path);
        let mut st = self.st();
        if let Some(it) = st.items.get_mut(&key(path)) {
            it.attributes |= ATTR_REPARSE_POINT;
            it.reparse_tag = TAG_MOUNT_POINT;
        }
        id
    }

    /// Makes listing `path` fail with access denied.
    pub fn deny(&self, path: &str) {
        if let Some(it) = self.st().items.get_mut(&key(path)) {
            it.denied = true;
        }
    }

    /// Makes `path` locked by another program.
    pub fn lock(&self, path: &str) {
        if let Some(it) = self.st().items.get_mut(&key(path)) {
            it.locked = true;
        }
    }

    /// Makes `path` disappear right before it is moved.
    pub fn vanish_on_move(&self, path: &str) {
        self.st().vanish_on_move.push(key(path));
    }

    /// Changes the content and last write time of a file, as another program would.
    pub fn touch(&self, path: &str, new_size: u64) {
        let mut st = self.st();
        if let Some(it) = st.items.get_mut(&key(path)) {
            it.logical = new_size;
            it.modified += 1;
        }
    }

    /// Makes every durable write fail with a full disk.
    pub fn set_full_disk(&self, full: bool) {
        self.st().full_disk = full;
    }

    /// Sets the clock (seconds since 1970).
    pub fn set_now(&self, now: i64) {
        self.st().now = now;
    }

    /// Moves the clock forward by `days`.
    pub fn advance_days(&self, days: i64) {
        self.st().now += days * 86_400;
    }

    /// Sets the volume information returned for every path.
    pub fn set_volume(&self, v: VolumeInfo) {
        self.st().volume = Some(v);
    }

    /// Every open so far, with how it accessed the item.
    pub fn opens(&self) -> Vec<(String, OpenKind)> {
        self.st().opens.clone()
    }

    /// Opens that read file data.
    pub fn data_opens(&self) -> Vec<String> {
        self.st()
            .opens
            .iter()
            .filter(|(_, k)| *k == OpenKind::Data)
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// True if `path` exists.
    pub fn has(&self, path: &str) -> bool {
        self.st().items.contains_key(&key(path))
    }

    /// The content of a file.
    pub fn content(&self, path: &str) -> Option<Vec<u8>> {
        self.st().items.get(&key(path)).map(|i| i.data.clone())
    }

    /// The attributes of an item.
    pub fn attributes(&self, path: &str) -> Option<u32> {
        self.st().items.get(&key(path)).map(|i| i.attributes)
    }

    /// The logical size of an item.
    pub fn logical(&self, path: &str) -> Option<u64> {
        self.st().items.get(&key(path)).map(|i| i.logical)
    }

    /// A full description of the tree, to compare before and after an operation.
    pub fn snapshot(&self) -> Vec<String> {
        self.st()
            .items
            .values()
            .map(|i| {
                format!(
                    "{}|{}|{}|{}|{:?}|{}|{}",
                    i.path,
                    i.is_dir,
                    i.logical,
                    i.attributes,
                    i.file_id,
                    i.modified,
                    i.data.len()
                )
            })
            .collect()
    }

    fn record(st: &mut State, path: &str, kind: OpenKind) {
        st.opens.push((path.to_string(), kind));
    }

    fn ident(st: &State, it: &Item) -> Identity {
        Identity {
            volume_serial: st.volume.as_ref().map(|v| v.serial).unwrap_or(0xC0FFEE),
            file_id: it.file_id,
            size: if it.is_dir { 0 } else { it.logical },
            modified: it.modified,
            attributes: it.attributes,
        }
    }

    fn check(st: &State, k: &str, expected: &Identity) -> Result<(), FsError> {
        let it = st.items.get(k).ok_or(FsError::NotFound)?;
        if it.locked {
            return Err(FsError::Locked);
        }
        let now = Self::ident(st, it);
        if now.file_id != expected.file_id
            || now.size != expected.size
            || now.modified != expected.modified
            || now.volume_serial != expected.volume_serial
        {
            return Err(FsError::Changed);
        }
        Ok(())
    }

    fn subtree_keys(st: &State, k: &str) -> Vec<String> {
        let prefix = format!("{k}\\");
        st.items
            .keys()
            .filter(|x| *x == k || x.starts_with(&prefix))
            .cloned()
            .collect()
    }
}

impl FileSystem for FakeFs {
    fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        let st = self.st();
        let k = key(path);
        let it = st.items.get(&k).ok_or(FsError::NotFound)?;
        if !it.is_dir {
            return Err(FsError::Unsupported {
                reason: "not a directory".into(),
            });
        }
        if it.denied {
            return Err(FsError::AccessDenied);
        }
        let prefix = format!("{k}\\");
        Ok(st
            .items
            .range(prefix.clone()..)
            .take_while(|(x, _)| x.starts_with(&prefix))
            .filter(|(x, _)| !x[prefix.len()..].contains('\\'))
            .map(|(_, i)| DirEntry {
                name: name_of(&i.path),
                lossy_name: i.lossy_name,
                is_dir: i.is_dir,
                logical: i.logical,
                on_disk: i.on_disk,
                attributes: i.attributes,
                reparse_tag: i.reparse_tag,
                file_id: i.file_id,
                modified: i.modified,
                created: 1,
            })
            .collect())
    }

    fn volume(&self, _path: &str) -> Result<VolumeInfo, FsError> {
        Ok(self.st().volume.clone().unwrap_or(VolumeInfo {
            total_bytes: 1 << 40,
            free_bytes: 100 << 30,
            serial: 0xC0FFEE,
            file_system: "NTFS".into(),
        }))
    }

    fn identity(&self, path: &str) -> Result<Identity, FsError> {
        let mut st = self.st();
        Self::record(&mut st, path, OpenKind::Attributes);
        let it = st.items.get(&key(path)).ok_or(FsError::NotFound)?;
        Ok(Self::ident(&st, it))
    }

    fn exists(&self, path: &str) -> bool {
        self.has(path)
    }

    fn move_verified(&self, from: &str, expected: &Identity, to: &str) -> Result<(), FsError> {
        let mut st = self.st();
        Self::record(&mut st, from, OpenKind::Attributes);
        let fk = key(from);
        if let Some(pos) = st.vanish_on_move.iter().position(|v| *v == fk) {
            st.vanish_on_move.remove(pos);
            for k in Self::subtree_keys(&st, &fk) {
                st.items.remove(&k);
            }
        }
        Self::check(&st, &fk, expected)?;
        let tk = key(to);
        if st.items.contains_key(&tk) {
            return Err(FsError::AlreadyExists);
        }
        match parent_key(&tk).and_then(|p| st.items.get(p)) {
            Some(p) if p.is_dir => {}
            _ => return Err(FsError::NotFound),
        }
        let to_display = to.trim_end_matches('\\').to_string();
        for k in Self::subtree_keys(&st, &fk) {
            if let Some(mut it) = st.items.remove(&k) {
                let rest = &k[fk.len()..];
                it.path = format!("{to_display}{}", &it.path[it.path.len() - rest.len()..]);
                st.items.insert(format!("{tk}{rest}"), it);
            }
        }
        Ok(())
    }

    fn delete_verified(&self, path: &str, expected: &Identity) -> Result<(), FsError> {
        let mut st = self.st();
        Self::record(&mut st, path, OpenKind::Attributes);
        let k = key(path);
        Self::check(&st, &k, expected)?;
        for x in Self::subtree_keys(&st, &k) {
            st.items.remove(&x);
        }
        Ok(())
    }

    fn delete_tree(&self, path: &str) -> Result<(), FsError> {
        let mut st = self.st();
        let k = key(path);
        if !st.items.contains_key(&k) {
            return Err(FsError::NotFound);
        }
        for x in Self::subtree_keys(&st, &k) {
            st.items.remove(&x);
        }
        Ok(())
    }

    fn create_dir_all(&self, path: &str) -> Result<(), FsError> {
        let parts: Vec<&str> = path.trim_end_matches('\\').split('\\').collect();
        for i in 1..=parts.len() {
            let p = parts[..i].join("\\");
            let exists = self.st().items.get(&key(&p)).map(|it| it.is_dir);
            match exists {
                Some(true) => {}
                Some(false) => return Err(FsError::AlreadyExists),
                None => {
                    if self.st().full_disk {
                        return Err(FsError::DiskFull);
                    }
                    self.add_dir(&p);
                }
            }
        }
        Ok(())
    }

    fn create_private_dir(&self, path: &str) -> Result<(), FsError> {
        self.create_dir_all(path)
    }

    fn write_durable(&self, path: &str, bytes: &[u8]) -> Result<(), FsError> {
        let k = key(path);
        {
            let st = self.st();
            if st.full_disk {
                return Err(FsError::DiskFull);
            }
            match parent_key(&k).and_then(|p| st.items.get(p)) {
                Some(p) if p.is_dir => {}
                _ => return Err(FsError::NotFound),
            }
        }
        let existing = self.st().items.get(&k).map(|i| i.file_id);
        match existing {
            Some(_) => {
                let mut st = self.st();
                if let Some(it) = st.items.get_mut(&k) {
                    it.data = bytes.to_vec();
                    it.logical = bytes.len() as u64;
                    it.modified += 1;
                }
            }
            None => {
                self.add_file_with(
                    path,
                    FileOpts {
                        data: bytes.to_vec(),
                        ..FileOpts::default()
                    },
                );
            }
        }
        Ok(())
    }

    fn read_small(&self, path: &str, max: usize) -> Result<Vec<u8>, FsError> {
        let st = self.st();
        let it = st.items.get(&key(path)).ok_or(FsError::NotFound)?;
        if it.data.len() > max {
            return Err(FsError::Unsupported {
                reason: "file too large".into(),
            });
        }
        Ok(it.data.clone())
    }

    fn append_line(&self, path: &str, line: &str) -> Result<(), FsError> {
        let k = key(path);
        if !self.has(path) {
            self.write_durable(path, b"")?;
        }
        let mut st = self.st();
        if st.full_disk {
            return Err(FsError::DiskFull);
        }
        if let Some(it) = st.items.get_mut(&k) {
            it.data.extend_from_slice(line.as_bytes());
            it.data.push(b'\n');
            it.logical = it.data.len() as u64;
        }
        Ok(())
    }

    fn read_range(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>, FsError> {
        let mut st = self.st();
        Self::record(&mut st, path, OpenKind::Data);
        let it = st.items.get(&key(path)).ok_or(FsError::NotFound)?;
        let start = (offset as usize).min(it.data.len());
        let end = start.saturating_add(len).min(it.data.len());
        Ok(it.data[start..end].to_vec())
    }

    fn placeholder_in_sync(&self, path: &str) -> Result<bool, FsError> {
        let mut st = self.st();
        Self::record(&mut st, path, OpenKind::Attributes);
        st.items
            .get(&key(path))
            .map(|i| i.in_sync)
            .ok_or(FsError::NotFound)
    }

    fn set_unpinned(&self, path: &str) -> Result<(), FsError> {
        let mut st = self.st();
        Self::record(&mut st, path, OpenKind::Attributes);
        let it = st.items.get_mut(&key(path)).ok_or(FsError::NotFound)?;
        it.attributes = (it.attributes & !ATTR_PINNED) | ATTR_UNPINNED | ATTR_RECALL_ON_DATA_ACCESS;
        it.on_disk = 0;
        Ok(())
    }

    fn now(&self) -> i64 {
        self.st().now
    }
}

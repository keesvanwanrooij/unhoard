//! [`WindowsFs`]: the core's [`FileSystem`] on real Windows volumes.
//!
//! Operations that need handle-level guarantees (listing, identity, rename and delete by
//! verified handle, pin state, placeholder state, private folders) go through the FFI
//! module; plain file writes use the standard library. Reading file data refuses cloud
//! files that are not on this PC (S2), as defence in depth behind the core's own checks.

use std::io::{Read, Seek, SeekFrom, Write};

use unhoard_core::fs::{
    ATTR_OFFLINE, ATTR_PINNED, ATTR_RECALL_ON_DATA_ACCESS, ATTR_RECALL_ON_OPEN, ATTR_UNPINNED,
    DirEntry, FileSystem, FsError, Identity, VolumeInfo,
};

use crate::win::{cloud, files, map_io, security};

/// The Windows file system.
#[derive(Clone, Copy, Debug, Default)]
pub struct WindowsFs;

fn is_online_only(path: &str) -> bool {
    files::attributes(path)
        .is_some_and(|a| a & (ATTR_RECALL_ON_DATA_ACCESS | ATTR_RECALL_ON_OPEN | ATTR_OFFLINE) != 0)
}

fn refuse_cloud(path: &str) -> Result<(), FsError> {
    if is_online_only(path) {
        Err(FsError::Unsupported {
            reason: "online-only cloud file".into(),
        })
    } else {
        Ok(())
    }
}

impl FileSystem for WindowsFs {
    fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError> {
        files::list_dir(path)
    }

    fn volume(&self, path: &str) -> Result<VolumeInfo, FsError> {
        files::volume(path)
    }

    fn identity(&self, path: &str) -> Result<Identity, FsError> {
        files::identity(path)
    }

    fn exists(&self, path: &str) -> bool {
        files::attributes(path).is_some()
    }

    fn move_verified(&self, from: &str, expected: &Identity, to: &str) -> Result<(), FsError> {
        files::rename_verified(from, expected, to)
    }

    fn delete_verified(&self, path: &str, expected: &Identity) -> Result<(), FsError> {
        files::delete_verified(path, expected)
    }

    fn delete_tree(&self, path: &str) -> Result<(), FsError> {
        files::delete_tree(path)
    }

    fn create_dir_all(&self, path: &str) -> Result<(), FsError> {
        std::fs::create_dir_all(path).map_err(|e| map_io(&e))
    }

    fn create_private_dir(&self, path: &str) -> Result<(), FsError> {
        if let Some(parent) = unhoard_core::roots::parent_of(path) {
            self.create_dir_all(&parent)?;
        }
        security::create_private_dir(path)
    }

    fn write_durable(&self, path: &str, bytes: &[u8]) -> Result<(), FsError> {
        let tmp = format!("{path}.unhoard-tmp");
        let write = || -> std::io::Result<()> {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, path)
        };
        write().map_err(|e| {
            let _cleanup = std::fs::remove_file(&tmp);
            map_io(&e)
        })
    }

    fn read_small(&self, path: &str, max: usize) -> Result<Vec<u8>, FsError> {
        refuse_cloud(path)?;
        let f = std::fs::File::open(path).map_err(|e| map_io(&e))?;
        let len = f.metadata().map_err(|e| map_io(&e))?.len();
        if len > max as u64 {
            return Err(FsError::Unsupported {
                reason: "file too large".into(),
            });
        }
        let mut out = Vec::with_capacity(len as usize);
        f.take(max as u64)
            .read_to_end(&mut out)
            .map_err(|e| map_io(&e))?;
        Ok(out)
    }

    fn append_line(&self, path: &str, line: &str) -> Result<(), FsError> {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| map_io(&e))?;
        f.write_all(format!("{line}\n").as_bytes())
            .and_then(|()| f.sync_data())
            .map_err(|e| map_io(&e))
    }

    fn read_range(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>, FsError> {
        refuse_cloud(path)?;
        let mut f = std::fs::File::open(path).map_err(|e| map_io(&e))?;
        f.seek(SeekFrom::Start(offset)).map_err(|e| map_io(&e))?;
        let mut out = vec![];
        f.take(len as u64)
            .read_to_end(&mut out)
            .map_err(|e| map_io(&e))?;
        Ok(out)
    }

    fn placeholder_in_sync(&self, path: &str) -> Result<bool, FsError> {
        cloud::in_sync(path)
    }

    fn set_unpinned(&self, path: &str) -> Result<(), FsError> {
        let a = files::attributes(path).ok_or(FsError::NotFound)?;
        files::set_attributes(path, (a & !ATTR_PINNED) | ATTR_UNPINNED)
    }

    fn now(&self) -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

//! The file-system contract between the core and the platform layer.
//!
//! The core never touches the disk itself. Everything it needs from the operating system
//! goes through [`FileSystem`], which the Windows platform layer implements and the fake
//! (feature `fake`) simulates in memory for tests. Paths are absolute Windows paths with
//! backslashes; the implementation handles the `\\?\` long-path form.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A Windows FILETIME: 100-nanosecond intervals since 1601-01-01 UTC.
pub type FileTime = u64;

/// The 128-bit NTFS file ID (`FILE_ID_128`), unique per volume.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct FileId(pub u128);

/// `FILE_ATTRIBUTE_READONLY`.
pub const ATTR_READONLY: u32 = 0x1;
/// `FILE_ATTRIBUTE_HIDDEN`.
pub const ATTR_HIDDEN: u32 = 0x2;
/// `FILE_ATTRIBUTE_SYSTEM`.
pub const ATTR_SYSTEM: u32 = 0x4;
/// `FILE_ATTRIBUTE_DIRECTORY`.
pub const ATTR_DIRECTORY: u32 = 0x10;
/// `FILE_ATTRIBUTE_REPARSE_POINT`.
pub const ATTR_REPARSE_POINT: u32 = 0x400;
/// `FILE_ATTRIBUTE_OFFLINE`.
pub const ATTR_OFFLINE: u32 = 0x1000;
/// `FILE_ATTRIBUTE_PINNED`: "always keep on this device".
pub const ATTR_PINNED: u32 = 0x80000;
/// `FILE_ATTRIBUTE_UNPINNED`: the user asked to free up space.
pub const ATTR_UNPINNED: u32 = 0x100000;
/// `FILE_ATTRIBUTE_RECALL_ON_OPEN`.
pub const ATTR_RECALL_ON_OPEN: u32 = 0x40000;
/// `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`: an online-only placeholder.
pub const ATTR_RECALL_ON_DATA_ACCESS: u32 = 0x400000;

/// `IO_REPARSE_TAG_MOUNT_POINT`: a junction. Never followed.
pub const TAG_MOUNT_POINT: u32 = 0xA000_0003;
/// `IO_REPARSE_TAG_SYMLINK`: a symbolic link. Never followed.
pub const TAG_SYMLINK: u32 = 0xA000_000C;

/// One entry of a directory listing, read in a batch without opening the entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    /// The entry's name. Not valid Unicode names are converted lossily and flagged.
    pub name: String,
    /// True when `name` could not be represented exactly.
    pub lossy_name: bool,
    /// True for a directory (including a directory junction).
    pub is_dir: bool,
    /// Logical size (`EndOfFile`).
    pub logical: u64,
    /// Size on disk (`AllocationSize`).
    pub on_disk: u64,
    /// The `FILE_ATTRIBUTE_*` flags.
    pub attributes: u32,
    /// The reparse tag, or 0.
    pub reparse_tag: u32,
    /// The 128-bit file ID.
    pub file_id: FileId,
    /// Last write time.
    pub modified: FileTime,
    /// Creation time.
    pub created: FileTime,
}

impl DirEntry {
    /// True for a junction or symbolic link, which is recorded but never followed.
    pub fn is_link(&self) -> bool {
        self.attributes & ATTR_REPARSE_POINT != 0
            && (self.reparse_tag == TAG_MOUNT_POINT || self.reparse_tag == TAG_SYMLINK)
    }
}

/// What identifies an item on disk, read through a handle that does not follow links.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    /// The volume serial number.
    pub volume_serial: u64,
    /// The 128-bit file ID.
    pub file_id: FileId,
    /// Logical size; 0 for a directory.
    pub size: u64,
    /// Last write time.
    pub modified: FileTime,
    /// The `FILE_ATTRIBUTE_*` flags.
    pub attributes: u32,
}

/// Space and identity of a volume.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeInfo {
    /// Total size in bytes.
    pub total_bytes: u64,
    /// Free bytes available to the user.
    pub free_bytes: u64,
    /// The volume serial number.
    pub serial: u64,
    /// The file system name, for example `NTFS`.
    pub file_system: String,
}

/// A file-system failure, reduced to the cases the core handles differently.
#[derive(Clone, Debug, Error, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FsError {
    /// The item does not exist (any more).
    #[error("not found")]
    NotFound,
    /// The user may not read or change the item.
    #[error("access denied")]
    AccessDenied,
    /// Another process has the item open.
    #[error("in use by another program")]
    Locked,
    /// The disk is full.
    #[error("the disk is full")]
    DiskFull,
    /// The destination already exists.
    #[error("already exists")]
    AlreadyExists,
    /// The item is not what the scan saw: it was changed or replaced.
    #[error("changed since the scan")]
    Changed,
    /// The operation is not supported for this item or volume.
    #[error("not supported: {reason}")]
    Unsupported {
        /// Why it is not supported.
        reason: String,
    },
    /// Any other error, with the OS error code.
    #[error("system error {code}")]
    Other {
        /// The Win32 error code.
        code: u32,
    },
}

/// The operations the core needs from the operating system.
///
/// Every method that changes something acts only on the exact path it is given and never
/// follows junctions or symbolic links.
pub trait FileSystem: Send + Sync {
    /// Lists a directory in one batched call, without opening its entries.
    fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>, FsError>;

    /// Returns the size, free space and serial of the volume that holds `path`.
    fn volume(&self, path: &str) -> Result<VolumeInfo, FsError>;

    /// Opens `path` without following links and with attribute access only, and returns
    /// its identity. Never reads file data.
    fn identity(&self, path: &str) -> Result<Identity, FsError>;

    /// True if `path` exists, without following a link at its end.
    fn exists(&self, path: &str) -> bool;

    /// Opens `from` without following links, checks it still has the `expected` identity
    /// (volume, file ID, size and last write), and renames it to `to` through that same
    /// handle. Fails with [`FsError::Changed`] on a mismatch and never overwrites `to`.
    fn move_verified(&self, from: &str, expected: &Identity, to: &str) -> Result<(), FsError>;

    /// Opens `path` without following links, checks the `expected` identity, and deletes
    /// it. A directory is deleted with its contents; links inside it are removed as links.
    fn delete_verified(&self, path: &str, expected: &Identity) -> Result<(), FsError>;

    /// Deletes a directory tree that holds only Unhoard's own files (an emptied batch).
    fn delete_tree(&self, path: &str) -> Result<(), FsError>;

    /// Creates a directory and its missing parents.
    fn create_dir_all(&self, path: &str) -> Result<(), FsError>;

    /// Creates a directory readable only by the current user, SYSTEM and Administrators,
    /// with permissions that are not inherited from its parent.
    fn create_private_dir(&self, path: &str) -> Result<(), FsError>;

    /// Writes a small file completely and flushes it to disk before returning, replacing
    /// any previous version atomically.
    fn write_durable(&self, path: &str, bytes: &[u8]) -> Result<(), FsError>;

    /// Reads a small file of Unhoard's own (a manifest), at most `max` bytes.
    fn read_small(&self, path: &str, max: usize) -> Result<Vec<u8>, FsError>;

    /// Appends one line to a log file, creating it when needed, and flushes it.
    fn append_line(&self, path: &str, line: &str) -> Result<(), FsError>;

    /// Reads `len` bytes of file data at `offset`. Only for local files: the core never
    /// calls it for a file inside a cloud sync root.
    fn read_range(&self, path: &str, offset: u64, len: usize) -> Result<Vec<u8>, FsError>;

    /// True when a cloud placeholder is fully uploaded ("in sync"). Opens the file with
    /// attribute access only.
    fn placeholder_in_sync(&self, path: &str) -> Result<bool, FsError>;

    /// Clears "always keep" and sets "free up space" on a cloud file, like
    /// `attrib -p +u`. Changes only the pin state.
    fn set_unpinned(&self, path: &str) -> Result<(), FsError>;

    /// The current time as seconds since 1970-01-01 UTC.
    fn now(&self) -> i64;
}

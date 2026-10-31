//! Directory listings, identities, rename by handle and delete without following links.

use std::mem::{offset_of, size_of};

use unhoard_core::fs::{
    ATTR_DIRECTORY, ATTR_REPARSE_POINT, DirEntry, FileId, FsError, Identity, VolumeInfo,
};
use windows::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CreateFileW, DELETE, FILE_ACCESS_RIGHTS, FILE_BASIC_INFO,
    FILE_DISPOSITION_FLAG_DELETE, FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE,
    FILE_DISPOSITION_FLAG_POSIX_SEMANTICS, FILE_DISPOSITION_INFO, FILE_DISPOSITION_INFO_EX,
    FILE_DISPOSITION_INFO_EX_FLAGS, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_FLAGS_AND_ATTRIBUTES, FILE_ID_BOTH_DIR_INFO, FILE_ID_EXTD_DIR_INFO, FILE_ID_INFO,
    FILE_INFO_BY_HANDLE_CLASS, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_RENAME_INFO,
    FILE_SHARE_DELETE, FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_STANDARD_INFO,
    FileBasicInfo, FileDispositionInfo, FileDispositionInfoEx, FileIdBothDirectoryInfo,
    FileIdBothDirectoryRestartInfo, FileIdExtdDirectoryInfo, FileIdExtdDirectoryRestartInfo,
    FileIdInfo, FileRenameInfo, FileStandardInfo, GetDiskFreeSpaceExW, GetDriveTypeW,
    GetFileAttributesW, GetFileInformationByHandle, GetFileInformationByHandleEx,
    GetLogicalDriveStringsW, GetVolumeInformationW, GetVolumePathNameW, INVALID_FILE_ATTRIBUTES,
    OPEN_EXISTING, SYNCHRONIZE, SetFileAttributesW, SetFileInformationByHandle,
};

use super::{Handle, Wide, from_wide, map_err};

const NO_MORE_FILES: u32 = 18;
const DRIVE_FIXED: u32 = 3;

/// Opens `path` with the given access. `follow` decides whether a link at the end of the
/// path is followed; Unhoard only follows when it lists a folder it already knows is not a
/// link (cloud folders need the filter engaged to list completely).
pub(crate) fn open(
    path: &str,
    access: FILE_ACCESS_RIGHTS,
    share: FILE_SHARE_MODE,
    follow: bool,
) -> Result<Handle, FsError> {
    let w = Wide::path(path);
    let mut flags = FILE_FLAG_BACKUP_SEMANTICS;
    if !follow {
        flags |= FILE_FLAG_OPEN_REPARSE_POINT;
    }
    // SAFETY: `w` is a valid NUL-terminated wide string for the duration of the call; no
    // security attributes or template handle are passed.
    let h = unsafe {
        CreateFileW(
            w.pcwstr(),
            (access | SYNCHRONIZE).0,
            share,
            None,
            OPEN_EXISTING,
            flags,
            None,
        )
    };
    h.map(Handle).map_err(|e| map_err(&e))
}

fn info<T: Default>(h: &Handle, class: FILE_INFO_BY_HANDLE_CLASS) -> Result<T, FsError> {
    let mut out = T::default();
    // SAFETY: `out` is a properly aligned, writable `T` of exactly the size passed, and `T`
    // is the structure Windows documents for `class`.
    unsafe {
        GetFileInformationByHandleEx(h.0, class, (&raw mut out).cast(), size_of::<T>() as u32)
    }
    .map_err(|e| map_err(&e))?;
    Ok(out)
}

/// The identity of an open item: volume, 128-bit file ID, size, last write and attributes.
pub(crate) fn handle_identity(h: &Handle) -> Result<Identity, FsError> {
    let basic: FILE_BASIC_INFO = info(h, FileBasicInfo)?;
    let std: FILE_STANDARD_INFO = info(h, FileStandardInfo)?;
    let (serial, id) = match info::<FILE_ID_INFO>(h, FileIdInfo) {
        Ok(i) => (
            i.VolumeSerialNumber,
            u128::from_le_bytes(i.FileId.Identifier),
        ),
        Err(_) => {
            // FAT and exFAT have no FILE_ID_INFO; fall back to the 64-bit index.
            let mut b = BY_HANDLE_FILE_INFORMATION::default();
            // SAFETY: `b` is a valid, writable BY_HANDLE_FILE_INFORMATION.
            unsafe { GetFileInformationByHandle(h.0, &mut b) }.map_err(|e| map_err(&e))?;
            (
                u64::from(b.dwVolumeSerialNumber),
                (u128::from(b.nFileIndexHigh) << 32) | u128::from(b.nFileIndexLow),
            )
        }
    };
    Ok(Identity {
        volume_serial: serial,
        file_id: FileId(id),
        size: if std.Directory {
            0
        } else {
            std.EndOfFile.max(0) as u64
        },
        modified: basic.LastWriteTime.max(0) as u64,
        attributes: basic.FileAttributes,
    })
}

/// Opens `path` without following links, with attribute access only, and reads its identity.
pub(crate) fn identity(path: &str) -> Result<Identity, FsError> {
    let h = open(
        path,
        FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        false,
    )?;
    handle_identity(&h)
}

fn same(a: &Identity, b: &Identity) -> bool {
    a.volume_serial == b.volume_serial
        && a.file_id == b.file_id
        && a.size == b.size
        && a.modified == b.modified
}

/// Opens `path` without following links and without sharing delete access, so nobody can
/// rename or replace it while Unhoard acts, and checks it is still `expected`.
fn open_verified(path: &str, expected: &Identity) -> Result<Handle, FsError> {
    let h = open(
        path,
        DELETE | FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        false,
    )?;
    if !same(&handle_identity(&h)?, expected) {
        return Err(FsError::Changed);
    }
    Ok(h)
}

/// Renames the item at `from` to `to` through a verified handle. Never replaces `to`.
pub(crate) fn rename_verified(from: &str, expected: &Identity, to: &str) -> Result<(), FsError> {
    let h = open_verified(from, expected)?;
    // An NT path (`\??\C:\…`) is accepted by every Windows version for FileRenameInfo.
    let target: Vec<u16> = format!(r"\??\{}", super::verbatim(to).trim_start_matches(r"\\?\"))
        .encode_utf16()
        .collect();
    let name_offset = offset_of!(FILE_RENAME_INFO, FileName);
    let bytes = name_offset + target.len() * 2 + 2;
    let mut buf = vec![0u64; bytes.div_ceil(8)];
    let info = buf.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: `buf` is 8-byte aligned and at least `bytes` long, which covers the fixed part
    // of FILE_RENAME_INFO plus the name and a terminating NUL. The name is copied within it.
    unsafe {
        (*info).Anonymous.ReplaceIfExists = false;
        (*info).FileNameLength = (target.len() * 2) as u32;
        std::ptr::copy_nonoverlapping(
            target.as_ptr(),
            buf.as_mut_ptr().cast::<u8>().add(name_offset).cast::<u16>(),
            target.len(),
        );
    }
    // SAFETY: the buffer holds a complete FILE_RENAME_INFO of `bytes` bytes.
    unsafe { SetFileInformationByHandle(h.0, FileRenameInfo, buf.as_ptr().cast(), bytes as u32) }
        .map_err(|e| map_err(&e))
}

/// Marks an open item for deletion. Links are deleted as links, because the handle was
/// opened without following them.
fn dispose(h: &Handle) -> Result<(), FsError> {
    let ex = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_INFO_EX_FLAGS(
            FILE_DISPOSITION_FLAG_DELETE.0
                | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS.0
                | FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE.0,
        ),
    };
    // SAFETY: `ex` is a valid FILE_DISPOSITION_INFO_EX of the size passed.
    let r = unsafe {
        SetFileInformationByHandle(
            h.0,
            FileDispositionInfoEx,
            (&raw const ex).cast(),
            size_of::<FILE_DISPOSITION_INFO_EX>() as u32,
        )
    };
    if r.is_ok() {
        return Ok(());
    }
    // File systems without POSIX delete (FAT) take the classic disposition.
    let classic = FILE_DISPOSITION_INFO { DeleteFile: true };
    // SAFETY: `classic` is a valid FILE_DISPOSITION_INFO of the size passed.
    unsafe {
        SetFileInformationByHandle(
            h.0,
            FileDispositionInfo,
            (&raw const classic).cast(),
            size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    }
    .map_err(|e| map_err(&e))
}

/// Deletes the contents of a folder, depth first, never following links.
fn delete_children(path: &str) -> Result<(), FsError> {
    for e in list_dir(path)? {
        let child = format!(r"{}\{}", path.trim_end_matches('\\'), e.name);
        let h = open(
            &child,
            DELETE | FILE_READ_ATTRIBUTES,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            false,
        )?;
        if e.is_dir && e.attributes & ATTR_REPARSE_POINT == 0 {
            delete_children(&child)?;
        }
        dispose(&h)?;
    }
    Ok(())
}

/// Deletes the item at `path` if it is still `expected`; a folder with its contents.
pub(crate) fn delete_verified(path: &str, expected: &Identity) -> Result<(), FsError> {
    let h = open_verified(path, expected)?;
    let id = handle_identity(&h)?;
    if id.attributes & ATTR_DIRECTORY != 0 && id.attributes & ATTR_REPARSE_POINT == 0 {
        delete_children(path)?;
    }
    dispose(&h)
}

/// Deletes a folder tree of Unhoard's own (an emptied batch), never following links.
pub(crate) fn delete_tree(path: &str) -> Result<(), FsError> {
    let h = open(
        path,
        DELETE | FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        false,
    )?;
    let id = handle_identity(&h)?;
    if id.attributes & ATTR_DIRECTORY != 0 && id.attributes & ATTR_REPARSE_POINT == 0 {
        delete_children(path)?;
    }
    dispose(&h)
}

/// Reads one entry of a FILE_ID_EXTD_DIR_INFO or FILE_ID_BOTH_DIR_INFO buffer.
struct RawEntry {
    next: u32,
    entry: DirEntry,
}

fn name_at(buf: &[u8], offset: usize, len_bytes: usize) -> Option<(String, bool)> {
    let bytes = buf.get(offset..offset + len_bytes)?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    Some(match String::from_utf16(&units) {
        Ok(s) => (s, false),
        Err(_) => (String::from_utf16_lossy(&units), true),
    })
}

fn parse_extd(buf: &[u8], off: usize) -> Option<RawEntry> {
    let fixed = offset_of!(FILE_ID_EXTD_DIR_INFO, FileName);
    if off + fixed > buf.len() {
        return None;
    }
    // SAFETY: at least `fixed` bytes are available at `off` (checked above); the read is
    // unaligned-safe and FILE_ID_EXTD_DIR_INFO is plain data.
    let i: FILE_ID_EXTD_DIR_INFO =
        unsafe { std::ptr::read_unaligned(buf.as_ptr().add(off).cast()) };
    let (name, lossy) = name_at(buf, off + fixed, i.FileNameLength as usize)?;
    Some(RawEntry {
        next: i.NextEntryOffset,
        entry: DirEntry {
            name,
            lossy_name: lossy,
            is_dir: i.FileAttributes & ATTR_DIRECTORY != 0,
            logical: i.EndOfFile.max(0) as u64,
            on_disk: i.AllocationSize.max(0) as u64,
            attributes: i.FileAttributes,
            reparse_tag: if i.FileAttributes & ATTR_REPARSE_POINT != 0 {
                i.ReparsePointTag
            } else {
                0
            },
            file_id: FileId(u128::from_le_bytes(i.FileId.Identifier)),
            modified: i.LastWriteTime.max(0) as u64,
            created: i.CreationTime.max(0) as u64,
        },
    })
}

fn parse_both(buf: &[u8], off: usize) -> Option<RawEntry> {
    let fixed = offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
    if off + fixed > buf.len() {
        return None;
    }
    // SAFETY: as in `parse_extd`, for FILE_ID_BOTH_DIR_INFO.
    let i: FILE_ID_BOTH_DIR_INFO =
        unsafe { std::ptr::read_unaligned(buf.as_ptr().add(off).cast()) };
    let (name, lossy) = name_at(buf, off + fixed, i.FileNameLength as usize)?;
    Some(RawEntry {
        next: i.NextEntryOffset,
        entry: DirEntry {
            name,
            lossy_name: lossy,
            is_dir: i.FileAttributes & ATTR_DIRECTORY != 0,
            logical: i.EndOfFile.max(0) as u64,
            on_disk: i.AllocationSize.max(0) as u64,
            attributes: i.FileAttributes,
            // For reparse points, the EA size field carries the reparse tag.
            reparse_tag: if i.FileAttributes & ATTR_REPARSE_POINT != 0 {
                i.EaSize
            } else {
                0
            },
            file_id: FileId(i.FileId as u64 as u128),
            modified: i.LastWriteTime.max(0) as u64,
            created: i.CreationTime.max(0) as u64,
        },
    })
}

/// Lists a folder in batches of 64 KiB without opening its entries.
pub(crate) fn list_dir(path: &str) -> Result<Vec<DirEntry>, FsError> {
    let h = open(
        path,
        FILE_LIST_DIRECTORY,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        true,
    )?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut out = vec![];
    let mut extd = true;
    let mut first = true;
    loop {
        let class = match (extd, first) {
            (true, true) => FileIdExtdDirectoryRestartInfo,
            (true, false) => FileIdExtdDirectoryInfo,
            (false, true) => FileIdBothDirectoryRestartInfo,
            (false, false) => FileIdBothDirectoryInfo,
        };
        // SAFETY: `buf` is writable for its full length, which is the size passed.
        let r = unsafe {
            GetFileInformationByHandleEx(h.0, class, buf.as_mut_ptr().cast(), buf.len() as u32)
        };
        if let Err(e) = r {
            let code = e.code().0 as u32 & 0xFFFF;
            if code == NO_MORE_FILES {
                break;
            }
            if extd && first && (code == 87 || code == 1 || code == 124) {
                // FAT and exFAT do not support the extended class.
                extd = false;
                continue;
            }
            return Err(map_err(&e));
        }
        first = false;
        let mut off = 0usize;
        let parse = if extd { parse_extd } else { parse_both };
        while let Some(raw) = parse(&buf, off) {
            if raw.entry.name != "." && raw.entry.name != ".." {
                out.push(raw.entry);
            }
            if raw.next == 0 {
                break;
            }
            off += raw.next as usize;
        }
    }
    Ok(out)
}

/// The attributes of `path` without following a link at its end.
pub(crate) fn attributes(path: &str) -> Option<u32> {
    let w = Wide::path(path);
    // SAFETY: `w` is a valid NUL-terminated wide string.
    let a = unsafe { GetFileAttributesW(w.pcwstr()) };
    (a != INVALID_FILE_ATTRIBUTES).then_some(a)
}

/// Sets the attributes of `path`.
pub(crate) fn set_attributes(path: &str, attributes: u32) -> Result<(), FsError> {
    let w = Wide::path(path);
    // SAFETY: `w` is a valid NUL-terminated wide string.
    unsafe { SetFileAttributesW(w.pcwstr(), FILE_FLAGS_AND_ATTRIBUTES(attributes)) }
        .map_err(|e| map_err(&e))
}

/// Size, free space, serial and file system of the volume that holds `path`.
pub(crate) fn volume(path: &str) -> Result<VolumeInfo, FsError> {
    let w = Wide::path(path);
    let mut root = vec![0u16; 1024];
    // SAFETY: `w` is NUL-terminated and `root` is a writable buffer of the given length.
    unsafe { GetVolumePathNameW(w.pcwstr(), &mut root) }.map_err(|e| map_err(&e))?;
    let root = Wide::new(&from_wide(&root));
    let (mut free, mut total) = (0u64, 0u64);
    // SAFETY: `root` is NUL-terminated and the out pointers are valid u64s.
    unsafe { GetDiskFreeSpaceExW(root.pcwstr(), Some(&mut free), Some(&mut total), None) }
        .map_err(|e| map_err(&e))?;
    let mut serial = 0u32;
    let mut fs_name = vec![0u16; 64];
    // SAFETY: `root` is NUL-terminated; `serial` and `fs_name` are valid writable buffers.
    unsafe {
        GetVolumeInformationW(
            root.pcwstr(),
            None,
            Some(&mut serial),
            None,
            None,
            Some(&mut fs_name),
        )
    }
    .map_err(|e| map_err(&e))?;
    Ok(VolumeInfo {
        total_bytes: total,
        free_bytes: free,
        serial: u64::from(serial),
        file_system: from_wide(&fs_name),
    })
}

/// The fixed drives of this PC, as `C:`.
pub(crate) fn fixed_drives() -> Vec<String> {
    let mut buf = vec![0u16; 512];
    // SAFETY: `buf` is a writable buffer; the return value is the number of units written.
    let n = unsafe { GetLogicalDriveStringsW(Some(&mut buf)) } as usize;
    buf.truncate(n.min(buf.len()));
    buf.split(|&c| c == 0)
        .filter(|s| !s.is_empty())
        .map(String::from_utf16_lossy)
        .filter(|root| {
            let w = Wide::new(root);
            // SAFETY: `w` is a valid NUL-terminated wide string.
            unsafe { GetDriveTypeW(w.pcwstr()) == DRIVE_FIXED }
        })
        .map(|root| root.trim_end_matches('\\').to_string())
        .collect()
}

//! Cloud Files API: is a placeholder fully uploaded?

use unhoard_core::fs::FsError;
use windows::Win32::Storage::CloudFilters::{
    CF_IN_SYNC_STATE_IN_SYNC, CF_PLACEHOLDER_BASIC_INFO, CF_PLACEHOLDER_INFO_BASIC,
    CfGetPlaceholderInfo,
};
use windows::Win32::Storage::FileSystem::{
    FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
};

use super::files::open;
use super::map_err;

/// `ERROR_NOT_A_CLOUD_FILE`: the file is not a placeholder (not uploaded yet).
const NOT_A_CLOUD_FILE: u32 = 0x8007_0178;
/// `ERROR_MORE_DATA`: the basic part was filled; the identity blob did not fit.
const MORE_DATA: u32 = 0x8007_00EA;

/// True when the placeholder at `path` is in sync with the cloud. The file is opened for
/// attributes only, which never downloads its data.
pub(crate) fn in_sync(path: &str) -> Result<bool, FsError> {
    let h = open(
        path,
        FILE_READ_ATTRIBUTES,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        true,
    )?;
    let mut buf = vec![0u64; 128];
    let len = (buf.len() * 8) as u32;
    // SAFETY: `buf` is writable, 8-byte aligned and `len` bytes long.
    let r = unsafe {
        CfGetPlaceholderInfo(
            h.0,
            CF_PLACEHOLDER_INFO_BASIC,
            buf.as_mut_ptr().cast(),
            len,
            None,
        )
    };
    if let Err(e) = r {
        let code = e.code().0 as u32;
        if code == NOT_A_CLOUD_FILE {
            return Ok(false);
        }
        if code != MORE_DATA {
            return Err(map_err(&e));
        }
    }
    // SAFETY: on success (or MORE_DATA) the buffer starts with a CF_PLACEHOLDER_BASIC_INFO.
    let info: CF_PLACEHOLDER_BASIC_INFO = unsafe { std::ptr::read_unaligned(buf.as_ptr().cast()) };
    Ok(info.InSyncState == CF_IN_SYNC_STATE_IN_SYNC)
}

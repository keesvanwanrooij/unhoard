//! Cloud sync roots registered with Windows, read from the registry.

use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
};
use windows::core::PWSTR;

use super::{Wide, from_wide};

const SYNC_ROOT_MANAGER: &str =
    r"SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\SyncRootManager";

/// Reads a string value; `None` if it does not exist.
fn string_value(root: HKEY, subkey: &str, name: &str) -> Option<String> {
    let (k, n) = (Wide::new(subkey), Wide::new(name));
    let mut len = 0u32;
    // SAFETY: a size query; both strings are NUL-terminated.
    let r = unsafe {
        RegGetValueW(
            root,
            k.pcwstr(),
            n.pcwstr(),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            None,
            Some(&mut len),
        )
    };
    if r != ERROR_SUCCESS || len == 0 {
        return None;
    }
    let mut buf = vec![0u16; (len as usize).div_ceil(2) + 1];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY: `buf` is writable for `bytes` bytes.
    let r = unsafe {
        RegGetValueW(
            root,
            k.pcwstr(),
            n.pcwstr(),
            RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    (r == ERROR_SUCCESS)
        .then(|| from_wide(&buf))
        .filter(|s| !s.is_empty())
}

/// The sync roots of the current user: every provider's `UserSyncRoots` value for the
/// user's SID, plus the OneDrive personal folder.
pub(crate) fn sync_roots(user_sid: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut key = HKEY::default();
    let base = Wide::new(SYNC_ROOT_MANAGER);
    // SAFETY: `base` is NUL-terminated and `key` is a valid out pointer.
    if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, base.pcwstr(), None, KEY_READ, &mut key) }
        == ERROR_SUCCESS
    {
        let mut index = 0;
        loop {
            let mut name = vec![0u16; 512];
            let mut len = name.len() as u32;
            // SAFETY: `name` is writable for `len` units.
            let r = unsafe {
                RegEnumKeyExW(
                    key,
                    index,
                    Some(PWSTR(name.as_mut_ptr())),
                    &mut len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if r != ERROR_SUCCESS {
                break;
            }
            index += 1;
            let provider = from_wide(&name);
            if let Some(path) = string_value(
                HKEY_LOCAL_MACHINE,
                &format!(r"{SYNC_ROOT_MANAGER}\{provider}\UserSyncRoots"),
                user_sid,
            ) {
                out.push(path);
            }
        }
        // SAFETY: `key` was opened above.
        let _closed = unsafe { RegCloseKey(key) };
    }
    if let Some(od) = string_value(
        HKEY_CURRENT_USER,
        r"Software\Microsoft\OneDrive\Accounts\Personal",
        "UserFolder",
    ) {
        out.push(od);
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.trim_end_matches('\\').to_lowercase()));
    out
}

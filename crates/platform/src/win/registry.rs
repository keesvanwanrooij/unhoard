//! What Unhoard reads from the registry: cloud sync roots, Windows' list of installed
//! programs, the program history (UserAssist) and Steam's folder. Read-only, with limits
//! on the number and size of entries.

use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_BINARY, REG_VALUE_TYPE,
    RRF_RT_REG_DWORD, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ, RegCloseKey, RegEnumKeyExW,
    RegEnumValueW, RegGetValueW, RegOpenKeyExW,
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

/// The most subkeys or values read from one key.
const MAX_ENTRIES: u32 = 4096;
/// The largest binary value read.
const MAX_BINARY: usize = 4096;

/// Opens a key for reading; `None` if it does not exist.
fn open(root: HKEY, subkey: &str) -> Option<HKEY> {
    let mut key = HKEY::default();
    let w = Wide::new(subkey);
    // SAFETY: `w` is NUL-terminated and `key` is a valid out pointer.
    (unsafe { RegOpenKeyExW(root, w.pcwstr(), None, KEY_READ, &mut key) } == ERROR_SUCCESS)
        .then_some(key)
}

fn close(key: HKEY) {
    // SAFETY: `key` was opened by `open`.
    let _closed = unsafe { RegCloseKey(key) };
}

/// The names of the subkeys of `subkey`.
fn subkeys(root: HKEY, subkey: &str) -> Vec<String> {
    let Some(key) = open(root, subkey) else {
        return vec![];
    };
    let mut out = vec![];
    for index in 0..MAX_ENTRIES {
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
        out.push(from_wide(&name));
    }
    close(key);
    out
}

/// Reads a DWORD value; `None` if it does not exist.
fn dword_value(root: HKEY, subkey: &str, name: &str) -> Option<u32> {
    let (k, n) = (Wide::new(subkey), Wide::new(name));
    let mut value = 0u32;
    let mut len = size_of::<u32>() as u32;
    // SAFETY: `value` is writable for `len` bytes; both strings are NUL-terminated.
    let r = unsafe {
        RegGetValueW(
            root,
            k.pcwstr(),
            n.pcwstr(),
            RRF_RT_REG_DWORD,
            None,
            Some((&raw mut value).cast()),
            Some(&mut len),
        )
    };
    (r == ERROR_SUCCESS).then_some(value)
}

/// Every binary value of `subkey`: its name and data, each data at most 4 KiB.
fn binary_values(root: HKEY, subkey: &str) -> Vec<(String, Vec<u8>)> {
    let Some(key) = open(root, subkey) else {
        return vec![];
    };
    let mut out = vec![];
    for index in 0..MAX_ENTRIES {
        let mut name = vec![0u16; 1024];
        let mut name_len = name.len() as u32;
        let mut data = vec![0u8; MAX_BINARY];
        let mut data_len = data.len() as u32;
        let mut kind = REG_VALUE_TYPE::default();
        // SAFETY: `name` and `data` are writable for the lengths passed.
        let r = unsafe {
            RegEnumValueW(
                key,
                index,
                Some(PWSTR(name.as_mut_ptr())),
                &mut name_len,
                None,
                Some(&raw mut kind.0),
                Some(data.as_mut_ptr()),
                Some(&mut data_len),
            )
        };
        if r == ERROR_NO_MORE_ITEMS {
            break;
        }
        // A value too large for the buffer is skipped, not read in part.
        if r != ERROR_SUCCESS || kind != REG_BINARY {
            continue;
        }
        data.truncate(data_len as usize);
        out.push((from_wide(&name), data));
    }
    close(key);
    out
}

/// One entry of Windows' list of installed programs, as raw registry values.
pub(crate) struct UninstallEntry {
    pub(crate) name: String,
    pub(crate) publisher: Option<String>,
    pub(crate) install_location: String,
    pub(crate) install_date: Option<String>,
    pub(crate) estimated_kib: Option<u32>,
}

/// The installed programs with an install folder, per machine (64- and 32-bit) and per
/// user. Entries Windows hides from Settings › Apps (`SystemComponent`) are left out.
pub(crate) fn installed_programs() -> Vec<UninstallEntry> {
    const KEYS: [(bool, &str); 3] = [
        (true, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (
            true,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            false,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];
    let mut out = vec![];
    for (machine, base) in KEYS {
        let root = if machine {
            HKEY_LOCAL_MACHINE
        } else {
            HKEY_CURRENT_USER
        };
        for sub in subkeys(root, base) {
            let key = format!(r"{base}\{sub}");
            if dword_value(root, &key, "SystemComponent") == Some(1) {
                continue;
            }
            let (Some(name), Some(location)) = (
                string_value(root, &key, "DisplayName"),
                string_value(root, &key, "InstallLocation"),
            ) else {
                continue;
            };
            let location = location.trim().trim_matches('"').to_string();
            if location.len() < 4 || location.len() > 1024 {
                continue;
            }
            out.push(UninstallEntry {
                name: name.chars().take(256).collect(),
                publisher: string_value(root, &key, "Publisher")
                    .map(|p| p.chars().take(256).collect()),
                install_location: location,
                install_date: string_value(root, &key, "InstallDate")
                    .filter(|d| d.len() == 8 && d.bytes().all(|b| b.is_ascii_digit())),
                estimated_kib: dword_value(root, &key, "EstimatedSize"),
            });
        }
    }
    out
}

/// Windows' program history: every UserAssist record of the current user, with its
/// still ROT13-encoded name.
pub(crate) fn user_assist() -> Vec<(String, Vec<u8>)> {
    const BASE: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\UserAssist";
    subkeys(HKEY_CURRENT_USER, BASE)
        .into_iter()
        .flat_map(|guid| binary_values(HKEY_CURRENT_USER, &format!(r"{BASE}\{guid}\Count")))
        .collect()
}

/// Steam's install folder, as Steam records it for the current user.
pub(crate) fn steam_path() -> Option<String> {
    string_value(HKEY_CURRENT_USER, r"Software\Valve\Steam", "SteamPath")
        .map(|p| p.replace('/', "\\"))
}

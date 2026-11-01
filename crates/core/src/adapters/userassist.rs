//! Windows' program history (the UserAssist registry key): for programs started from
//! the Start menu or Explorer, Windows keeps a ROT13-encoded path and a 72-byte record
//! whose last eight bytes before the tail hold the time of the last start.

use std::collections::BTreeMap;

use crate::time::filetime_to_unix;

/// Offset of the last-start FILETIME in a UserAssist record.
const LAST_RUN_OFFSET: usize = 60;
/// Size of a UserAssist record (Windows 7 and later).
const RECORD_LEN: usize = 72;

/// Decodes (or encodes) ROT13: letters shift by 13, everything else stays.
pub fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => char::from((c as u8 - b'a' + 13) % 26 + b'a'),
            'A'..='Z' => char::from((c as u8 - b'A' + 13) % 26 + b'A'),
            _ => c,
        })
        .collect()
}

/// The time of the last start in a record, in seconds since 1970.
pub fn last_run(data: &[u8]) -> Option<i64> {
    if data.len() < RECORD_LEN {
        return None;
    }
    let bytes: [u8; 8] = data
        .get(LAST_RUN_OFFSET..LAST_RUN_OFFSET + 8)?
        .try_into()
        .ok()?;
    let ft = u64::from_le_bytes(bytes);
    (ft > 0).then(|| filetime_to_unix(ft))
}

/// The full path of the program a decoded value name points to. Names start with a drive
/// or with a known-folder GUID, which `known` maps to its path. Only `.exe` files count:
/// shortcuts and app IDs do not name a folder.
pub fn resolve(name: &str, known: &BTreeMap<String, String>) -> Option<String> {
    if !name.to_ascii_lowercase().ends_with(".exe") {
        return None;
    }
    if let Some(rest) = name.strip_prefix('{') {
        let (guid, tail) = rest.split_once('}')?;
        let base = known.get(&format!("{{{}}}", guid.to_ascii_uppercase()))?;
        return Some(format!("{}{tail}", base.trim_end_matches('\\')));
    }
    let b = name.as_bytes();
    (b.len() > 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\')
        .then(|| name.to_string())
}

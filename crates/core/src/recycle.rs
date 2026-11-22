//! The Windows Recycle Bin: what each deleted item was. Windows keeps a pair per item in
//! `X:\$Recycle.Bin\<SID>`: `$R…` holds the data under a meaningless name, `$I…` holds the
//! original path, the size and the deletion time. This module reads the `$I` format.

use serde::{Deserialize, Serialize};

use crate::time::filetime_to_unix;

/// One item in the Recycle Bin, as its `$I` file describes it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecycledItem {
    /// Where it was before it was deleted.
    pub original: String,
    /// Its size in bytes (a folder: all its files).
    pub size: u64,
    /// When it was deleted, in Unix seconds.
    pub deleted_at: i64,
}

/// The longest path a `$I` file may hold, in UTF-16 units.
const MAX_PATH_UNITS: usize = 32_768;

/// Reads a `$I` file: version 2 (Windows 10 and later) holds the path length, version 1
/// (Windows Vista to 8.1) a fixed 260-unit path. `None` for anything else.
pub fn parse_info(bytes: &[u8]) -> Option<RecycledItem> {
    let u64_at = |at: usize| -> Option<u64> {
        Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
    };
    let version = u64_at(0)?;
    let size = u64_at(8)?;
    let filetime = u64_at(16)?;
    let (start, units) = match version {
        1 => (24, 260),
        2 => {
            let n = u32::from_le_bytes(bytes.get(24..28)?.try_into().ok()?) as usize;
            if n == 0 || n > MAX_PATH_UNITS {
                return None;
            }
            (28, n)
        }
        _ => return None,
    };
    let raw = bytes.get(start..start + units * 2)?;
    let wide: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .take_while(|u| *u != 0)
        .collect();
    let original = String::from_utf16(&wide).ok()?;
    if original.is_empty() {
        return None;
    }
    Some(RecycledItem {
        original,
        size,
        deleted_at: filetime_to_unix(filetime),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test data is built here")]

    use super::*;

    /// A version 2 `$I` file for `path`.
    fn v2(path: &str, size: u64, filetime: u64) -> Vec<u8> {
        let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
        let mut b = vec![];
        b.extend_from_slice(&2u64.to_le_bytes());
        b.extend_from_slice(&size.to_le_bytes());
        b.extend_from_slice(&filetime.to_le_bytes());
        b.extend_from_slice(&u32::try_from(wide.len()).unwrap().to_le_bytes());
        for u in wide {
            b.extend_from_slice(&u.to_le_bytes());
        }
        b
    }

    const EPOCH: u64 = 116_444_736_000_000_000;

    #[test]
    fn a_version_2_info_file_gives_name_size_and_date() {
        let b = v2(
            r"C:\Users\u\Downloads\model.safetensors",
            12_345_678_901,
            EPOCH + 10_000_000 * 86_400,
        );
        let item = parse_info(&b).unwrap();
        assert_eq!(item.original, r"C:\Users\u\Downloads\model.safetensors");
        assert_eq!(item.size, 12_345_678_901, "sizes above 4 GB stay right");
        assert_eq!(item.deleted_at, 86_400);
    }

    #[test]
    fn a_version_1_info_file_has_a_fixed_path_field() {
        let mut b = vec![];
        b.extend_from_slice(&1u64.to_le_bytes());
        b.extend_from_slice(&42u64.to_le_bytes());
        b.extend_from_slice(&EPOCH.to_le_bytes());
        let mut path: Vec<u16> = r"D:\old.txt".encode_utf16().collect();
        path.resize(260, 0);
        for u in path {
            b.extend_from_slice(&u.to_le_bytes());
        }
        let item = parse_info(&b).unwrap();
        assert_eq!(item.original, r"D:\old.txt");
        assert_eq!((item.size, item.deleted_at), (42, 0));
    }

    #[test]
    fn broken_info_files_are_ignored() {
        assert_eq!(parse_info(&[]), None);
        assert_eq!(parse_info(&[3, 0, 0, 0, 0, 0, 0, 0]), None);
        let mut b = v2(r"C:\x", 1, EPOCH);
        b.truncate(30);
        assert_eq!(parse_info(&b), None, "a path cut short");
        let mut huge = v2(r"C:\x", 1, EPOCH);
        huge[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(parse_info(&huge), None, "an absurd length");
    }
}

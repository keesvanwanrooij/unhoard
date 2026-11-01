//! Well-known locations of this PC and path helpers.
//!
//! The platform layer reads the environment, the Known Folders and the cloud sync roots
//! once; the core only receives the resulting strings. All comparisons are
//! case-insensitive and component-aware, so `C:\Program Files` never matches
//! `C:\Program Files Extra`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::tree::Category;

/// The name of the quarantine folder, the same in every language.
pub const QUARANTINE_DIR: &str = "Unhoard Quarantine";

/// The locations Unhoard needs to know.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Roots {
    /// Environment variables, keys in upper case (`WINDIR`, `LOCALAPPDATA`, …).
    pub env: BTreeMap<String, String>,
    /// The user's Downloads folder (Known Folder).
    pub downloads: Option<String>,
    /// Cloud sync roots on this PC.
    pub sync_roots: Vec<String>,
}

impl Roots {
    /// Builds the roots from environment variables, the Downloads folder and sync roots.
    pub fn new(
        env: BTreeMap<String, String>,
        downloads: Option<String>,
        sync_roots: Vec<String>,
    ) -> Self {
        let env = env
            .into_iter()
            .map(|(k, v)| (k.to_uppercase(), trim(&v)))
            .collect();
        Self {
            env,
            downloads: downloads.map(|d| trim(&d)),
            sync_roots: sync_roots.iter().map(|s| trim(s)).collect(),
        }
    }

    /// An environment variable. `DOWNLOADS` is the Downloads Known Folder, which has no
    /// environment variable of its own.
    pub fn var(&self, name: &str) -> Option<&str> {
        if name.eq_ignore_ascii_case("DOWNLOADS") {
            return self.downloads.as_deref();
        }
        self.env.get(&name.to_uppercase()).map(String::as_str)
    }

    /// The Windows folder.
    pub fn windows(&self) -> Option<&str> {
        self.var("WINDIR").or_else(|| self.var("SYSTEMROOT"))
    }

    /// `Program Files` and `Program Files (x86)`.
    pub fn program_files(&self) -> Vec<&str> {
        ["PROGRAMFILES", "PROGRAMFILES(X86)", "PROGRAMW6432"]
            .iter()
            .filter_map(|k| self.var(k))
            .collect()
    }

    /// `ProgramData`.
    pub fn program_data(&self) -> Option<&str> {
        self.var("PROGRAMDATA")
    }

    /// The current user's profile folder.
    pub fn profile(&self) -> Option<&str> {
        self.var("USERPROFILE")
    }

    /// The folder that holds all profiles, usually `C:\Users`.
    pub fn users(&self) -> Option<String> {
        self.profile().and_then(parent_of)
    }

    /// The sync root that holds `path`, if any.
    pub fn sync_root_of(&self, path: &str) -> Option<&str> {
        self.sync_roots
            .iter()
            .map(String::as_str)
            .find(|r| is_within(path, r))
    }

    /// The kind of content a folder gets from where it is, if its location says so.
    pub fn location_category(&self, dir: &str) -> Option<Category> {
        if self.sync_root_of(dir).is_some() {
            return Some(Category::Cloud);
        }
        if self.windows().is_some_and(|w| is_within(dir, w)) {
            return Some(Category::System);
        }
        if self.program_files().iter().any(|p| is_within(dir, p)) {
            return Some(Category::Apps);
        }
        if self.downloads.as_deref().is_some_and(|d| is_within(dir, d)) {
            return Some(Category::Downloads);
        }
        None
    }

    /// Where an item at `path` goes when it is quarantined: the cloud quarantine of its
    /// sync root, or the quarantine folder of its drive.
    pub fn quarantine_root_for(&self, path: &str) -> Option<String> {
        if let Some(sr) = self.sync_root_of(path) {
            return Some(join(sr, QUARANTINE_DIR));
        }
        drive_of(path).map(|d| format!("{d}\\{QUARANTINE_DIR}"))
    }

    /// True if `path` lies inside a quarantine root (local or cloud).
    pub fn in_quarantine(&self, path: &str) -> bool {
        if let Some(d) = drive_of(path)
            && is_within(path, &format!("{d}\\{QUARANTINE_DIR}"))
        {
            return true;
        }
        self.sync_roots
            .iter()
            .any(|s| is_within(path, &join(s, QUARANTINE_DIR)))
    }
}

fn trim(p: &str) -> String {
    let p = p.replace('/', "\\");
    p.trim_end_matches('\\').to_string()
}

fn norm(p: &str) -> String {
    p.replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

/// True if `path` equals `base` or lies inside it.
pub fn is_within(path: &str, base: &str) -> bool {
    let (p, b) = (norm(path), norm(base));
    p == b || p.starts_with(&format!("{b}\\"))
}

/// True if two paths name the same location.
pub fn same_path(a: &str, b: &str) -> bool {
    norm(a) == norm(b)
}

/// The drive of a path, `C:`.
pub fn drive_of(path: &str) -> Option<String> {
    let b = path.as_bytes();
    (b.len() >= 2 && b[1] == b':' && b[0].is_ascii_alphabetic()).then(|| path[..2].to_uppercase())
}

/// The parent folder of a path; `None` for a drive root.
pub fn parent_of(path: &str) -> Option<String> {
    let t = path.trim_end_matches('\\');
    let i = t.rfind('\\')?;
    let p = &t[..i];
    Some(if p.len() == 2 && p.ends_with(':') {
        format!("{p}\\")
    } else {
        p.to_string()
    })
}

/// The last component of a path.
pub fn file_name(path: &str) -> &str {
    path.trim_end_matches('\\')
        .rsplit('\\')
        .next()
        .unwrap_or("")
}

/// Joins a folder and a name with one backslash.
pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('\\') {
        format!("{dir}{name}")
    } else {
        format!("{dir}\\{name}")
    }
}

/// Splits a file name into stem and extension (`photo.jpg` → `photo`, `.jpg`).
pub fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn within_is_component_aware() {
        assert!(is_within(r"C:\Program Files\App", r"C:\program files"));
        assert!(!is_within(r"C:\Program Files Extra", r"C:\Program Files"));
        assert!(is_within(r"C:\", r"C:\"));
    }

    #[test]
    fn helpers() {
        assert_eq!(drive_of(r"d:\x").as_deref(), Some("D:"));
        assert_eq!(parent_of(r"C:\a").as_deref(), Some(r"C:\"));
        assert_eq!(parent_of(r"C:\"), None);
        assert_eq!(split_ext("a.tar.gz"), ("a.tar", ".gz"));
        assert_eq!(split_ext(".hidden"), (".hidden", ""));
    }
}

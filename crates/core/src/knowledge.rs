//! What Unhoard knows about this PC beyond the file tree: the installed programs, when
//! programs and games were last used, and the records of apps with adapters (Steam).
//!
//! The platform layer gathers it before a scan; the core only looks things up, so the
//! descriptions of items stay testable without Windows. Every value here came from the
//! registry or an app's files and is treated as data: paths are only compared, never
//! opened.

use serde::{Deserialize, Serialize};

use crate::time::filetime_to_unix;

/// Where a "last used" date comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UseSource {
    /// A launcher's "last played" (Steam).
    Played,
    /// The last start of a program, from Windows' program history.
    Started,
    /// The newest change of any file inside.
    Changed,
}

/// When an item was last used, and how Unhoard knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LastUse {
    /// Seconds since 1970.
    pub at: i64,
    /// The source of the date.
    pub source: UseSource,
}

/// An installed program from Windows' list (the `Uninstall` registry keys).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Program {
    /// Its display name.
    pub name: String,
    /// Its publisher.
    pub publisher: Option<String>,
    /// The folder it was installed to.
    pub install_location: String,
    /// The install date as Windows records it (`YYYYMMDD`).
    pub install_date: Option<String>,
    /// The size the installer reported.
    pub estimated_bytes: Option<u64>,
}

/// A recorded use of a file or folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    /// The program file or folder that was used.
    pub path: String,
    /// When, in seconds since 1970.
    pub at: i64,
    /// The source of the record.
    pub source: UseSource,
}

/// A game Steam has installed, from its `appmanifest_*.acf`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamGame {
    /// Steam's app ID.
    pub app_id: u32,
    /// The game's name.
    pub name: String,
    /// The game's folder.
    pub install_dir: String,
    /// The size Steam reports.
    pub size_on_disk: u64,
    /// When it was last played, in seconds since 1970.
    pub last_played: Option<i64>,
}

/// One Steam library: its `steamapps\common` folder and the games Steam lists in it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SteamLibrary {
    /// The `steamapps\common` folder.
    pub common: String,
    /// The games with a manifest in this library.
    pub games: Vec<SteamGame>,
}

/// What Steam knows about a folder in a library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SteamInfo {
    /// An installed game.
    #[serde(rename_all = "camelCase")]
    Game {
        /// Steam's app ID.
        app_id: u32,
        /// The game's name.
        name: String,
        /// When it was last played.
        last_played: Option<i64>,
    },
    /// A game folder without a manifest: what an uninstalled game left behind.
    Leftover,
}

/// Everything gathered about this PC besides the files.
#[derive(Clone, Debug, Default)]
pub struct Knowledge {
    /// Installed programs.
    pub programs: Vec<Program>,
    /// Recorded uses of programs.
    pub activity: Vec<Activity>,
    /// Steam libraries.
    pub steam: Vec<SteamLibrary>,
}

/// Lower case with backslashes and no trailing separator, the form paths compare in.
fn key(path: &str) -> String {
    path.replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

/// True when `path` is `dir` or inside it (both in [`key`] form).
fn within(path: &str, dir: &str) -> bool {
    path == dir || (path.starts_with(dir) && path.as_bytes().get(dir.len()) == Some(&b'\\'))
}

impl Knowledge {
    /// The installed program whose install folder is exactly `path`.
    pub fn program_at(&self, path: &str) -> Option<&Program> {
        let k = key(path);
        self.programs
            .iter()
            .find(|p| !p.install_location.trim().is_empty() && key(&p.install_location) == k)
    }

    /// True when `path` is the `steamapps\common` folder of a known library.
    pub fn is_steam_common(&self, path: &str) -> bool {
        let k = key(path);
        self.steam.iter().any(|l| key(&l.common) == k)
    }

    /// What Steam knows about `path`, if it is a folder directly in a library.
    pub fn steam_info(&self, path: &str) -> Option<SteamInfo> {
        let k = key(path);
        let (parent, _) = k.rsplit_once('\\')?;
        let lib = self.steam.iter().find(|l| key(&l.common) == parent)?;
        Some(match lib.games.iter().find(|g| key(&g.install_dir) == k) {
            Some(g) => SteamInfo::Game {
                app_id: g.app_id,
                name: g.name.clone(),
                last_played: g.last_played,
            },
            None => SteamInfo::Leftover,
        })
    }

    /// When `path` was last used: Steam's last played for a game, the latest recorded start
    /// of a program inside it, else the newest change inside (`changed`, a file time).
    pub fn last_use(&self, path: &str, changed: u64) -> Option<LastUse> {
        if let Some(SteamInfo::Game {
            last_played: Some(at),
            ..
        }) = self.steam_info(path)
        {
            return Some(LastUse {
                at,
                source: UseSource::Played,
            });
        }
        let k = key(path);
        let recorded = self
            .activity
            .iter()
            .filter(|a| within(&key(&a.path), &k))
            .max_by_key(|a| a.at);
        if let Some(a) = recorded {
            return Some(LastUse {
                at: a.at,
                source: a.source,
            });
        }
        (changed > 0).then(|| LastUse {
            at: filetime_to_unix(changed),
            source: UseSource::Changed,
        })
    }
}

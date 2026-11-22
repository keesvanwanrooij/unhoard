//! Leftovers of uninstalled apps (AC4.14 to AC4.16).
//!
//! A folder directly in `AppData\Local`, `AppData\Roaming`, `AppData\LocalLow` or
//! `ProgramData` is a leftover on one of two levels:
//!
//! - **Gone app** (Likely safe): its name matches a program that ran on this PC and whose
//!   executable no longer exists, from Windows' own program history.
//! - **Unclaimed** (Review): no installed program, rule or definition claims it, and nothing
//!   in it changed for 180 days.
//!
//! A folder that matches an installed program, a Windows folder or a keep definition is
//! never a leftover. Names compare without case, spaces, punctuation and version numbers.

use serde::{Deserialize, Serialize};

use crate::knowledge::Knowledge;
use crate::roots::{Roots, file_name, parent_of, same_path};
use crate::time::filetime_to_unix;
use crate::tree::{NodeId, NodeKind, Tree};

/// Days without a change before an unclaimed folder counts as a leftover.
pub const UNCLAIMED_DAYS: i64 = 180;
/// Shortest normalised name that may match by containment; shorter names must be equal.
const MIN_CONTAINS: usize = 4;

/// Folders in these places that belong to Windows itself, never leftovers.
const WINDOWS_FOLDERS: [&str; 24] = [
    "microsoft",
    "packages",
    "publishers",
    "temp",
    "programs",
    "virtualstore",
    "connecteddevicesplatform",
    "comms",
    "crashdumps",
    "d3dscache",
    "placeholdertilelogofolder",
    "squirreltemp",
    "packagecache",
    "elevateddiagnostics",
    "history",
    "applicationdata",
    "regid",
    "ssh",
    "usoshared",
    "usoprivate",
    "windowsholographicdevices",
    "softwaredistribution",
    "desktop",
    "documents",
];

/// Why a folder is a leftover.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Leftover {
    /// It belongs to a program that is gone.
    #[serde(rename_all = "camelCase")]
    GoneApp {
        /// The program's folder or file name.
        name: String,
        /// When it last ran, in seconds since 1970.
        last_run: i64,
    },
    /// No installed program claims it and it has not changed for a long time.
    #[serde(rename_all = "camelCase")]
    Unclaimed {
        /// Days since anything in it changed.
        unchanged_days: i64,
    },
}

/// A name in the form names compare in: lower case letters and digits only, without a
/// trailing version (`Some App 3`, `Tool v2.1`, `Tool 2024` become `someapp`, `tool`).
fn norm(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '.')
        .filter(|w| !w.is_empty())
        .collect();
    while words.len() > 1
        && words.last().is_some_and(|w| {
            let w = w.trim_start_matches('v');
            !w.is_empty() && w.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
    {
        words.pop();
    }
    words
        .concat()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// True when two names stand for the same app.
pub fn same_app(a: &str, b: &str) -> bool {
    let (a, b) = (norm(a), norm(b));
    !a.is_empty() && a == b
}

/// True when `a` and `b` are the same app or one name contains the other (a long enough
/// name); used to keep anything an installed program might claim.
fn related(a: &str, b: &str) -> bool {
    let (a, b) = (norm(a), norm(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b
        || (a.len() >= MIN_CONTAINS && b.contains(&a))
        || (b.len() >= MIN_CONTAINS && a.contains(&b))
}

/// The places whose direct children may be leftovers.
pub fn leftover_parents(roots: &Roots) -> Vec<String> {
    let mut out = vec![];
    if let Some(l) = roots.var("LOCALAPPDATA") {
        out.push(l.to_string());
    }
    if let Some(r) = roots.var("APPDATA") {
        out.push(r.to_string());
    }
    if let Some(p) = roots.profile() {
        out.push(format!(r"{p}\AppData\LocalLow"));
    }
    if let Some(d) = roots.program_data() {
        out.push(d.to_string());
    }
    out
}

/// The names a program's records give it: its name, publisher and install folder.
fn program_names(k: &Knowledge) -> Vec<String> {
    k.programs
        .iter()
        .flat_map(|p| {
            let mut v = vec![p.name.clone()];
            if let Some(pb) = &p.publisher {
                v.push(pb.clone());
            }
            if !p.install_location.trim().is_empty() {
                v.push(file_name(p.install_location.trim_end_matches('\\')).to_string());
            }
            v
        })
        .collect()
}

/// The names a gone program goes by: its executable's folder (below Program Files or
/// AppData, its own folder) and the executable's name without extension.
fn gone_names(path: &str) -> Vec<String> {
    let mut out = vec![];
    if let Some(dir) = parent_of(path) {
        out.push(file_name(&dir).to_string());
    }
    let exe = file_name(path);
    out.push(crate::roots::split_ext(exe).0.to_string());
    out
}

/// Whether the folder `id` is a leftover. `kept` says a rule or definition claims it.
pub fn leftover_of(
    tree: &Tree,
    id: NodeId,
    k: &Knowledge,
    roots: &Roots,
    now: i64,
    kept: bool,
) -> Option<Leftover> {
    let node = tree.get(id)?;
    if node.kind != NodeKind::Folder || kept {
        return None;
    }
    let path = tree.path(id);
    let parent = parent_of(&path)?;
    if !leftover_parents(roots)
        .iter()
        .any(|p| same_path(p, &parent))
    {
        return None;
    }
    let name = node.name.as_ref();
    let n = norm(name);
    if n.is_empty() || WINDOWS_FOLDERS.iter().any(|w| n.starts_with(w)) {
        return None;
    }
    // Anything an installed program might claim stays.
    if program_names(k).iter().any(|p| related(p, name)) {
        return None;
    }
    let gone = k
        .gone
        .iter()
        .filter(|a| gone_names(&a.path).iter().any(|g| same_app(g, name)))
        .max_by_key(|a| a.at);
    if let Some(a) = gone {
        let label = parent_of(&a.path)
            .map(|d| file_name(&d).to_string())
            .unwrap_or_else(|| file_name(&a.path).to_string());
        return Some(Leftover::GoneApp {
            name: label,
            last_run: a.at,
        });
    }
    let changed = filetime_to_unix(node.newest);
    let days = (now - changed) / 86_400;
    (node.newest > 0 && days >= UNCLAIMED_DAYS).then_some(Leftover::Unclaimed {
        unchanged_days: days,
    })
}

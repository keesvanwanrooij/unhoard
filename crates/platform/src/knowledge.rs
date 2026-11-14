//! Gathers what Unhoard knows about this PC besides the files, before a scan: Windows' list
//! of installed programs, the program history, and Steam's libraries. Also reads Ollama's
//! model manifests on request. Everything is read-only; files go through [`WindowsFs`],
//! which never opens an online-only cloud file for its data (S2).

use std::collections::BTreeMap;

use unhoard_core::adapters::ollama::{self, ManifestFile, OllamaModel};
use unhoard_core::adapters::{steam, userassist};
use unhoard_core::fs::FileSystem;
use unhoard_core::knowledge::{Activity, Knowledge, Program, SteamLibrary, UseSource};
use unhoard_core::roots::{Roots, join};
use unhoard_core::time::filetime_to_unix;

use crate::WindowsFs;
use crate::win::registry;

/// The largest app record read (a Steam manifest, an Ollama manifest).
const MAX_RECORD: usize = 1 << 20;
/// The most Steam libraries and app manifests read.
const MAX_STEAM: usize = 4096;
/// How deep the Ollama manifest tree is walked.
const MAX_MANIFEST_DEPTH: usize = 6;

/// Everything known about this PC besides the files.
pub fn gather(roots: &Roots) -> Knowledge {
    let activity = activity(roots);
    // A recorded program whose executable is gone was uninstalled (or moved).
    let gone = activity
        .iter()
        .filter(|a| a.path.to_ascii_lowercase().ends_with(".exe"))
        .filter(|a| !std::path::Path::new(&a.path).exists())
        .cloned()
        .collect();
    Knowledge {
        programs: programs(),
        activity,
        gone,
        steam: steam_libraries(),
    }
}

fn programs() -> Vec<Program> {
    registry::installed_programs()
        .into_iter()
        .map(|e| Program {
            name: e.name,
            publisher: e.publisher,
            install_location: e.install_location,
            install_date: e.install_date,
            estimated_bytes: e.estimated_kib.map(|k| u64::from(k) * 1024),
        })
        .collect()
}

/// The known folders UserAssist names by GUID, mapped to this PC's paths.
fn known_folders(roots: &Roots) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    let mut put = |guid: &str, path: Option<String>| {
        if let Some(p) = path {
            m.insert(guid.to_string(), p);
        }
    };
    let pf = roots
        .var("PROGRAMW6432")
        .or_else(|| roots.var("PROGRAMFILES"))
        .map(str::to_string);
    let windows = roots.windows().map(str::to_string);
    put("{6D809377-6AF0-444B-8957-A3773F02200E}", pf.clone());
    put("{905E63B6-C1BF-494E-B29C-65B732D3D21A}", pf);
    put(
        "{7C5A40EF-A0FB-4BFC-874A-C0F2E0B9FA8E}",
        roots.var("PROGRAMFILES(X86)").map(str::to_string),
    );
    put("{F38BF404-1D43-42F2-9305-67DE0B28FC23}", windows.clone());
    put(
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}",
        windows.as_deref().map(|w| join(w, "System32")),
    );
    put(
        "{D65231B0-B2F1-4857-A4CE-A8E7C6EA7D27}",
        windows.as_deref().map(|w| join(w, "SysWOW64")),
    );
    m
}

fn activity(roots: &Roots) -> Vec<Activity> {
    let known = known_folders(roots);
    registry::user_assist()
        .into_iter()
        .filter_map(|(name, data)| {
            let path = userassist::resolve(&userassist::rot13(&name), &known)?;
            Some(Activity {
                path,
                at: userassist::last_run(&data)?,
                source: UseSource::Started,
            })
        })
        .collect()
}

/// Reads a small text file, never an online-only one.
fn read_text(path: &str) -> Option<String> {
    let bytes = WindowsFs.read_small(path, MAX_RECORD).ok()?;
    String::from_utf8(bytes).ok()
}

fn steam_libraries() -> Vec<SteamLibrary> {
    let Some(steam_dir) = registry::steam_path() else {
        return vec![];
    };
    let vdf = join(&join(&steam_dir, "steamapps"), "libraryfolders.vdf");
    let mut libraries = read_text(&vdf)
        .map(|t| steam::library_paths(&t))
        .unwrap_or_default();
    if libraries.is_empty() {
        libraries.push(steam_dir);
    }
    libraries
        .into_iter()
        .take(MAX_STEAM)
        .map(|lib| {
            let apps = join(&lib, "steamapps");
            let games = WindowsFs
                .read_dir(&apps)
                .unwrap_or_default()
                .into_iter()
                .filter(|e| {
                    let n = e.name.to_ascii_lowercase();
                    !e.is_dir && n.starts_with("appmanifest_") && n.ends_with(".acf")
                })
                .take(MAX_STEAM)
                .filter_map(|e| steam::parse_appmanifest(&read_text(&join(&apps, &e.name))?, &lib))
                .collect();
            SteamLibrary {
                common: join(&apps, "common"),
                games,
            }
        })
        .collect()
}

/// Ollama's model folder: `OLLAMA_MODELS`, else `.ollama\models` in the profile.
pub fn ollama_dir(roots: &Roots) -> Option<String> {
    roots.var("OLLAMA_MODELS").map(str::to_string).or_else(|| {
        roots
            .var("USERPROFILE")
            .map(|p| join(&join(p, ".ollama"), "models"))
    })
}

fn walk_manifests(dir: &str, relative: &str, depth: usize, out: &mut Vec<ManifestFile>) {
    if depth > MAX_MANIFEST_DEPTH || out.len() >= MAX_STEAM {
        return;
    }
    for e in WindowsFs.read_dir(dir).unwrap_or_default() {
        if e.is_link() {
            continue;
        }
        let path = join(dir, &e.name);
        let rel = if relative.is_empty() {
            e.name.clone()
        } else {
            format!("{relative}/{}", e.name)
        };
        if e.is_dir {
            walk_manifests(&path, &rel, depth + 1, out);
        } else if let Some(json) = read_text(&path) {
            out.push(ManifestFile {
                relative: rel,
                modified: filetime_to_unix(e.modified),
                json,
            });
        }
    }
}

/// The Ollama models on this PC, read from their manifests.
pub fn ollama_models(roots: &Roots) -> Vec<OllamaModel> {
    let Some(dir) = ollama_dir(roots) else {
        return vec![];
    };
    let mut manifests = vec![];
    walk_manifests(&join(&dir, "manifests"), "", 0, &mut manifests);
    ollama::models(&dir, &manifests)
}

//! The webview's own data folder (`%LOCALAPPDATA%\<identifier>\EBWebView`): Edge WebView2
//! downloads browser components (DRM, filter lists, speech) and keeps an HTTP cache there,
//! though Unhoard shows only its own built-in pages. The window starts with flags that stop
//! those downloads (`tauri.conf.json`); this removes what an earlier start left behind.

use std::path::Path;

/// Folders of the WebView2 profile that Unhoard never needs, relative to `EBWebView`.
pub const UNNEEDED: [&str; 7] = [
    "component_crx_cache",
    "extensions_crx_cache",
    "WidevineCdm",
    "Subresource Filter",
    "Speech Recognition",
    "hyphen-data",
    r"Default\Cache",
];

/// Removes the [`UNNEEDED`] folders inside `webview_dir` and returns the bytes freed. Runs
/// before the window opens, so no file is in use. A name that is a link, not a folder, is
/// left alone; nothing outside `webview_dir` is touched.
pub fn trim(webview_dir: &Path) -> u64 {
    let mut freed = 0;
    for name in UNNEEDED {
        let dir = webview_dir.join(name);
        let Ok(meta) = std::fs::symlink_metadata(&dir) else {
            continue;
        };
        if !meta.is_dir() || meta.file_type().is_symlink() {
            continue;
        }
        let size = folder_size(&dir);
        if std::fs::remove_dir_all(&dir).is_ok() {
            freed += size;
        }
    }
    freed
}

/// The size of the files in `dir`, without following links.
fn folder_size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.file_type().is_symlink() => 0,
            Ok(m) if m.is_dir() => folder_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test setup fails loudly on purpose")]

    use super::*;

    fn file(path: &Path, bytes: usize) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![1u8; bytes]).unwrap();
    }

    #[test]
    fn only_the_unneeded_webview_folders_are_removed() {
        let root = tempfile::tempdir().unwrap();
        let web = root.path().join("EBWebView");
        file(&web.join(r"WidevineCdm\4.10\cdm.dll"), 3000);
        file(&web.join(r"component_crx_cache\a.crx"), 2000);
        file(&web.join(r"Default\Cache\Cache_Data\data_0"), 1000);
        // Kept: the profile's own state and caches that speed up the start.
        file(&web.join(r"Default\Preferences"), 10);
        file(&web.join(r"Default\Code Cache\js\index"), 10);
        file(&web.join(r"Local State"), 10);
        // Outside the webview folder.
        file(&root.path().join(r"WidevineCdm\keep.txt"), 10);

        assert_eq!(trim(&web), 6000);
        assert!(!web.join("WidevineCdm").exists());
        assert!(!web.join("component_crx_cache").exists());
        assert!(!web.join(r"Default\Cache").exists());
        assert!(web.join(r"Default\Preferences").exists());
        assert!(web.join(r"Default\Code Cache\js\index").exists());
        assert!(web.join("Local State").exists());
        assert!(root.path().join(r"WidevineCdm\keep.txt").exists());
        // A second start finds nothing to do.
        assert_eq!(trim(&web), 0);
    }

    #[test]
    fn a_missing_webview_folder_is_fine() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(trim(&root.path().join("nope")), 0);
    }
}

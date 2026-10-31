//! Build script: generates the Tauri context and one permission per app command, so the
//! capability file can grant exactly these commands and nothing else.

const COMMANDS: &[&str] = &[
    "app_info",
    "settings_get",
    "settings_set",
    "scan_start",
    "scan_cancel",
    "scan_status",
    "view",
    "details",
    "suggestions",
    "quarantine_preview",
    "quarantine_move",
    "quarantine_state",
    "quarantine_restore",
    "quarantine_warning_shown",
    "quarantine_open",
    "show_in_explorer",
    "open_tool",
    "free_up_preview",
    "free_up_apply",
    "reveal_hidden",
    "photos_analyze",
    "photo_thumbnail",
    "cloud_roots",
];

fn main() {
    let attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));
    if let Err(e) = tauri_build::try_build(attributes) {
        eprintln!("tauri build failed: {e:#}");
        std::process::exit(1);
    }
}

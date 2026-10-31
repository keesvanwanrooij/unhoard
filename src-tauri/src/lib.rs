//! The Unhoard desktop app: builds the service from this PC's locations and exposes it to
//! the webview through a small, validated command surface.
#![forbid(unsafe_code)]

pub mod commands;
pub mod service;

use std::sync::Arc;

use tauri::Emitter;
use unhoard_core::photos::Features;
use unhoard_core::rules::Rules;
use unhoard_core::scan::ScanContext;
use unhoard_platform::WindowsFs;
use unhoard_platform::system::{quarantine_roots, system_roots};

use service::{MediaSource, Service};

/// Photo features from the Windows thumbnail API and property system.
struct WindowsMedia;

impl MediaSource for WindowsMedia {
    fn features(&self, path: &str, video: bool, in_cloud: bool) -> Features {
        unhoard_platform::media::features(path, video, in_cloud)
    }
}

/// Builds the service for this PC.
fn build_service() -> anyhow::Result<Service> {
    let roots = system_roots();
    let rules = Rules::builtin(&roots)?;
    let local = roots
        .var("LOCALAPPDATA")
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is not set"))?;
    let data = format!(r"{local}\Unhoard");
    std::fs::create_dir_all(&data)?;
    let qroots = quarantine_roots(&roots);
    let now = unhoard_core::fs::FileSystem::now(&WindowsFs);
    Ok(Service::new(
        Arc::new(WindowsFs),
        ScanContext { rules, roots, now },
        Arc::new(WindowsMedia),
        format!(r"{data}\activity.jsonl"),
        qroots,
        commands::readme,
    ))
}

/// Runs the app.
pub fn run() -> anyhow::Result<()> {
    let service = Arc::new(build_service()?);
    let maintenance = service.clone();
    tauri::Builder::default()
        .manage(service)
        .setup(move |app| {
            let handle = app.handle().clone();
            // Settle interrupted moves and run the 30-day purge, off the UI thread.
            std::thread::spawn(move || {
                let report = maintenance.startup_maintenance();
                let _ui_may_not_listen_yet = handle.emit("maintenance-done", report);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::settings_get,
            commands::settings_set,
            commands::scan_start,
            commands::scan_cancel,
            commands::scan_status,
            commands::view,
            commands::details,
            commands::suggestions,
            commands::quarantine_preview,
            commands::quarantine_move,
            commands::quarantine_state,
            commands::quarantine_restore,
            commands::quarantine_warning_shown,
            commands::quarantine_open,
            commands::show_in_explorer,
            commands::open_tool,
            commands::free_up_preview,
            commands::free_up_apply,
            commands::reveal_hidden,
            commands::photos_analyze,
            commands::photo_thumbnail,
            commands::cloud_roots,
        ])
        .run(tauri::generate_context!())?;
    Ok(())
}

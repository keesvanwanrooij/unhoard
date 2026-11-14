//! The Unhoard desktop app: builds the service from this PC's locations and exposes it to
//! the webview through a small, validated command surface.
#![forbid(unsafe_code)]

pub mod chat;
pub mod commands;
pub mod history;
pub mod service;
pub mod webview_data;

use std::sync::Arc;

use tauri::Emitter;
use unhoard_core::photos::Features;
use unhoard_core::rules::Rules;
use unhoard_core::scan::ScanContext;
use unhoard_platform::WindowsFs;
use unhoard_platform::system::{quarantine_roots, system_roots};

use history::HistoryStore;
use service::{MediaSource, Service};

/// Photo features from the Windows thumbnail API and property system.
struct WindowsMedia;

impl MediaSource for WindowsMedia {
    fn features(&self, path: &str, video: bool, in_cloud: bool) -> Features {
        unhoard_platform::media::features(path, video, in_cloud)
    }
}

/// The storage-history file in Unhoard's data folder.
fn history_store(data: &str) -> Arc<HistoryStore> {
    Arc::new(HistoryStore::new(
        std::path::Path::new(data)
            .join("insights")
            .join("history.jsonl"),
    ))
}

/// Unhoard's data folder, `%LOCALAPPDATA%\Unhoard`.
fn data_dir(roots: &unhoard_core::roots::Roots) -> anyhow::Result<String> {
    let local = roots
        .var("LOCALAPPDATA")
        .ok_or_else(|| anyhow::anyhow!("LOCALAPPDATA is not set"))?;
    let data = format!(r"{local}\Unhoard");
    std::fs::create_dir_all(&data)?;
    Ok(data)
}

/// Builds the service for this PC.
fn build_service() -> anyhow::Result<Service> {
    let roots = system_roots();
    let rules = Rules::builtin(&roots)?;
    let data = data_dir(&roots)?;
    let qroots = quarantine_roots(&roots);
    let now = unhoard_core::fs::FileSystem::now(&WindowsFs);
    Ok(Service::new(
        Arc::new(WindowsFs),
        ScanContext {
            rules,
            roots,
            now,
            knowledge: Default::default(),
        },
        Arc::new(WindowsMedia),
        format!(r"{data}\activity.jsonl"),
        qroots,
        commands::readme,
    )
    .with_knowledge(Arc::new(unhoard_platform::knowledge::gather))
    .with_history(history_store(&data)))
}

/// The daily measurement (`--measure`): waits for a better moment on battery saver or
/// while a full-screen app runs, lowers its own priority, scans the system drive with two
/// threads and writes a snapshot, unless the CPU budget runs out first.
pub fn measure() -> anyhow::Result<()> {
    use unhoard_platform::schedule;
    if !commands::settings_get().insights || schedule::should_wait() {
        return Ok(());
    }
    schedule::enter_background();
    // Two threads keep the measurement light; the pool is built on first use, after this.
    let _pool = rayon::ThreadPoolBuilder::new()
        .num_threads(2)
        .build_global();
    let service = Arc::new(build_service()?);
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let start = schedule::cpu_seconds();
    service
        .measure(&format!(r"{drive}\"), &|| {
            schedule::cpu_seconds() - start > schedule::CPU_BUDGET_SECONDS
        })
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    Ok(())
}

/// Brings the scheduled task in line with the setting, and measures now when the newest
/// snapshot is more than a day old. Runs off the UI thread.
fn sync_insights(service: &Service) {
    use unhoard_platform::schedule;
    if !commands::settings_get().insights {
        schedule::remove();
        return;
    }
    schedule::install();
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
    let newest = service
        .history_timeline(&format!(r"{drive}\"))
        .last()
        .map(|p| p.at);
    let now = unhoard_core::fs::FileSystem::now(&WindowsFs);
    if newest.is_none_or(|at| now - at > unhoard_core::history::DAY) {
        schedule::run_now();
    }
}

/// Runs the app.
pub fn run() -> anyhow::Result<()> {
    let context = tauri::generate_context!();
    // Before the window opens: drop browser components an earlier start downloaded.
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let dir = std::path::Path::new(&local)
            .join(&context.config().identifier)
            .join("EBWebView");
        let _freed = webview_data::trim(&dir);
    }
    let service = Arc::new(build_service()?);
    let maintenance = service.clone();
    tauri::Builder::default()
        .manage(service)
        .manage(Arc::new(commands::AdminState::default()))
        .manage(Arc::new(commands::OptimiseState::default()))
        .manage(Arc::new(commands::ChatState::default()))
        .setup(move |app| {
            let handle = app.handle().clone();
            // Settle interrupted moves, run the 30-day purge and set up the daily
            // measurement, off the UI thread.
            std::thread::spawn(move || {
                let report = maintenance.startup_maintenance();
                let _ui_may_not_listen_yet = handle.emit("maintenance-done", report);
                sync_insights(&maintenance);
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
            commands::matrix,
            commands::admin_start,
            commands::admin_read,
            commands::ollama_models,
            commands::docker_usage,
            commands::app_uninstall,
            commands::open_app,
            commands::history_timeline,
            commands::history_growth,
            commands::history_clear,
            commands::optimise_check,
            commands::optimise_updates,
            commands::startup_preview,
            commands::startup_set,
            commands::startup_undo,
            commands::chat_models,
            commands::chat_send,
            commands::chat_stop,
            commands::chat_setup,
            commands::chat_pull,
        ])
        .run(context)?;
    Ok(())
}

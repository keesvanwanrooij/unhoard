//! Tauri commands: thin, validated wrappers around the [`Service`].
//!
//! Every argument is typed and checked in Rust. Destructive commands take a scan
//! generation and item IDs, never paths. Drives must be fixed drives of this PC; tools
//! come from an allowlist. Heavy work runs on blocking worker threads so the window stays
//! responsive.

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use unhoard_core::adapters::docker::{self, DockerUsage};
use unhoard_core::adapters::ollama::OllamaModel;
use unhoard_core::cloud::FreeUpPlan;
use unhoard_core::fs::FsError;
use unhoard_core::hidden::{HiddenKind, hidden_kind};
use unhoard_core::optimise::{Finding, OptimiseData, StartupScope, findings};
use unhoard_core::quarantine::{ItemResult, Manifest, MovePlan, MoveReport};
use unhoard_core::rules::WindowsTool;
use unhoard_core::view::{MatrixPoint, Suggestion};
use unhoard_platform::ollama::ChatError;
use unhoard_platform::reveal::{HelperSession, RevealError, RevealedFolder, start_helper};
use unhoard_platform::tools::ToolError;
use unhoard_platform::{knowledge, media, optimise, system, tools};

use crate::chat;

use crate::service::{
    CloudRoot, CmdError, Details, GrowthView, Measured, MeasuredSummary, PhotoState,
    QuarantineState, Service, TimelinePoint, ViewResponse,
};

/// The shared service.
pub type Svc<'a> = State<'a, Arc<Service>>;

fn io(e: std::io::Error) -> CmdError {
    CmdError::Fs {
        error: FsError::Other {
            code: e.raw_os_error().unwrap_or(0) as u32,
        },
    }
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, CmdError> + Send + 'static,
) -> Result<T, CmdError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| CmdError::Fs {
            error: FsError::Other { code: 0 },
        })?
}

/// A drive for the drive picker.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveInfo {
    drive: String,
    total_bytes: u64,
    free_bytes: u64,
    file_system: String,
    system: bool,
}

/// What the UI needs at start.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    drives: Vec<DriveInfo>,
    sync_roots: Vec<String>,
    build: String,
}

/// Drives, sync roots and the build identifier.
#[tauri::command]
pub async fn app_info(svc: Svc<'_>) -> Result<AppInfo, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || {
        let system_drive = svc
            .context()
            .roots
            .var("SYSTEMDRIVE")
            .unwrap_or("C:")
            .to_uppercase();
        let drives = system::fixed_drives()
            .into_iter()
            .filter_map(|d| {
                let v = unhoard_core::fs::FileSystem::volume(
                    &unhoard_platform::WindowsFs,
                    &format!("{d}\\"),
                )
                .ok()?;
                Some(DriveInfo {
                    system: d == system_drive,
                    drive: d,
                    total_bytes: v.total_bytes,
                    free_bytes: v.free_bytes,
                    file_system: v.file_system,
                })
            })
            .collect();
        Ok(AppInfo {
            drives,
            sync_roots: svc.context().roots.sync_roots.clone(),
            build: env!("CARGO_PKG_VERSION").into(),
        })
    })
    .await
}

/// User settings, stored in `%APPDATA%\Unhoard\settings.json`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// `en`, `nl`, or `None` to follow Windows.
    pub language: Option<String>,
    /// `cells`, `treemap` or `matrix`.
    pub map_style: String,
    /// `living` or `calm`.
    pub motion: String,
    /// `onDisk` or `logical`.
    pub measure: String,
    /// `normal` or `quiet`.
    pub companion: String,
    /// Reading folders the user may not read: `ask`, `always` or `never`.
    pub admin_read: String,
    /// The first-run welcome was shown.
    pub first_run_done: bool,
    /// Background insights: the daily measurement for storage history (on by default).
    pub insights: bool,
    /// The AI chat through Ollama on this PC (off by default; when off, no connection).
    pub chat: bool,
    /// The chat model the user picked, or `None` for the recommended one.
    pub chat_model: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: None,
            map_style: "cells".into(),
            motion: "living".into(),
            measure: "onDisk".into(),
            companion: "normal".into(),
            admin_read: "ask".into(),
            first_run_done: false,
            insights: true,
            chat: false,
            chat_model: None,
        }
    }
}

impl Settings {
    fn valid(&self) -> bool {
        self.language
            .as_deref()
            .is_none_or(|l| l == "en" || l == "nl")
            && ["cells", "treemap", "matrix"].contains(&self.map_style.as_str())
            && ["living", "calm"].contains(&self.motion.as_str())
            && ["onDisk", "logical"].contains(&self.measure.as_str())
            && ["normal", "quiet"].contains(&self.companion.as_str())
            && ["ask", "always", "never"].contains(&self.admin_read.as_str())
            && self.chat_model.as_deref().is_none_or(valid_model)
    }
}

/// A model name as Ollama writes them: letters, digits and `.:_-/`, at most 200 long.
fn valid_model(m: &str) -> bool {
    !m.is_empty()
        && m.len() <= 200
        && m.chars()
            .all(|c| c.is_ascii_alphanumeric() || ".:_-/".contains(c))
}

fn settings_path() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|a| {
        std::path::Path::new(&a)
            .join("Unhoard")
            .join("settings.json")
    })
}

/// Reads the settings; defaults when there are none yet.
#[tauri::command]
pub fn settings_get() -> Settings {
    settings_path()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
        .filter(Settings::valid)
        .unwrap_or_default()
}

/// Saves the settings.
#[tauri::command]
pub fn settings_set(settings: Settings) -> Result<(), CmdError> {
    if !settings.valid() {
        return Err(CmdError::InvalidArgument {
            what: "settings".into(),
        });
    }
    let p = settings_path().ok_or(CmdError::InvalidArgument {
        what: "APPDATA".into(),
    })?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let bytes = serde_json::to_vec_pretty(&settings).map_err(|_| CmdError::InvalidArgument {
        what: "settings".into(),
    })?;
    let insights = settings.insights;
    std::fs::write(p, bytes).map_err(io)?;
    // The daily measurement's task follows the switch at once, off the UI thread.
    std::thread::spawn(move || {
        if insights {
            unhoard_platform::schedule::install();
        } else {
            unhoard_platform::schedule::remove();
        }
    });
    Ok(())
}

/// The startup switches made this session, so each can be undone by its index; the old
/// values never come back from the webview.
#[derive(Default)]
pub struct OptimiseState(Mutex<Vec<optimise::StartupChange>>);

/// Runs the check-up and returns its findings, except Windows Update (about a second of
/// CPU sampling).
#[tauri::command]
pub async fn optimise_check(svc: Svc<'_>) -> Result<Vec<Finding>, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || {
        let (free, total) = svc.system_space();
        let data = optimise::gather(&svc.context().roots, free, total);
        Ok(findings(&data))
    })
    .await
}

/// The Windows Update finding on its own: its search can take half a minute.
#[tauri::command]
pub async fn optimise_updates() -> Result<Vec<Finding>, CmdError> {
    blocking(|| {
        let now = unhoard_core::fs::FileSystem::now(&unhoard_platform::WindowsFs);
        let data = OptimiseData {
            updates: optimise::updates(now),
            ..OptimiseData::default()
        };
        Ok(findings(&data)
            .into_iter()
            .filter(|f| f.id == "updates")
            .collect())
    })
    .await
}

/// What switching a startup entry would change (a dry run), or the reason it cannot.
#[tauri::command]
pub async fn startup_preview(
    svc: Svc<'_>,
    scope: StartupScope,
    name: String,
    enabled: bool,
) -> Result<Result<optimise::StartupChange, optimise::StartupError>, CmdError> {
    check_name(&name)?;
    let svc = svc.inner().clone();
    blocking(move || {
        Ok(optimise::startup_set(
            &svc.context().roots,
            scope,
            &name,
            enabled,
            0,
            true,
        ))
    })
    .await
}

/// Switches a startup entry of the current user on or off (High: one registry value, as
/// Task Manager writes it). Returns the index to undo it with.
#[tauri::command]
pub async fn startup_set(
    svc: Svc<'_>,
    state: State<'_, Arc<OptimiseState>>,
    scope: StartupScope,
    name: String,
    enabled: bool,
) -> Result<Result<usize, optimise::StartupError>, CmdError> {
    check_name(&name)?;
    let (svc, state) = (svc.inner().clone(), state.inner().clone());
    blocking(move || {
        let now = unhoard_core::fs::FileSystem::now(&unhoard_platform::WindowsFs);
        let filetime = unhoard_core::time::unix_to_filetime(now);
        let result =
            optimise::startup_set(&svc.context().roots, scope, &name, enabled, filetime, false);
        let op = if enabled {
            "startupEnable"
        } else {
            "startupDisable"
        };
        svc.audit_startup(op, &name, result.is_ok());
        Ok(result.map(|change| {
            let mut changes = state
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            changes.push(change);
            changes.len() - 1
        }))
    })
    .await
}

/// Undoes a startup switch made this session: writes back the exact old value.
#[tauri::command]
pub async fn startup_undo(
    svc: Svc<'_>,
    state: State<'_, Arc<OptimiseState>>,
    index: usize,
) -> Result<Result<(), optimise::StartupError>, CmdError> {
    let (svc, state) = (svc.inner().clone(), state.inner().clone());
    blocking(move || {
        let change = state
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(index)
            .cloned()
            .ok_or(CmdError::InvalidArgument {
                what: "index".into(),
            })?;
        let result = optimise::startup_undo(&change);
        svc.audit_startup("startupUndo", &change.name, result.is_ok());
        Ok(result)
    })
    .await
}

/// A startup entry name from the webview: plain text of a sane length.
fn check_name(name: &str) -> Result<(), CmdError> {
    // Registry value names may hold backslashes; they are never used as a path.
    if name.is_empty() || name.len() > 260 || name.contains('\0') {
        return Err(CmdError::InvalidArgument {
            what: "name".into(),
        });
    }
    Ok(())
}

/// Stops the answer being written.
#[derive(Default)]
pub struct ChatState(AtomicBool);

/// The models Ollama has installed, with the recommended one marked. Opens a connection
/// only when the chat is switched on (AC8.6).
#[tauri::command]
pub async fn chat_models() -> Result<Result<Vec<chat::ModelInfo>, ChatError>, CmdError> {
    blocking(|| {
        if !settings_get().chat {
            return Ok(Err(ChatError::Off));
        }
        Ok(chat::models())
    })
    .await
}

/// Answers the last message of `messages`; the answer arrives as `chat-event` events.
#[tauri::command]
pub async fn chat_send(
    app: AppHandle,
    svc: Svc<'_>,
    state: State<'_, Arc<ChatState>>,
    messages: Vec<chat::Message>,
    lang: String,
) -> Result<Result<(), ChatError>, CmdError> {
    if messages.is_empty() || messages.len() > 200 || !["en", "nl"].contains(&lang.as_str()) {
        return Err(CmdError::InvalidArgument {
            what: "messages".into(),
        });
    }
    let (svc, state) = (svc.inner().clone(), state.inner().clone());
    blocking(move || {
        let settings = settings_get();
        if !settings.chat {
            return Ok(Err(ChatError::Off));
        }
        let model = settings
            .chat_model
            .unwrap_or_else(|| chat::RECOMMENDED.to_string());
        state.0.store(false, Ordering::SeqCst);
        let emit = |e: chat::ChatEvent| {
            let _ui_may_be_closed = app.emit("chat-event", e);
        };
        Ok(chat::converse(
            &svc, &model, &messages, &lang, &emit, &state.0,
        ))
    })
    .await
}

/// Stops the answer being written; Ollama keeps running.
#[tauri::command]
pub fn chat_stop(state: State<'_, Arc<ChatState>>) {
    state.0.store(true, Ordering::SeqCst);
}

/// The storage-history timeline of a fixed drive (`C:`).
#[tauri::command]
pub async fn history_timeline(svc: Svc<'_>, drive: String) -> Result<Vec<TimelinePoint>, CmdError> {
    let root = drive_root(&drive)?;
    let svc = svc.inner().clone();
    blocking(move || Ok(svc.history_timeline(&root))).await
}

/// What grew on a fixed drive over the last `days` days (1, 7, 30 or 90).
#[tauri::command]
pub async fn history_growth(
    svc: Svc<'_>,
    drive: String,
    days: u32,
) -> Result<GrowthView, CmdError> {
    if ![1, 7, 30, 90].contains(&days) {
        return Err(CmdError::InvalidArgument {
            what: "days".into(),
        });
    }
    let root = drive_root(&drive)?;
    let svc = svc.inner().clone();
    blocking(move || Ok(svc.history_growth(&root, days))).await
}

/// Deletes the storage history (Settings › Background insights › Clear).
#[tauri::command]
pub async fn history_clear(svc: Svc<'_>) -> Result<(), CmdError> {
    let svc = svc.inner().clone();
    blocking(move || svc.history_clear().map_err(io)).await
}

/// `C:\` for `C:` when it is one of this PC's fixed drives.
fn drive_root(drive: &str) -> Result<String, CmdError> {
    let drive = drive.trim_end_matches('\\').to_uppercase();
    if !system::fixed_drives().contains(&drive) {
        return Err(CmdError::InvalidArgument {
            what: "drive".into(),
        });
    }
    Ok(format!("{drive}\\"))
}

/// Live scan progress, sent as the `scan-progress` event.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatus {
    generation: u64,
    root: String,
    drive_scan: bool,
    files: u64,
    on_disk: u64,
    done: bool,
    partial: bool,
    elapsed_ms: u64,
}

fn status(svc: &Service) -> Result<ScanStatus, CmdError> {
    let s = svc.current()?;
    let done = s.done.load(Ordering::SeqCst);
    Ok(ScanStatus {
        generation: s.generation,
        root: s.root.clone(),
        drive_scan: s.drive_scan,
        files: s.control.files.load(Ordering::Relaxed),
        on_disk: s.control.on_disk.load(Ordering::Relaxed),
        done,
        partial: done && s.control.cancel.load(Ordering::SeqCst),
        elapsed_ms: if done {
            s.duration_ms.load(Ordering::SeqCst)
        } else {
            s.started.elapsed().as_millis() as u64
        },
    })
}

/// Starts a scan of a fixed drive (`C:`) and streams progress every 200 ms.
#[tauri::command]
pub fn scan_start(app: AppHandle, svc: Svc<'_>, drive: String) -> Result<u64, CmdError> {
    let drive = drive.trim_end_matches('\\').to_uppercase();
    if !system::fixed_drives().contains(&drive) {
        return Err(CmdError::InvalidArgument {
            what: "drive".into(),
        });
    }
    let session = svc.start_scan(&format!("{drive}\\"), true)?;
    let generation = session.generation;
    let svc = svc.inner().clone();
    std::thread::spawn(move || {
        loop {
            let done = session.done.load(Ordering::SeqCst);
            if let Ok(s) = status(&svc) {
                let _ui_may_be_closed = app.emit("scan-progress", s);
            }
            if done {
                break;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    });
    Ok(generation)
}

/// Stops the running scan; the partial result stays.
#[tauri::command]
pub fn scan_cancel(svc: Svc<'_>) {
    svc.cancel_scan();
}

/// The state of the current scan.
#[tauri::command]
pub fn scan_status(svc: Svc<'_>) -> Result<ScanStatus, CmdError> {
    status(&svc)
}

/// One level of the tree.
#[tauri::command]
pub fn view(
    svc: Svc<'_>,
    generation: u64,
    id: u32,
    top: u16,
    logical: bool,
) -> Result<ViewResponse, CmdError> {
    svc.view(generation, id, top, logical)
}

/// The details of one item.
#[tauri::command]
pub fn details(svc: Svc<'_>, generation: u64, id: u32) -> Result<Details, CmdError> {
    svc.details(generation, id)
}

/// The Suggestions view.
#[tauri::command]
pub async fn suggestions(svc: Svc<'_>, limit: u16) -> Result<(u64, Vec<Suggestion>), CmdError> {
    let svc = svc.inner().clone();
    blocking(move || svc.suggestions(limit)).await
}

/// The preview of a move. Writes nothing.
#[tauri::command]
pub async fn quarantine_preview(
    svc: Svc<'_>,
    generation: u64,
    ids: Vec<u32>,
) -> Result<MovePlan, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || svc.quarantine_preview(generation, &ids)).await
}

/// Moves items to the quarantine after the user confirmed the preview.
#[tauri::command]
pub async fn quarantine_move(
    svc: Svc<'_>,
    generation: u64,
    ids: Vec<u32>,
) -> Result<MoveReport, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || svc.quarantine_move(generation, &ids)).await
}

/// The Quarantine view.
#[tauri::command]
pub async fn quarantine_state(svc: Svc<'_>) -> Result<QuarantineState, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || Ok(svc.quarantine_state())).await
}

/// Restores items of a batch.
#[tauri::command]
pub async fn quarantine_restore(
    svc: Svc<'_>,
    batch_id: String,
    item_ids: Vec<String>,
) -> Result<Vec<ItemResult>, CmdError> {
    if item_ids.is_empty() || item_ids.len() > 10_000 {
        return Err(CmdError::InvalidArgument {
            what: "itemIds".into(),
        });
    }
    let svc = svc.inner().clone();
    blocking(move || svc.restore(&batch_id, &item_ids)).await
}

/// Records that the companion showed a purge warning.
#[tauri::command]
pub fn quarantine_warning_shown(
    svc: Svc<'_>,
    batch_id: String,
    days_left: i64,
) -> Result<(), CmdError> {
    svc.warning_shown(&batch_id, days_left)
}

/// Opens a batch folder in Explorer (explicit click only).
#[tauri::command]
pub fn quarantine_open(svc: Svc<'_>, batch_id: String) -> Result<(), CmdError> {
    system::open_folder(&svc.batch_dir(&batch_id)?).map_err(io)
}

/// Shows a scanned item in Explorer (explicit click only).
#[tauri::command]
pub fn show_in_explorer(svc: Svc<'_>, generation: u64, id: u32) -> Result<(), CmdError> {
    system::show_in_explorer(&svc.path_of(generation, id)?).map_err(io)
}

/// Opens a Windows tool from the allowlist (explicit click only).
#[tauri::command]
pub fn open_tool(tool: WindowsTool) -> Result<(), CmdError> {
    system::open_tool(tool).map_err(io)
}

/// The cloud sync roots with their sizes in the current scan.
#[tauri::command]
pub fn cloud_roots(svc: Svc<'_>, generation: u64) -> Result<Vec<CloudRoot>, CmdError> {
    svc.cloud_roots(generation)
}

/// The preview of "free up space".
#[tauri::command]
pub async fn free_up_preview(
    svc: Svc<'_>,
    generation: u64,
    ids: Vec<u32>,
) -> Result<FreeUpPlan, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || svc.free_up_preview(generation, &ids)).await
}

/// The result of "free up space".
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeUpResult {
    failed: Vec<(String, FsError)>,
    requested_bytes: u64,
    still_local_bytes: u64,
}

/// Frees up space (pin state only) and re-measures.
#[tauri::command]
pub async fn free_up_apply(
    svc: Svc<'_>,
    generation: u64,
    ids: Vec<u32>,
) -> Result<FreeUpResult, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || {
        let (results, requested, still) = svc.free_up_apply(generation, &ids)?;
        let failed = results
            .into_iter()
            .filter_map(|(p, r)| r.err().map(|e| (p, e)))
            .collect();
        Ok(FreeUpResult {
            failed,
            requested_bytes: requested,
            still_local_bytes: still,
        })
    })
    .await
}

/// The elevated helper, kept for the whole app session once the user said yes, so the UAC
/// prompt appears only once.
#[derive(Default)]
pub struct AdminState(Mutex<Option<HelperSession>>);

/// The lock on the session; a panic in another reader does not lose it.
fn session(admin: &AdminState) -> std::sync::MutexGuard<'_, Option<HelperSession>> {
    admin
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn measured(r: &RevealedFolder) -> Option<Measured> {
    r.error.is_none().then(|| Measured {
        path: r.path.clone(),
        logical: r.logical,
        on_disk: r.on_disk,
        files: r.files,
        children: r.children.clone(),
    })
}

/// Reads `folders` with the running helper, or starts one now (UAC) and keeps it, and fills
/// the scan with what it measured. Reads wait for each other, so the helper serves one
/// request at a time.
fn read_elevated(
    svc: &Service,
    admin: &AdminState,
    generation: u64,
    folders: &[String],
) -> Result<Result<(Vec<RevealedFolder>, MeasuredSummary), RevealError>, CmdError> {
    let mut guard = session(admin);
    if !guard.as_ref().is_some_and(HelperSession::alive) {
        match start_helper() {
            Ok(s) => *guard = Some(s),
            Err(e) => return Ok(Err(e)),
        }
    }
    let Some(helper) = guard.as_ref() else {
        return Ok(Err(RevealError::Declined));
    };
    let results = match helper.read(folders) {
        Ok(r) => r,
        Err(e) => {
            // A failed helper is not reused; the next read starts a new one.
            *guard = None;
            return Ok(Err(e));
        }
    };
    drop(guard);
    let found: Vec<Measured> = results.iter().filter_map(measured).collect();
    let summary = svc.apply_measured(generation, &found)?;
    Ok(Ok((results, summary)))
}

/// Starts the elevated read-only helper (one UAC prompt per app session), so it can read
/// the folders the scan cannot once the scan ends. `Ok(false)` when the user declines;
/// `Ok(true)` at once when the helper already runs.
#[tauri::command]
pub async fn admin_start(
    admin: State<'_, Arc<AdminState>>,
) -> Result<Result<bool, RevealError>, CmdError> {
    let admin = admin.inner().clone();
    blocking(move || {
        let mut guard = session(&admin);
        if guard.as_ref().is_some_and(HelperSession::alive) {
            return Ok(Ok(true));
        }
        Ok(match start_helper() {
            Ok(s) => {
                *guard = Some(s);
                Ok(true)
            }
            Err(RevealError::Declined) => Ok(false),
            Err(e) => Err(e),
        })
    })
    .await
}

/// Reads unreadable folders with admin rights: the given ones, or all of the current scan.
#[tauri::command]
pub async fn admin_read(
    svc: Svc<'_>,
    admin: State<'_, Arc<AdminState>>,
    generation: u64,
    ids: Option<Vec<u32>>,
) -> Result<Result<MeasuredSummary, RevealError>, CmdError> {
    let (svc, admin) = (svc.inner().clone(), admin.inner().clone());
    blocking(move || {
        let folders = svc.unscanned_paths(generation, ids.as_deref())?;
        if folders.is_empty() {
            // Nothing to read now; the helper stays for the next scan.
            return Ok(Ok(MeasuredSummary::default()));
        }
        Ok(read_elevated(&svc, &admin, generation, &folders)?.map(|(_, sum)| sum))
    })
    .await
}

/// "Reveal hidden space": reads the unreadable folders of the current drive scan plus System
/// Volume Information with admin rights, fills the scan with what it measured, and returns
/// every folder for the hidden-space panel.
#[tauri::command]
pub async fn reveal_hidden(
    svc: Svc<'_>,
    admin: State<'_, Arc<AdminState>>,
    generation: u64,
) -> Result<Result<Vec<RevealedView>, RevealError>, CmdError> {
    let (svc, admin) = (svc.inner().clone(), admin.inner().clone());
    blocking(move || {
        let s = svc.current()?;
        if s.generation != generation {
            return Err(CmdError::StaleScan);
        }
        if !s.drive_scan {
            return Err(CmdError::InvalidArgument {
                what: "scan".into(),
            });
        }
        let mut folders = svc.unscanned_paths(generation, None)?;
        folders.truncate(4095);
        let svi = format!("{}System Volume Information", s.root);
        if !folders.iter().any(|f| f.eq_ignore_ascii_case(&svi)) {
            folders.push(svi);
        }
        let roots = svc.context().roots.clone();
        Ok(
            read_elevated(&svc, &admin, generation, &folders)?.map(|(all, _)| {
                all.into_iter()
                    .map(|folder| RevealedView {
                        kind: hidden_kind(&folder.path, &roots),
                        folder,
                    })
                    .collect()
            }),
        )
    })
    .await
}

/// A revealed folder with its kind, for grouping in the hidden-space panel.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevealedView {
    #[serde(flatten)]
    folder: RevealedFolder,
    kind: HiddenKind,
}

/// The priority matrix of the current scan.
#[tauri::command]
pub async fn matrix(svc: Svc<'_>, limit: u16) -> Result<(u64, Vec<MatrixPoint>), CmdError> {
    let svc = svc.inner().clone();
    blocking(move || svc.matrix(limit)).await
}

/// One Ollama model, with the scan IDs that move it to the quarantine as a whole.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OllamaModelView {
    #[serde(flatten)]
    model: OllamaModel,
    /// The manifest and the blobs only this model uses; `None` if one is not in the scan.
    ids: Option<Vec<u32>>,
}

/// The Ollama models on this PC, read from their manifests.
#[tauri::command]
pub async fn ollama_models(
    svc: Svc<'_>,
    generation: u64,
) -> Result<Vec<OllamaModelView>, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || {
        let models = knowledge::ollama_models(&svc.context().roots);
        models
            .into_iter()
            .map(|model| {
                let mut paths = vec![model.manifest.clone()];
                paths.extend(model.unique_blobs.iter().cloned());
                let ids = svc.ids_for_paths(generation, &paths)?;
                Ok(OllamaModelView {
                    ids: ids.into_iter().collect(),
                    model,
                })
            })
            .collect()
    })
    .await
}

/// What fills Docker Desktop's disk, from `docker system df` (on a click only).
#[tauri::command]
pub async fn docker_usage(svc: Svc<'_>) -> Result<Result<Vec<DockerUsage>, ToolError>, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || {
        Ok(tools::docker_system_df(&svc.context().roots).map(|out| docker::parse_system_df(&out)))
    })
    .await
}

/// Opens Docker Desktop, Ollama or Steam (explicit click only).
#[tauri::command]
pub fn open_app(svc: Svc<'_>, app: tools::App) -> Result<Result<(), ToolError>, CmdError> {
    Ok(tools::open_app(app, &svc.context().roots))
}

/// Opens the uninstall of the app that installed an item: Steam's own dialog for a game.
#[tauri::command]
pub fn app_uninstall(svc: Svc<'_>, generation: u64, id: u32) -> Result<(), CmdError> {
    let app_id = svc.steam_app_id(generation, id)?;
    tools::steam_uninstall(app_id).map_err(io)
}

/// Analyses photos and videos of the current scan.
#[tauri::command]
pub async fn photos_analyze(svc: Svc<'_>) -> Result<PhotoState, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || svc.photos()).await
}

/// A photo thumbnail for the review grid.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbOut {
    width: u32,
    height: u32,
    /// RGBA pixels, base64.
    rgba: String,
}

/// The thumbnail of a photo from the last analysis (cache only for cloud files).
#[tauri::command]
pub async fn photo_thumbnail(
    svc: Svc<'_>,
    generation: u64,
    index: u32,
    side: u32,
) -> Result<ThumbOut, CmdError> {
    let svc = svc.inner().clone();
    blocking(move || {
        let (path, in_cloud) = svc.photo_path(generation, index)?;
        let t = media::thumbnail(&path, side.clamp(32, 256), in_cloud)?;
        let mut rgba = t.bgra;
        for px in rgba.as_chunks_mut::<4>().0 {
            px.swap(0, 2);
            px[3] = 255;
        }
        Ok(ThumbOut {
            width: t.width,
            height: t.height,
            rgba: base64(&rgba),
        })
    })
    .await
}

/// Standard base64 with padding.
pub fn base64(data: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(A[((n >> (18 - 6 * i)) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// The read-me written into every batch folder, in English and Dutch.
pub fn readme(m: &Manifest) -> String {
    let mut s = format!(
        "Unhoard quarantine batch. These items are kept until {} and then deleted.\r\n\
         Restore them in Unhoard › Quarantine. To delete earlier, delete them here in Explorer.\r\n\r\n\
         Quarantainebatch van Unhoard. Deze items blijven tot {} en worden daarna verwijderd.\r\n\
         Zet ze terug via Unhoard › Quarantaine. Eerder verwijderen kan hier in Verkenner.\r\n\r\n",
        &m.purge_after[..10.min(m.purge_after.len())],
        &m.purge_after[..10.min(m.purge_after.len())]
    );
    for i in &m.items {
        s.push_str(&format!("{}\r\n", i.original_path));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn settings_are_validated() {
        assert!(Settings::default().valid());
        let bad = Settings {
            map_style: "pie".into(),
            ..Settings::default()
        };
        assert!(!bad.valid());
        let lang = Settings {
            language: Some("de".into()),
            ..Settings::default()
        };
        assert!(!lang.valid());
    }
}

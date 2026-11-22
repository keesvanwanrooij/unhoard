//! Local tools an app adapter may use, only on an explicit user click: Docker's
//! `docker system df`, Steam's own uninstall dialog, and opening Docker Desktop, Ollama or
//! Steam.
//!
//! A tool runs from the app's fixed install folder (never from `PATH`, the registry or a
//! scanned path),
//! with fixed arguments, no shell and no console window, a time limit and an output
//! limit. Its output is untrusted data for the core's parsers.

use std::io::Read;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use unhoard_core::roots::{Roots, join};

/// `CREATE_NO_WINDOW`: no console window flashes up.
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// How long a tool may run.
const TIME_LIMIT: Duration = Duration::from_secs(15);
/// The most output read from a tool.
const MAX_OUTPUT: u64 = 64 << 10;

/// Why a tool gave no answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ToolError {
    /// The tool is not installed in its usual folder.
    NotInstalled,
    /// The tool ran but failed, for example because Docker Desktop is not running.
    NotRunning,
    /// The tool took longer than the time limit and was stopped.
    TimedOut,
    /// The user declined the UAC prompt; nothing changed.
    Declined,
    /// The tool could not be started.
    Failed,
}

/// An app Unhoard can open on a click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum App {
    /// Docker Desktop.
    DockerDesktop,
    /// The Ollama app.
    Ollama,
    /// Steam's library.
    Steam,
    /// Ollama's download page in the browser (a fixed address).
    OllamaDownload,
}

/// The Ollama app's file in its default per-user install folder.
fn ollama_app(roots: &Roots) -> Option<String> {
    existing(
        roots
            .var("LOCALAPPDATA")
            .map(|l| path_of(l, &["Programs", "Ollama", "ollama app.exe"]))
            .into_iter()
            .collect(),
    )
}

/// True when the Ollama app is installed in its default folder.
pub fn ollama_installed(roots: &Roots) -> bool {
    ollama_app(roots).is_some()
}

fn path_of(base: &str, parts: &[&str]) -> String {
    parts
        .iter()
        .fold(base.to_string(), |p, part| join(&p, part))
}

/// Docker Desktop's install folders: per user (the default) and per machine.
fn docker_roots(roots: &Roots) -> Vec<String> {
    let mut out = vec![];
    if let Some(local) = roots.var("LOCALAPPDATA") {
        out.push(path_of(local, &["Programs", "DockerDesktop"]));
    }
    if let Some(pf) = roots
        .var("PROGRAMW6432")
        .or_else(|| roots.var("PROGRAMFILES"))
    {
        out.push(path_of(pf, &["Docker", "Docker"]));
    }
    out
}

/// The first of `candidates` that exists.
fn existing(candidates: Vec<String>) -> Option<String> {
    candidates
        .into_iter()
        .find(|p| std::path::Path::new(p).is_file())
}

/// `docker.exe` in Docker Desktop's install folder.
fn docker_exe(roots: &Roots) -> Option<String> {
    existing(
        docker_roots(roots)
            .iter()
            .map(|r| path_of(r, &["resources", "bin", "docker.exe"]))
            .collect(),
    )
}

/// Opens an app from its fixed install folder, Steam through its own link, or Ollama's
/// download page in the browser.
pub fn open_app(app: App, roots: &Roots) -> Result<(), ToolError> {
    let target = match app {
        App::DockerDesktop => existing(
            docker_roots(roots)
                .iter()
                .map(|r| join(r, "Docker Desktop.exe"))
                .collect(),
        ),
        App::Ollama => ollama_app(roots),
        App::Steam => Some("steam://open/games".to_string()),
        App::OllamaDownload => Some("https://ollama.com/download/windows".to_string()),
    }
    .ok_or(ToolError::NotInstalled)?;
    crate::win::elevate::shell_open(&target).map_err(|_| ToolError::Failed)
}

/// Runs `exe` with fixed `args`, no shell and no window, for at most `limit`, and returns
/// at most [`MAX_OUTPUT`] of its standard output.
fn run(exe: &str, args: &[&str], limit: Duration) -> Result<String, ToolError> {
    let mut child = Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|_| ToolError::Failed)?;
    let stdout = child.stdout.take().ok_or(ToolError::Failed)?;
    let reader = std::thread::spawn(move || {
        let mut out = String::new();
        let _read = stdout.take(MAX_OUTPUT).read_to_string(&mut out);
        out
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < limit => std::thread::sleep(Duration::from_millis(50)),
            Ok(None) => {
                let _killed = child.kill();
                let _reaped = child.wait();
                return Err(ToolError::TimedOut);
            }
            Err(_) => return Err(ToolError::Failed),
        }
    };
    let out = reader.join().map_err(|_| ToolError::Failed)?;
    if status.success() {
        Ok(out)
    } else {
        Err(ToolError::NotRunning)
    }
}

/// Runs `docker system df` as JSON lines and returns its output.
pub fn docker_system_df(roots: &Roots) -> Result<String, ToolError> {
    let exe = docker_exe(roots).ok_or(ToolError::NotInstalled)?;
    run(
        &exe,
        &["system", "df", "--format", "{{json .}}"],
        TIME_LIMIT,
    )
}

/// Docker's own `wsl` folder, below which its virtual disks live.
pub fn docker_wsl_dir(roots: &Roots) -> Option<String> {
    roots
        .var("LOCALAPPDATA")
        .map(|l| path_of(l, &["Docker", "wsl"]))
}

/// Docker's data disk: where it is, what it takes on disk and what is in use inside it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerDisk {
    /// The `.vhdx` file.
    pub path: String,
    /// Its size on disk.
    pub size: u64,
    /// The bytes in use inside, when Docker runs and could tell.
    pub used: Option<u64>,
}

/// `wsl.exe` in the Windows folder.
fn wsl_exe(roots: &Roots) -> Option<String> {
    existing(
        roots
            .var("WINDIR")
            .map(|w| path_of(w, &["System32", "wsl.exe"]))
            .into_iter()
            .collect(),
    )
}

/// Docker's data disk, measured. Runs `df` inside Docker's own WSL distribution, so call it
/// only on a click.
pub fn docker_disk(roots: &Roots) -> Option<DockerDisk> {
    let dir = docker_wsl_dir(roots)?;
    let path = existing(vec![
        path_of(&dir, &["disk", "docker_data.vhdx"]),
        path_of(&dir, &["data", "ext4.vhdx"]),
    ])?;
    let size = std::fs::metadata(&path).ok()?.len();
    let used = wsl_exe(roots).and_then(|wsl| {
        run(
            &wsl,
            &[
                "-d",
                "docker-desktop",
                "-u",
                "root",
                "-e",
                "df",
                "-B1",
                "/mnt/docker-desktop-disk",
            ],
            TIME_LIMIT,
        )
        .ok()
        .and_then(|out| unhoard_core::adapters::docker::parse_df_used(&out))
    });
    Some(DockerDisk { path, size, used })
}

/// Compacts Docker's data disk: stops Docker Desktop, shuts WSL down, runs `diskpart` with a
/// fixed script through one UAC prompt, and starts Docker Desktop again. The script is
/// written to `work_dir` (the app's own folder) and removed after. Returns the bytes freed.
/// Only Docker's own `.vhdx` below its `wsl` folder is accepted (S7).
pub fn compact_docker_disk(roots: &Roots, work_dir: &str) -> Result<u64, ToolError> {
    let disk = docker_disk(roots).ok_or(ToolError::NotInstalled)?;
    let dir = docker_wsl_dir(roots).ok_or(ToolError::NotInstalled)?;
    let script = unhoard_core::adapters::docker::compact_script(&disk.path, &dir)
        .ok_or(ToolError::Failed)?;
    let docker = docker_exe(roots).ok_or(ToolError::NotInstalled)?;
    let wsl = wsl_exe(roots).ok_or(ToolError::NotInstalled)?;
    let diskpart = existing(
        roots
            .var("WINDIR")
            .map(|w| path_of(w, &["System32", "diskpart.exe"]))
            .into_iter()
            .collect(),
    )
    .ok_or(ToolError::NotInstalled)?;
    let script_path = join(work_dir, "compact-docker-disk.txt");
    std::fs::write(&script_path, script).map_err(|_| ToolError::Failed)?;
    // Docker Desktop may already be stopped; then this fails and the work goes on.
    let _stopped = run(&docker, &["desktop", "stop"], Duration::from_secs(120));
    let result = run(&wsl, &["--shutdown"], Duration::from_secs(60)).and_then(|_| {
        match crate::win::elevate::run_elevated(&diskpart, &format!("/s \"{script_path}\"")) {
            Ok(Some(p)) => {
                if p.wait(30 * 60 * 1000) {
                    Ok(())
                } else {
                    Err(ToolError::TimedOut)
                }
            }
            Ok(None) => Err(ToolError::Declined),
            Err(_) => Err(ToolError::Failed),
        }
    });
    let _removed = std::fs::remove_file(&script_path);
    let _started = run(&docker, &["desktop", "start"], Duration::from_secs(180));
    result?;
    let after = std::fs::metadata(&disk.path)
        .map(|m| m.len())
        .unwrap_or(disk.size);
    Ok(disk.size.saturating_sub(after))
}

/// Opens Steam's own uninstall dialog for a game. Steam asks the user to confirm.
pub fn steam_uninstall(app_id: u32) -> std::io::Result<()> {
    // The URL holds only a number, so nothing from a file name reaches the command line.
    Command::new("explorer.exe")
        .arg(format!("steam://uninstall/{app_id}"))
        .spawn()
        .map(drop)
}

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

/// Runs `docker system df` as JSON lines and returns its output.
pub fn docker_system_df(roots: &Roots) -> Result<String, ToolError> {
    let exe = docker_exe(roots).ok_or(ToolError::NotInstalled)?;
    let mut child = Command::new(exe)
        .args(["system", "df", "--format", "{{json .}}"])
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
            Ok(None) if start.elapsed() < TIME_LIMIT => {
                std::thread::sleep(Duration::from_millis(50))
            }
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

/// Opens Steam's own uninstall dialog for a game. Steam asks the user to confirm.
pub fn steam_uninstall(app_id: u32) -> std::io::Result<()> {
    // The URL holds only a number, so nothing from a file name reaches the command line.
    Command::new("explorer.exe")
        .arg(format!("steam://uninstall/{app_id}"))
        .spawn()
        .map(drop)
}

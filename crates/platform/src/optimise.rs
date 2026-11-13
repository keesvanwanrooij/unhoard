//! Optimise (F16): gathers what the check-up shows, and the startup toggle.
//!
//! Everything here reads, except [`startup_set`], which writes only the current user's
//! `StartupApproved` value of one entry, the way Task Manager does, and returns the old value
//! so the change can be undone exactly. Risk class High (AGENTS.md).

use std::path::Path;

use serde::{Deserialize, Serialize};
use unhoard_core::optimise::{
    OptimiseData, StartupEntry, StartupScope, approved, disabled_value, enabled_value,
};
use unhoard_core::roots::Roots;

use crate::win::{checkup, registry};

/// The program file a startup command starts: the quoted part, or everything up to `.exe`,
/// with `%VAR%` expanded.
fn command_file(command: &str) -> Option<String> {
    let c = command.trim();
    let raw = if let Some(rest) = c.strip_prefix('"') {
        rest.split('"').next()?.to_string()
    } else {
        let lower = c.to_ascii_lowercase();
        let end = lower.find(".exe").map(|i| i + 4).unwrap_or(c.len());
        c[..end].to_string()
    };
    let expanded = expand(&raw);
    (expanded.len() >= 3).then_some(expanded)
}

/// Replaces `%VAR%` with the environment value.
fn expand(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                out.push_str(&std::env::var(name).unwrap_or_else(|_| format!("%{name}%")));
                rest = &after[end + 1..];
            }
            None => {
                out.push('%');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The startup list name of a scope in `StartupApproved`.
fn list_of(scope: StartupScope, run32: bool) -> &'static str {
    match scope {
        StartupScope::UserFolder | StartupScope::CommonFolder => "StartupFolder",
        _ if run32 => "Run32",
        _ => "Run",
    }
}

/// Every startup entry of the user and the machine, with its approved state.
pub fn startup_entries(roots: &Roots) -> Vec<StartupEntry> {
    let mut out = vec![];
    for (machine, key, list) in registry::RUN_KEYS {
        let scope = if machine {
            StartupScope::MachineRun
        } else {
            StartupScope::UserRun
        };
        for (name, command) in registry::run_entries(machine, key) {
            let file = command_file(&command);
            let value = registry::approved_value(machine, registry::APPROVED, list, &name);
            out.push(StartupEntry {
                file_exists: file.as_deref().is_none_or(|f| Path::new(f).exists()),
                enabled: approved(value.as_deref()),
                name,
                command,
                file,
                scope,
                publisher: None,
            });
        }
    }
    let folders = [
        (
            false,
            roots
                .var("APPDATA")
                .map(|a| format!(r"{a}\Microsoft\Windows\Start Menu\Programs\Startup")),
        ),
        (
            true,
            roots
                .program_data()
                .map(|d| format!(r"{d}\Microsoft\Windows\Start Menu\Programs\StartUp")),
        ),
    ];
    for (machine, dir) in folders {
        let Some(dir) = dir else { continue };
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in read.flatten().take(256) {
            let name = e.file_name().to_string_lossy().to_string();
            if name.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            let value =
                registry::approved_value(machine, registry::APPROVED, "StartupFolder", &name);
            out.push(StartupEntry {
                command: format!(r"{dir}\{name}"),
                file: Some(format!(r"{dir}\{name}")),
                file_exists: true,
                enabled: approved(value.as_deref()),
                name,
                scope: if machine {
                    StartupScope::CommonFolder
                } else {
                    StartupScope::UserFolder
                },
                publisher: None,
            });
        }
    }
    out
}

/// Everything the check-up shows except Windows Update, measured now. Takes about a second
/// (CPU sampling); call it off the UI thread.
pub fn gather(roots: &Roots, system_free: Option<u64>, system_total: Option<u64>) -> OptimiseData {
    OptimiseData {
        memory: checkup::memory(),
        processes: checkup::top_processes(5),
        disks: checkup::disks(),
        system_free,
        system_total,
        startup: startup_entries(roots),
        updates: None,
        page_file_on: registry::page_file_on(),
        power_mode: registry::power_mode(),
    }
}

/// Pending updates and recent failures from the Windows Update Agent. Its search can take
/// half a minute, so it runs on its own and the screen shows the rest first.
pub fn updates(now: i64) -> Option<unhoard_core::optimise::Updates> {
    checkup::updates(now)
}

/// What a startup switch changed, so it can be undone exactly.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupChange {
    /// The entry's name.
    pub name: String,
    /// Its scope.
    pub scope: StartupScope,
    /// The value before the change; `None` when there was none.
    pub old: Option<Vec<u8>>,
    /// Whether the entry is enabled now.
    pub enabled: bool,
}

/// Why a startup switch did not happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StartupError {
    /// The entry is machine-wide; switching it needs admin, so Task Manager is the place.
    NeedsAdmin,
    /// No such entry.
    NotFound,
    /// Windows refused the write.
    Failed,
}

/// Enables or disables a startup entry of the current user, as Task Manager does. With
/// `dry_run`, nothing is written and the change it would make is returned.
pub fn startup_set(
    roots: &Roots,
    scope: StartupScope,
    name: &str,
    enabled: bool,
    filetime_now: u64,
    dry_run: bool,
) -> Result<StartupChange, StartupError> {
    if !scope.user_may_switch() {
        return Err(StartupError::NeedsAdmin);
    }
    if !startup_entries(roots)
        .iter()
        .any(|e| e.scope == scope && e.name == name)
    {
        return Err(StartupError::NotFound);
    }
    let list = list_of(scope, false);
    let old = registry::approved_value(false, registry::APPROVED, list, name);
    let new = if enabled {
        enabled_value()
    } else {
        disabled_value(filetime_now)
    };
    if !dry_run && !registry::set_user_approved(registry::APPROVED, list, name, Some(&new)) {
        return Err(StartupError::Failed);
    }
    Ok(StartupChange {
        name: name.to_string(),
        scope,
        old,
        enabled,
    })
}

/// Undoes a startup switch: writes back the exact old value, or removes the value when
/// there was none.
pub fn startup_undo(change: &StartupChange) -> Result<(), StartupError> {
    if !change.scope.user_may_switch() {
        return Err(StartupError::NeedsAdmin);
    }
    let list = list_of(change.scope, false);
    if registry::set_user_approved(
        registry::APPROVED,
        list,
        &change.name,
        change.old.as_deref(),
    ) {
        Ok(())
    } else {
        Err(StartupError::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f16_the_program_file_comes_from_the_command() {
        assert_eq!(
            command_file(r#""C:\Apps\chat.exe" --tray"#).as_deref(),
            Some(r"C:\Apps\chat.exe")
        );
        assert_eq!(
            command_file(r"C:\Apps\tool.exe /silent").as_deref(),
            Some(r"C:\Apps\tool.exe")
        );
        unsafe_free_env_check();
    }

    /// `%SystemRoot%` expands without touching the environment.
    fn unsafe_free_env_check() {
        let got = command_file(r"%SystemRoot%\System32\x.exe").unwrap_or_default();
        assert!(got.to_ascii_lowercase().ends_with(r"\system32\x.exe"));
        assert!(!got.contains('%'));
    }

    #[test]
    fn f16_t16_4_a_dry_run_writes_nothing_and_machine_entries_need_admin() {
        let roots = Roots::default();
        assert_eq!(
            startup_set(&roots, StartupScope::MachineRun, "x", false, 1, true),
            Err(StartupError::NeedsAdmin)
        );
        assert_eq!(
            startup_set(
                &roots,
                StartupScope::UserRun,
                "no such entry zz",
                false,
                1,
                true
            ),
            Err(StartupError::NotFound)
        );
    }
}

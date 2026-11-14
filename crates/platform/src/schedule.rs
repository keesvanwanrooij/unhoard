//! The daily measurement of storage history (F17): a scheduled task for the current user
//! that runs `unhoard-app.exe --measure` once a day when the PC is idle, and the checks the
//! measurement makes before and while it runs.
//!
//! The task is created and removed with Windows' own `schtasks.exe` from `System32`, with an
//! XML definition Unhoard writes itself: it needs no admin, runs only while the user is
//! signed in, never on battery, at most 30 minutes, at the lowest task priority.

use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

/// The task's name in Task Scheduler.
pub const TASK_NAME: &str = "Unhoard daily measurement";
/// The argument that makes the executable run the measurement and exit.
pub const MEASURE_ARG: &str = "--measure";
/// CPU time one measurement may use, in seconds; the scan stops when it is reached. Twice
/// what a measurement of a 1 TB system drive with 2.8 million files took (124 s), which is
/// below the 0.3% of one core per day the Optimise limits allow (259 s).
pub const CPU_BUDGET_SECONDS: f64 = 250.0;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Escapes text for an XML element.
fn xml_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The task definition for `exe`: daily at 12:00, started when missed, only when idle for
/// 10 minutes, not on battery, at most 30 minutes, priority 7 (below normal).
pub fn task_xml(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo><Description>Measures the size of the largest folders once a day for Unhoard's storage history. Turn it off in Unhoard › Settings › Background insights.</Description></RegistrationInfo>
  <Triggers>
    <CalendarTrigger>
      <StartBoundary>2026-01-01T12:00:00</StartBoundary>
      <ScheduleByDay><DaysInterval>1</DaysInterval></ScheduleByDay>
    </CalendarTrigger>
  </Triggers>
  <Principals><Principal id="Author"><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>true</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <RunOnlyIfIdle>true</RunOnlyIfIdle>
    <IdleSettings><Duration>PT10M</Duration><WaitTimeout>PT12H</WaitTimeout><StopOnIdleEnd>false</StopOnIdleEnd><RestartOnIdle>false</RestartOnIdle></IdleSettings>
    <ExecutionTimeLimit>PT30M</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author"><Exec><Command>{}</Command><Arguments>{MEASURE_ARG}</Arguments></Exec></Actions>
</Task>
"#,
        xml_text(exe)
    )
}

/// `schtasks.exe` from `System32`, never from `PATH`.
fn schtasks() -> Option<String> {
    std::env::var("SystemRoot")
        .ok()
        .map(|w| format!(r"{w}\System32\schtasks.exe"))
}

/// Runs `schtasks.exe` with `args` and no window; true on success.
fn run(args: &[&str]) -> bool {
    let Some(exe) = schtasks() else {
        return false;
    };
    Command::new(exe)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .is_ok_and(|s| s.success())
}

/// Creates or replaces the task for the current user, pointing at this executable.
pub fn install() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let xml = task_xml(&exe.to_string_lossy());
    // schtasks reads the definition as UTF-16 with a byte-order mark.
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    let path = std::env::temp_dir().join(format!("unhoard-task-{}.xml", std::process::id()));
    if std::fs::write(&path, bytes).is_err() {
        return false;
    }
    let ok = run(&[
        "/Create",
        "/TN",
        TASK_NAME,
        "/XML",
        &path.to_string_lossy(),
        "/F",
    ]);
    let _removed = std::fs::remove_file(&path);
    ok
}

/// Removes the task; true when it is gone (also when it never existed).
pub fn remove() -> bool {
    run(&["/Delete", "/TN", TASK_NAME, "/F"]) || !exists()
}

/// True when the task exists.
pub fn exists() -> bool {
    run(&["/Query", "/TN", TASK_NAME])
}

/// Starts the task now, for a measurement right after the app opens.
pub fn run_now() -> bool {
    run(&["/Run", "/TN", TASK_NAME])
}

/// True when the measurement should not start now: battery saver is on, or a full-screen
/// app, a game or a presentation runs.
pub fn should_wait() -> bool {
    crate::win::power::battery_saver() || crate::win::power::user_busy()
}

/// The CPU time this process has used so far, in seconds.
pub fn cpu_seconds() -> f64 {
    crate::win::power::cpu_seconds()
}

/// Lowers this process to background priority (CPU, I/O and memory).
pub fn enter_background() -> bool {
    crate::win::power::enter_background()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f17_the_task_runs_this_exe_with_the_measure_argument_and_safe_settings() {
        let xml = task_xml(r"C:\Users\a&b\AppData\Local\Unhoard\unhoard-app.exe");
        assert!(xml.contains(
            r"<Command>C:\Users\a&amp;b\AppData\Local\Unhoard\unhoard-app.exe</Command>"
        ));
        assert!(xml.contains("<Arguments>--measure</Arguments>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>true</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<RunOnlyIfIdle>true</RunOnlyIfIdle>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT30M</ExecutionTimeLimit>"));
    }
}

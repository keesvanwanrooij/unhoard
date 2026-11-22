//! What the daily measurement asks of Windows before and while it runs: battery saver, a
//! full-screen app, its own CPU time, and background priority.

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::Threading::{
    GetCurrentProcess, GetProcessTimes, PROCESS_MODE_BACKGROUND_BEGIN, SetPriorityClass,
};
use windows::Win32::UI::Shell::{
    QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState,
};

/// True when battery saver is on.
pub(crate) fn battery_saver() -> bool {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid out pointer for the call.
    unsafe { GetSystemPowerStatus(&mut status) }.is_ok() && status.SystemStatusFlag == 1
}

/// True while a full-screen app, a game or a presentation runs.
pub(crate) fn user_busy() -> bool {
    // SAFETY: no arguments besides the returned state.
    match unsafe { SHQueryUserNotificationState() } {
        Ok(state) => [
            QUNS_BUSY,
            QUNS_RUNNING_D3D_FULL_SCREEN,
            QUNS_PRESENTATION_MODE,
        ]
        .contains(&state),
        Err(_) => false,
    }
}

/// The CPU time this process has used so far (kernel plus user), in seconds.
pub(crate) fn cpu_seconds() -> f64 {
    let (mut c, mut e, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: the pseudo-handle of the current process needs no closing; all four out
    // pointers are valid.
    if unsafe { GetProcessTimes(GetCurrentProcess(), &mut c, &mut e, &mut k, &mut u) }.is_err() {
        return 0.0;
    }
    let ticks = |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
    (ticks(k) + ticks(u)) as f64 / 10_000_000.0
}

/// Lowers this process to background mode: idle CPU priority and low I/O and memory
/// priority, so the measurement gives way to everything the user does.
pub(crate) fn enter_background() -> bool {
    // SAFETY: the pseudo-handle of the current process needs no closing.
    unsafe { SetPriorityClass(GetCurrentProcess(), PROCESS_MODE_BACKGROUND_BEGIN) }.is_ok()
}

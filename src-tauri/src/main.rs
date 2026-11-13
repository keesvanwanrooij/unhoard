//! The Unhoard executable. With `--reveal-helper` it runs as the elevated, read-only helper
//! and exits; otherwise it starts the app.
#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some(unhoard_platform::reveal::HELPER_ARG) {
        let code = unhoard_platform::reveal::run_helper(&args[1..]);
        return ExitCode::from(u8::try_from(code).unwrap_or(1));
    }
    match unhoard_app::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(1),
    }
}

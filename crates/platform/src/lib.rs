//! Unhoard platform layer: the Windows side of the core's contracts.
//!
//! [`WindowsFs`] implements the core's file-system trait on real volumes. [`system`]
//! reads this PC's well-known locations and opens Explorer or a Windows tool on a user
//! click. [`media`] gets thumbnails and media properties without downloads, [`reveal`]
//! runs the elevated read-only helper, [`knowledge`] gathers installed programs, program
//! history and app records, [`tools`] runs an app's read-only tool on a click, and
//! [`schedule`] manages the daily measurement's scheduled task. Only the private `win`
//! module uses `unsafe`.
#![cfg(windows)]
#![deny(unsafe_code)]

mod fs;
pub mod knowledge;
pub mod media;
pub mod ollama;
pub mod optimise;
pub mod reveal;
pub mod schedule;
pub mod system;
pub mod tools;
mod win;

pub use fs::WindowsFs;

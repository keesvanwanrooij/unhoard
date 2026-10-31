//! Unhoard platform layer: the Windows side of the core's contracts.
//!
//! [`WindowsFs`] implements the core's file-system trait on real volumes. [`system`]
//! reads this PC's well-known locations and opens Explorer or a Windows tool on a user
//! click. [`media`] gets thumbnails and media properties without downloads, and [`reveal`]
//! runs the elevated read-only helper. Only the private `win` module uses `unsafe`.
#![cfg(windows)]
#![deny(unsafe_code)]

mod fs;
pub mod media;
pub mod reveal;
pub mod system;
mod win;

pub use fs::WindowsFs;

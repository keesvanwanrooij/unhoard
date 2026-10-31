//! Unhoard core: the platform-neutral heart of Unhoard.
//!
//! It holds the scan tree and its totals, classification and the known-locations rules,
//! risk tiers and protected paths, the quarantine with its manifests, restore and purge,
//! cloud placeholder logic and photo grouping. It performs no I/O of its own: everything
//! goes through the [`fs::FileSystem`] trait, which the platform layer implements.
#![forbid(unsafe_code)]

pub mod classify;
pub mod cloud;
#[cfg(feature = "fake")]
pub mod fake;
pub mod fs;
pub mod hidden;
pub mod photos;
pub mod protect;
pub mod quarantine;
pub mod report;
pub mod roots;
pub mod rules;
pub mod scan;
pub mod time;
pub mod tree;
pub mod view;

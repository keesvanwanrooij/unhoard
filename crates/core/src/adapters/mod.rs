//! App adapters: readers for the records some apps keep about what they installed. They
//! are pure parsers over text the platform layer read from disk or from a tool, so they
//! are tested without Windows. Every input is untrusted: sizes are bounded, malformed
//! entries are skipped, and nothing parsed here is ever executed or opened.

pub mod docker;
pub mod ollama;
pub mod steam;
pub mod userassist;

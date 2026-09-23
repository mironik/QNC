//! Compatibility path for older Ingest callers.
//!
//! What a player or the background worker tells the others lives in the neutral public
//! component `qnc-playback-activity`. This crate must not grow its own implementation.

pub use qnc_playback_activity::*;

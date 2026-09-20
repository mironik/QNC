//! Playback is running: a marker other processes can read.
//!
//! The form that runs a player refreshes a small file in the temporary directory of
//! this machine while the player prepares or plays and removes it when it stops.
//! A background process treats the marker as active only while it is fresh, so a
//! crashed form never blocks background work for long. Nothing here knows a form, an
//! application or a database, and it works the same on every operating system.

use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};

pub const MODULE_ID: &str = "qnc.module.playback-marker";
pub const VERSION: &str = "0.1.0";

/// The marker is refreshed at least this often while a player works.
pub const REFRESH_EVERY: Duration = Duration::from_secs(1);

/// A marker older than this is a stale one.
pub const FRESH_FOR: Duration = Duration::from_secs(5);

fn marker_path() -> PathBuf {
    std::env::temp_dir().join("qnc-playback-active")
}

/// A player works: create or refresh the marker.
pub fn touch() {
    let _ = fs::write(marker_path(), b"");
}

/// The player stopped.
pub fn clear() {
    let _ = fs::remove_file(marker_path());
}

/// A player works right now (the marker is fresh).
pub fn is_active() -> bool {
    is_fresh(&marker_path(), FRESH_FOR)
}

fn is_fresh(path: &std::path::Path, max_age: Duration) -> bool {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age <= max_age)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_file_is_fresh_and_an_absent_file_is_not() {
        let dir = std::env::temp_dir().join("qnc-playback-marker-test");
        let _ = fs::create_dir_all(&dir);
        let file = dir.join("m");
        let _ = fs::remove_file(&file);
        assert!(!is_fresh(&file, Duration::from_secs(5)));
        fs::write(&file, b"").unwrap();
        assert!(is_fresh(&file, Duration::from_secs(5)));
        let _ = fs::remove_file(&file);
    }
}

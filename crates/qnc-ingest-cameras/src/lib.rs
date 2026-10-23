//! The cameras Ingest can read. This is the one place that names the record formats; the
//! cameras themselves are text profiles (`catalogs/cameras`, user 2026-10-23), read into the
//! camera database of this computer and handed to the adapter of their format.

use qnc_camera_adapter::CameraRegistry;
use std::{path::Path, sync::Arc};

pub const MODULE_ID: &str = "qnc.module.ingest-cameras";
pub const VERSION: &str = "0.2.0";

/// Registry of every camera adapter Ingest composes, with the camera profiles of `root`. A
/// profile file that is not valid is not taken and is said in the log; the last valid record
/// of it stays.
pub fn registry(root: &Path) -> Result<CameraRegistry, String> {
    let synced = qnc_camera_profiles::sync(root)?;
    for refused in &synced.refused {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("camera-profile refused {refused}"));
    }
    let mut registry = CameraRegistry::new();
    let sony = Arc::new(qnc_sony_metadata::SonyMediaProfile);
    registry.register(Arc::new(qnc_camera_profile_adapter::ProfileAdapter::new(sony, &synced.profiles)?))?;
    // Single files in a folder that is no card of a known camera (incoming / FTP).
    registry.register(Arc::new(qnc_camera_generic_file::GenericFile::new()))?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_reads_sony_cards_through_the_camera_profiles() {
        let root = tempfile::tempdir().unwrap();
        let profiles = root.path().join(qnc_camera_profiles::PROFILE_DIR).join("sony");
        std::fs::create_dir_all(&profiles).unwrap();
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(qnc_camera_profiles::PROFILE_DIR).join("sony");
        for entry in std::fs::read_dir(shipped).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, profiles.join(path.file_name().unwrap())).unwrap();
        }
        let registry = registry(root.path()).unwrap();
        assert!(registry.for_reader("camera.sony.index.read").is_some());
        assert!(root.path().join(qnc_camera_profiles::DATABASE).is_file(), "the profiles are kept in the camera database");
    }

    #[test]
    fn without_camera_profiles_sony_cards_are_not_read() {
        let root = tempfile::tempdir().unwrap();
        assert!(registry(root.path()).is_err());
    }
}

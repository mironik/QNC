//! The cameras Ingest can read. The record formats come from the catalog of formats
//! (`catalogs/record-formats/catalog.json`): each is built in or a plug-in program (user
//! 2026-10-23: a new format is a plug-in and one line in the catalog; the application is not
//! rebuilt). The cameras are text profiles (`catalogs/cameras`), read into the camera database
//! of this computer and handed to the adapter of their format.

use qnc_camera_adapter::{CameraRegistry, RecordFormat};
use serde::Deserialize;
use std::{path::Path, sync::Arc, time::Duration};

pub const MODULE_ID: &str = "qnc.module.ingest-cameras";
pub const VERSION: &str = "0.3.0";
/// The catalog of record formats, under the QNC root.
pub const FORMAT_CATALOG: &str = "catalogs/record-formats/catalog.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    version: u32,
    formats: Vec<Entry>,
}

/// One format (serde cannot refuse unknown fields beside a flattened kind; the kind refuses its own).
#[derive(Debug, Deserialize)]
struct Entry {
    reader_id: String,
    #[serde(flatten)]
    kind: Kind,
    #[serde(default)]
    #[allow(dead_code)]
    notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Kind {
    /// A format built into this application.
    Builtin { name: String },
    /// A format as a plug-in program beside the application.
    Plugin { program: String, contract_version: u32, timeout_ms: u64 },
}

/// The record formats built into this application, by their catalog name.
fn builtin(name: &str) -> Option<Arc<dyn RecordFormat>> {
    match name {
        "sony-mediaprofile" => Some(Arc::new(qnc_sony_metadata::SonyMediaProfile)),
        _ => None,
    }
}

fn format(entry: &Entry) -> Result<Arc<dyn RecordFormat>, String> {
    let format: Arc<dyn RecordFormat> = match &entry.kind {
        Kind::Builtin { name } => builtin(name).ok_or_else(|| format!("no built-in record format '{name}'"))?,
        Kind::Plugin { program, contract_version, timeout_ms } => {
            let spec = qnc_plugin_process::PluginSpec {
                executable: qnc_plugin_process::beside_application(program)?,
                contract: qnc_record_format_plugin::CONTRACT.into(),
                version: *contract_version,
                timeout: Duration::from_millis(*timeout_ms),
            };
            Arc::new(qnc_record_format_plugin::PluginFormat::start(spec)?)
        }
    };
    let reader = format.index().reader_id();
    if reader != entry.reader_id {
        return Err(format!("the record format of '{}' reads '{reader}'", entry.reader_id));
    }
    Ok(format)
}

/// Registry of every camera adapter Ingest composes: the record formats of `root`'s catalog with
/// its camera profiles. A profile file or a format that cannot be used is not taken and is said
/// in the log; the last valid record of a profile stays.
pub fn registry(root: &Path) -> Result<CameraRegistry, String> {
    let synced = qnc_camera_profiles::sync(root)?;
    for refused in &synced.refused {
        log(format!("camera-profile refused {refused}"));
    }
    let text = std::fs::read_to_string(root.join(FORMAT_CATALOG)).map_err(|e| format!("{FORMAT_CATALOG}: {e}"))?;
    let catalog: Catalog = serde_json::from_str(&text).map_err(|e| format!("{FORMAT_CATALOG}: {e}"))?;
    if catalog.version != 1 {
        return Err(format!("{FORMAT_CATALOG}: version {} is not 1", catalog.version));
    }
    let mut registry = CameraRegistry::new();
    for entry in &catalog.formats {
        let adapter = format(entry).and_then(|format| qnc_camera_profile_adapter::ProfileAdapter::new(format, &synced.profiles));
        match adapter {
            Ok(adapter) => registry.register(Arc::new(adapter))?,
            Err(error) => log(format!("record-format refused {}: {error}", entry.reader_id)),
        }
    }
    if registry.is_empty() {
        return Err(format!("{FORMAT_CATALOG}: no record format can be used"));
    }
    // Single files in a folder that is no card of a known camera (incoming / FTP).
    registry.register(Arc::new(qnc_camera_generic_file::GenericFile::new()))?;
    Ok(registry)
}

fn log(line: String) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A QNC root with the profiles and the catalog of formats shipped with QNC.
    fn shipped_root() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let shipped = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let profiles = root.path().join(qnc_camera_profiles::PROFILE_DIR).join("sony");
        std::fs::create_dir_all(&profiles).unwrap();
        for entry in std::fs::read_dir(shipped.join(qnc_camera_profiles::PROFILE_DIR).join("sony")).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, profiles.join(path.file_name().unwrap())).unwrap();
        }
        std::fs::create_dir_all(root.path().join(FORMAT_CATALOG).parent().unwrap()).unwrap();
        std::fs::copy(shipped.join(FORMAT_CATALOG), root.path().join(FORMAT_CATALOG)).unwrap();
        root
    }

    #[test]
    fn the_registry_reads_sony_cards_through_the_catalog_and_the_camera_profiles() {
        let root = shipped_root();
        let registry = registry(root.path()).unwrap();
        assert!(registry.for_reader("camera.sony.index.read").is_some());
        assert!(root.path().join(qnc_camera_profiles::DATABASE).is_file(), "the profiles are kept in the camera database");
    }

    #[test]
    fn without_camera_profiles_or_with_an_unknown_format_sony_cards_are_not_read() {
        let root = shipped_root();
        std::fs::remove_dir_all(root.path().join(qnc_camera_profiles::PROFILE_DIR)).unwrap();
        assert!(registry(root.path()).is_err(), "no profile: no default profile of the format");
        let root = shipped_root();
        let catalog = std::fs::read_to_string(root.path().join(FORMAT_CATALOG)).unwrap().replace("\"sony-mediaprofile\"", "\"sony-unknown\"");
        std::fs::write(root.path().join(FORMAT_CATALOG), catalog).unwrap();
        assert!(registry(root.path()).is_err());
    }
}

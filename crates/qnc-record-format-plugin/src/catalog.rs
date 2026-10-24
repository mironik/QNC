//! The catalog of record formats (`catalogs/record-formats/catalog.json`, moved unchanged from
//! the Ingest cameras): each format is built into the application or a plug-in program.

use serde::Deserialize;
use std::path::Path;

/// The catalog of record formats, under the QNC root.
pub const FORMAT_CATALOG: &str = "catalogs/record-formats/catalog.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormatCatalog {
    pub version: u32,
    pub formats: Vec<FormatEntry>,
}

/// One format (serde cannot refuse unknown fields beside a flattened kind; the kind refuses its own).
#[derive(Debug, Deserialize)]
pub struct FormatEntry {
    pub reader_id: String,
    #[serde(flatten)]
    pub kind: FormatKind,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FormatKind {
    /// A format built into this application.
    Builtin { name: String },
    /// A format as a plug-in program beside the application.
    Plugin { program: String, contract_version: u32, timeout_ms: u64 },
}

impl FormatCatalog {
    /// The catalog of `root`; only version 1 is read.
    pub fn read(root: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(root.join(FORMAT_CATALOG)).map_err(|e| format!("{FORMAT_CATALOG}: {e}"))?;
        let catalog: Self = serde_json::from_str(&text).map_err(|e| format!("{FORMAT_CATALOG}: {e}"))?;
        if catalog.version != 1 {
            return Err(format!("{FORMAT_CATALOG}: version {} is not 1", catalog.version));
        }
        Ok(catalog)
    }
}

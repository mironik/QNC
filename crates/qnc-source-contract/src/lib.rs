//! Pure QNC source references and transport data. No I/O or application dependencies.
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadError {
    InvalidReference,
    UnboundSource,
    NotFound,
    NotFile,
    UnsupportedType,
    AccessDenied,
    TooLarge,
    InvalidUtf8,
    Changed,
    TransportConfig,
    Unavailable,
    Protocol,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidReference => "Invalid QNC source reference",
            Self::UnboundSource => "Source is not bound to this reader",
            Self::NotFound => "Source entry not found",
            Self::NotFile => "Source entry is not a regular file",
            Self::UnsupportedType => "Unsupported source entry type",
            Self::AccessDenied => "Source access denied",
            Self::TooLarge => "Source document exceeds the read limit",
            Self::InvalidUtf8 => "Source document is not UTF-8",
            Self::Changed => "Source document changed during reading",
            Self::TransportConfig => "Invalid source transport configuration",
            Self::Unavailable => "Source transport unavailable",
            Self::Protocol => "Invalid source transport response",
        };
        f.write_str(message)
    }
}

impl std::error::Error for ReadError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceReference {
    source_uri: String,
    relative_path: String,
}

impl SourceReference {
    pub fn new(source_uri: &str, relative_path: &str) -> Result<Self, ReadError> {
        let reference = Self {
            source_uri: source_uri.into(),
            relative_path: relative_path.into(),
        };
        reference.validate()?;
        Ok(reference)
    }

    pub fn source_uri(&self) -> &str {
        &self.source_uri
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// Resolve a decoded recording-relative reference without accessing a filesystem.
    pub fn descendant(&self, relative_path: &str) -> Result<Self, ReadError> {
        self.validate()?;
        SourceReference::new(self.source_uri(), relative_path)?;
        let path = if self.relative_path == "." {
            relative_path.to_owned()
        } else if relative_path == "." {
            self.relative_path.clone()
        } else {
            format!("{}/{relative_path}", self.relative_path)
        };
        Self::new(self.source_uri(), &path)
    }

    pub fn is_within(&self, root: &Self) -> bool {
        self.validate().is_ok()
            && root.validate().is_ok()
            && self.source_uri == root.source_uri
            && (root.relative_path == "."
                || self.relative_path == root.relative_path
                || self
                    .relative_path
                    .starts_with(&format!("{}/", root.relative_path)))
    }

    pub fn uri(&self) -> String {
        if self.relative_path == "." {
            return self.source_uri.clone();
        }
        let encoded = self
            .relative_path
            .split('/')
            .map(|s| utf8_percent_encode(s, NON_ALPHANUMERIC).to_string())
            .collect::<Vec<_>>()
            .join("/");
        format!("{}/file/{encoded}", self.source_uri)
    }

    pub fn validate(&self) -> Result<(), ReadError> {
        validate_source_uri(&self.source_uri)?;
        if self.relative_path == "." {
            return Ok(());
        }
        if self.relative_path.len() > 4096 {
            return Err(ReadError::InvalidReference);
        }
        for part in self.relative_path.split('/') {
            if part.is_empty()
                || matches!(part, "." | "..")
                || part.trim() != part
                || part.ends_with('.')
                || part.contains(['\\', ':', '%', '?', '#', '*', '"', '<', '>', '|'])
                || part.chars().any(char::is_control)
            {
                return Err(ReadError::InvalidReference);
            }
            // Reject Win32 device aliases on every OS for identical public semantics.
            let stem = part
                .split('.')
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                || ["COM", "LPT"].iter().any(|prefix| {
                    stem.strip_prefix(prefix).is_some_and(|tail| {
                        tail.len() == 1 && tail.bytes().all(|b| (b'1'..=b'9').contains(&b))
                    })
                })
            {
                return Err(ReadError::InvalidReference);
            }
        }
        Ok(())
    }
}

pub fn validate_source_uri(uri: &str) -> Result<qnc_contracts::QncUri, ReadError> {
    let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|_| ReadError::InvalidReference)?;
    fn identifier(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 256
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    }
    if uri != uri.trim()
        || parsed.resource_kind != "source"
        || !identifier(&parsed.resource_id)
        || parsed.authority.as_ref().is_some_and(|a| !identifier(a))
    {
        return Err(ReadError::InvalidReference);
    }
    Ok(parsed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    File,
    Directory,
    Link,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MatchCase {
    #[default]
    Exact,
    Insensitive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryEntry {
    pub name: String,
    pub reference: SourceReference,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectoryListing {
    pub directory_uri: String,
    pub match_case: MatchCase,
    pub entries: Vec<DirectoryEntry>,
}

impl DirectoryListing {
    pub fn validate(
        &self,
        reference: &SourceReference,
        max_entries: usize,
    ) -> Result<(), ReadError> {
        if self.directory_uri != reference.uri() || self.entries.len() > max_entries {
            return Err(ReadError::Protocol);
        }
        let mut names = std::collections::BTreeSet::new();
        for entry in &self.entries {
            if entry.name.contains('/') || !names.insert(&entry.name) {
                return Err(ReadError::Protocol);
            }
            let relative = if reference.relative_path() == "." {
                entry.name.clone()
            } else {
                format!("{}/{}", reference.relative_path(), entry.name)
            };
            let expected = SourceReference::new(reference.source_uri(), &relative)?;
            if entry.reference != expected || matches!(entry.name.as_str(), "" | "." | "..") {
                return Err(ReadError::Protocol);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileInfo {
    pub uri: String,
    pub kind: EntryKind,
    pub byte_len: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextDocument {
    pub info: FileInfo,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinaryDocument {
    pub info: FileInfo,
    pub bytes: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_references_remain_source_scoped_and_os_neutral() {
        for uri in [
            "qnc://local/source/card",
            "qnc://lan/storage/source/card",
            "qnc://intranet/storage/source/card",
        ] {
            let source = SourceReference::new(uri, ".").unwrap();
            let root = source.descendant("PRIVATE/XDROOT").unwrap();
            let file = root.descendant("Clip/TEST A.MXF").unwrap();
            assert!(file.is_within(&root));
            assert_eq!(file.relative_path(), "PRIVATE/XDROOT/Clip/TEST A.MXF");
            assert!(file.uri().ends_with("Clip/TEST%20A%2EMXF"));
            assert!(!source
                .descendant("PRIVATE/XDROOT-other/A.MXF")
                .unwrap()
                .is_within(&root));
            for path in ["../A", "C:/A", "Clip\\A", "Clip/%2e%2e/A"] {
                assert!(root.descendant(path).is_err());
            }
        }
    }

    #[test]
    fn reference_wire_format_is_preserved_but_untrusted_fields_are_revalidated() {
        let valid = SourceReference::new("qnc://local/source/card", "Clip/A.MXF").unwrap();
        let mut json = serde_json::to_value(&valid).unwrap();
        assert_eq!(
            serde_json::from_value::<SourceReference>(json.clone()).unwrap(),
            valid
        );
        json["relative_path"] = serde_json::json!("../A.MXF");
        let bad: SourceReference = serde_json::from_value(json).unwrap();
        assert!(bad.validate().is_err());
        assert!(!bad.is_within(&SourceReference::new(valid.source_uri(), ".").unwrap()));
    }

    #[test]
    fn manifest_only_exposes_data_validation() {
        assert!(qnc_contracts::validate_module_manifest_json(
            "source-contract",
            include_str!("../../../contracts/modules/source-contract.module.json")
        )
        .is_ok());
    }
}

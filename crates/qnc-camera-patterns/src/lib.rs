//! Read-only projection of the published camera-pattern database.
mod database;
mod transport;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub use transport::{read_uri, respond, ENDPOINT};

pub const ID: &str = "qnc.catalog.camera-patterns";
pub const VERSION: &str = "0.1.0";
pub const MAX_BYTES: u64 = 8 * 1024 * 1024;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub kind: String,
    pub publisher: String,
    pub title: String,
    pub locator: String,
    pub section: String,
    pub reviewed_on: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileRule {
    pub path: String,
    pub role: String,
    pub condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataField {
    pub document: String,
    pub namespace: String,
    pub selector: String,
    pub meaning: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: String,
    pub supports: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pattern {
    pub id: String,
    pub manufacturer: String,
    pub family: String,
    pub applicability: String,
    pub naming_rule: String,
    pub evidence_level: String,
    pub status: String,
    pub status_reason: String,
    pub root_scope: String,
    pub grouping_method: String,
    pub grouping_notes: String,
    pub limitations: String,
    pub roots: Vec<String>,
    pub files: Vec<FileRule>,
    pub metadata: Vec<MetadataField>,
    pub evidence: Vec<Evidence>,
}

impl Pattern {
    pub fn analysis_candidate(&self) -> bool {
        self.status == "enabled"
            && matches!(self.evidence_level.as_str(), "documented" | "observed")
            && self.root_scope != "unknown"
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageGap {
    pub id: String,
    pub manufacturer: String,
    pub family: String,
    pub missing_evidence: String,
    pub source: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub catalog_uri: String,
    pub contract_version: String,
    pub catalog_id: String,
    pub schema_version: u32,
    pub dataset_version: String,
    pub reviewed_on: String,
    pub runtime_status: String,
    pub patterns: Vec<Pattern>,
    pub sources: Vec<Source>,
    pub gaps: Vec<CoverageGap>,
}

impl Catalog {
    pub fn validate(&self) -> Result<()> {
        validate_uri(&self.catalog_uri)?;
        if self.catalog_id != ID
            || self.contract_version != VERSION
            || self.schema_version != 1
            || self.runtime_status != "research_only"
        {
            return Err("unsupported camera catalog identity or semantics".into());
        }
        required(&self.dataset_version)?;
        required(&self.reviewed_on)?;
        if self.patterns.len() > 1024 || self.sources.len() > 4096 || self.gaps.len() > 4096 {
            return Err("catalog row limit exceeded".into());
        }
        let mut ids = BTreeSet::new();
        for source in &self.sources {
            if !ids.insert(&source.id) {
                return Err("duplicate source identity".into());
            }
            for s in [
                &source.id,
                &source.publisher,
                &source.title,
                &source.section,
                &source.reviewed_on,
            ] {
                required(s)?;
            }
            match source.kind.as_str() {
                "manufacturer_document" if source.locator.starts_with("https://") => {
                    required(&source.locator)?;
                }
                "card_observation" => {
                    required(&source.locator)?;
                    let (path, fragment) = source
                        .locator
                        .split_once('#')
                        .map_or((source.locator.as_str(), None), |(p, f)| (p, Some(f)));
                    validate_expression(path)?;
                    if path.contains(['*', '?'])
                        || fragment.is_some_and(|f| {
                            f.is_empty() || f.contains('#') || f.chars().any(char::is_control)
                        })
                    {
                        return Err("invalid observation document reference".into());
                    }
                }
                _ => return Err("invalid catalog evidence source".into()),
            }
        }
        let mut ids = BTreeSet::new();
        for p in &self.patterns {
            if !ids.insert(&p.id) {
                return Err("duplicate pattern identity".into());
            }
            for s in [
                &p.id,
                &p.manufacturer,
                &p.family,
                &p.applicability,
                &p.naming_rule,
                &p.status_reason,
                &p.grouping_notes,
                &p.limitations,
            ] {
                required(s)?;
            }
            if !matches!(p.status.as_str(), "enabled" | "disabled" | "incorrect")
                || !matches!(
                    p.evidence_level.as_str(),
                    "documented" | "observed" | "partial"
                )
                || !matches!(
                    p.root_scope.as_str(),
                    "card_relative" | "recording_relative" | "reel_relative" | "unknown"
                )
                || !matches!(
                    p.grouping_method.as_str(),
                    "manifest_references"
                        | "metadata_identity"
                        | "directory_segments"
                        | "paired_names"
                        | "unresolved"
                )
            {
                return Err("unsupported pattern semantics".into());
            }
            if p.roots.len() > 64
                || p.files.len() > 128
                || p.metadata.len() > 512
                || p.evidence.is_empty()
                || p.evidence.len() > 64
            {
                return Err("invalid pattern size/evidence".into());
            }
            if p.root_scope == "unknown" && !p.roots.is_empty() {
                return Err("unknown scope cannot declare roots".into());
            }
            unique(p.roots.iter().map(String::as_str))?;
            unique(p.files.iter().map(|f| (&f.path, &f.role)))?;
            unique(p.metadata.iter().map(|m| (&m.document, &m.selector)))?;
            unique(p.evidence.iter().map(|e| &e.source))?;
            for root in &p.roots {
                validate_expression(root)?;
            }
            for f in &p.files {
                validate_expression(&f.path)?;
                required(&f.condition)?;
                if !matches!(
                    f.role.as_str(),
                    "original_candidate"
                        | "proxy_candidate"
                        | "preview"
                        | "thumbnail"
                        | "metadata"
                        | "index"
                        | "audio_component"
                        | "companion_data"
                ) {
                    return Err("unknown file role".into());
                }
            }
            for m in &p.metadata {
                validate_expression(&m.document)?;
                required(&m.namespace)?;
                required(&m.selector)?;
                required(&m.meaning)?;
            }
            let mut documented = false;
            let mut observed = false;
            for e in &p.evidence {
                required(&e.supports)?;
                let source = self
                    .sources
                    .iter()
                    .find(|s| s.id == e.source)
                    .ok_or("missing evidence source")?;
                documented |= source.kind == "manufacturer_document";
                observed |= source.kind == "card_observation";
            }
            if (p.evidence_level == "documented" && !documented)
                || (p.evidence_level == "observed" && !observed)
                || (p.status == "enabled"
                    && (!documented
                        || p.evidence_level == "partial"
                        || p.root_scope == "unknown"
                        || p.roots.is_empty()
                        || !p.files.iter().any(|f| f.role == "original_candidate")))
            {
                return Err("pattern evidence does not support enabled analysis".into());
            }
        }
        unique(self.gaps.iter().map(|g| &g.id))?;
        for gap in &self.gaps {
            for s in [
                &gap.id,
                &gap.manufacturer,
                &gap.family,
                &gap.missing_evidence,
            ] {
                required(s)?;
            }
            if gap.status != "research_pending"
                || gap
                    .source
                    .as_ref()
                    .is_some_and(|id| !self.sources.iter().any(|s| &s.id == id))
            {
                return Err("invalid coverage gap".into());
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| "catalog encoding failed")?
            .len() as u64
            > MAX_BYTES
        {
            return Err("catalog size limit exceeded".into());
        }
        Ok(())
    }
}

fn required(s: &str) -> Result<()> {
    if s.trim().is_empty() || s.len() > 4096 || s.contains('\0') {
        Err("invalid catalog text field".into())
    } else {
        Ok(())
    }
}

fn unique<T: Ord>(values: impl Iterator<Item = T>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err("duplicate catalog entry".into());
        }
    }
    Ok(())
}

pub fn validate_expression(value: &str) -> Result<()> {
    required(value)?;
    if value == "." {
        return Ok(());
    }
    if value.contains(['\\', ':', '%', '#', '[', ']', '{', '}', '"', '<', '>', '|'])
        || value.chars().any(char::is_control)
        || value.split('/').any(|s| {
            matches!(s, "" | "." | "..") || s.trim() != s || (s.contains("**") && s != "**")
        })
    {
        return Err("unsafe or unsupported catalog path expression".into());
    }
    Ok(())
}

pub(crate) fn validate_uri(uri: &str) -> Result<qnc_contracts::QncUri> {
    let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|_| "invalid catalog URI")?;
    if uri.trim() != uri
        || parsed.resource_kind != "catalog"
        || parsed.resource_id != "camera-patterns"
        || parsed.authority.as_ref().is_some_and(|a| {
            a.is_empty()
                || a.len() > 256
                || !a
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
    {
        return Err("invalid catalog URI".into());
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests;

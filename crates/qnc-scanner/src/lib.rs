//! Read-only role scanning with caller-supplied public recording readers.
use qnc_camera_detector::{detect, DetectionReport, Limits, SourceScope};
use qnc_camera_patterns::Catalog;
use qnc_source_groups::{
    assemble, FileFact, FileState, GroupProposal, GroupReport, IndexDocument, IndexReader,
    MAX_FILES, MAX_GROUPS,
};
use qnc_source_reader::{EntryKind, ReadError, SourceReader, SourceReference, MAX_TEXT_BYTES};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub struct ScanLimits {
    pub detection: Limits,
    pub max_indexes: usize,
    pub max_groups: usize,
    pub max_file_checks: usize,
}
impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            detection: Limits::default(),
            max_indexes: 64,
            max_groups: MAX_GROUPS,
            max_file_checks: MAX_FILES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanIssueCode {
    ReaderUnavailable,
    ReaderAmbiguous,
    IndexUnreadable,
    IndexInvalid,
    FileUnavailable,
    LimitExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanIssue {
    pub code: ScanIssueCode,
    pub reference: SourceReference,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnresolvedFile {
    pub reference: SourceReference,
    pub candidate_roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanReport {
    pub contract_version: String,
    pub detection: DetectionReport,
    pub grouping: GroupReport,
    pub file_facts: Vec<FileFact>,
    pub issues: Vec<ScanIssue>,
    pub unresolved_files: Vec<UnresolvedFile>,
    pub indexes_read: usize,
}

impl ScanReport {
    /// Relationship completeness only. This says nothing about media/probe completeness.
    pub fn relationships_resolved(&self) -> bool {
        self.detection.traversal_complete
            && self.issues.is_empty()
            && self.grouping.blocked.is_empty()
            && self.unresolved_files.is_empty()
    }
}

pub fn scan_roles(
    catalog: &Catalog,
    source: &SourceReader,
    scope: SourceScope,
    readers: &[&dyn IndexReader],
    limits: ScanLimits,
) -> Result<ScanReport, String> {
    if limits.max_indexes == 0
        || limits.max_indexes > 256
        || limits.max_groups == 0
        || limits.max_groups > MAX_GROUPS
        || limits.max_file_checks == 0
        || limits.max_file_checks > MAX_FILES
    {
        return Err("invalid scanner limits".into());
    }
    let mut ids = BTreeSet::new();
    for reader in readers {
        if reader.reader_id().trim().is_empty()
            || reader.namespace().trim().is_empty()
            || !ids.insert(reader.reader_id())
        {
            return Err("invalid or duplicate index reader identity".into());
        }
    }
    let detection = detect(catalog, source, scope, limits.detection)?;
    let mut issues = vec![];
    let mut proposals = vec![];
    let mut facts = BTreeMap::<String, FileFact>::new();
    let mut indexes_read = 0;
    let mut parsed_indexes = BTreeSet::new();
    let mut consumed_indexes = BTreeSet::new();
    for root in &detection.roots {
        let pattern = catalog
            .patterns
            .iter()
            .find(|p| p.id == root.pattern_id)
            .ok_or("detector returned unknown pattern")?;
        for index in root.files.iter().filter(|f| f.role == "index") {
            let namespaces: BTreeSet<_> = pattern
                .metadata
                .iter()
                .filter(|m| index.rules.iter().any(|r| r.pattern == m.document))
                .map(|m| m.namespace.as_str())
                .collect();
            let matching: Vec<_> = readers
                .iter()
                .filter(|r| namespaces.contains(r.namespace()))
                .collect();
            if matching.len() != 1 {
                issues.push(ScanIssue {
                    code: if matching.is_empty() {
                        ScanIssueCode::ReaderUnavailable
                    } else {
                        ScanIssueCode::ReaderAmbiguous
                    },
                    reference: index.reference.clone(),
                    message: "index requires one registered reader for its catalog namespace"
                        .into(),
                });
                continue;
            }
            if !parsed_indexes.insert((
                root.root.uri(),
                index.reference.uri(),
                matching[0].reader_id().to_string(),
            )) {
                continue;
            }
            if indexes_read == limits.max_indexes {
                issues.push(ScanIssue {
                    code: ScanIssueCode::LimitExceeded,
                    reference: index.reference.clone(),
                    message: "index read limit exceeded".into(),
                });
                continue;
            }
            indexes_read += 1;
            let document = match source.read_text(&index.reference, MAX_TEXT_BYTES) {
                Ok(document) => document,
                Err(error) => {
                    issues.push(ScanIssue {
                        code: ScanIssueCode::IndexUnreadable,
                        reference: index.reference.clone(),
                        message: error.to_string(),
                    });
                    continue;
                }
            };
            let parsed = matching[0]
                .read(
                    &root.root,
                    &IndexDocument {
                        reference: index.reference.clone(),
                        text: document.text,
                    },
                )
                .and_then(|groups| {
                    if groups.len() + proposals.len() > limits.max_groups {
                        return Err("group proposal limit exceeded".into());
                    }
                    for group in &groups {
                        group.validate(source.source_uri())?;
                        if group.root != root.root
                            || group.evidence.document != index.reference
                            || group.evidence.reader_id != matching[0].reader_id()
                        {
                            return Err(
                                "index reader returned a foreign root/evidence identity".into()
                            );
                        }
                    }
                    Ok(groups)
                });
            match parsed {
                Ok(groups) => {
                    if !facts.contains_key(&index.reference.uri())
                        && facts.len() >= limits.max_file_checks
                    {
                        return Err("file verification limit exceeded".into());
                    }
                    facts.insert(
                        index.reference.uri(),
                        FileFact {
                            reference: index.reference.clone(),
                            state: FileState::File,
                        },
                    );
                    consumed_indexes.insert(index.reference.uri());
                    proposals.extend(groups);
                }
                Err(error) => issues.push(ScanIssue {
                    code: ScanIssueCode::IndexInvalid,
                    reference: index.reference.clone(),
                    message: error,
                }),
            }
        }
    }
    // Check each referenced file once, including files explicitly indexed outside suffix hints.
    for reference in proposals.iter().flat_map(GroupProposal::references) {
        if facts.contains_key(&reference.uri()) {
            continue;
        }
        if facts.len() >= limits.max_file_checks {
            return Err("file verification limit exceeded; no grouping result published".into());
        }
        let status = source.stat(reference).and_then(|info| {
            if info.kind == EntryKind::File {
                Ok(())
            } else {
                Err(ReadError::NotFile)
            }
        });
        let state = match status {
            Ok(()) => FileState::File,
            Err(error) => {
                issues.push(ScanIssue {
                    code: ScanIssueCode::FileUnavailable,
                    reference: reference.clone(),
                    message: error.to_string(),
                });
                if error == ReadError::NotFound {
                    FileState::Missing
                } else {
                    FileState::Unavailable
                }
            }
        };
        facts.insert(
            reference.uri(),
            FileFact {
                reference: reference.clone(),
                state,
            },
        );
    }
    let file_facts: Vec<_> = facts.into_values().collect();
    let grouping = assemble(source.source_uri(), proposals, &file_facts)?;
    let used: BTreeSet<_> = grouping
        .groups
        .iter()
        .flat_map(|g| g.proposal.references())
        .map(SourceReference::uri)
        .chain(consumed_indexes)
        .collect();
    let mut remaining = BTreeMap::<String, (SourceReference, BTreeSet<String>)>::new();
    for file in detection.roots.iter().flat_map(|r| &r.files) {
        if !used.contains(&file.reference.uri()) {
            remaining
                .entry(file.reference.uri())
                .or_insert_with(|| (file.reference.clone(), BTreeSet::new()))
                .1
                .insert(file.role.clone());
        }
    }
    let unresolved_files = remaining
        .into_values()
        .map(|(reference, roles)| UnresolvedFile {
            reference,
            candidate_roles: roles.into_iter().collect(),
        })
        .collect();
    Ok(ScanReport {
        contract_version: "0.1.0".into(),
        detection,
        grouping,
        file_facts,
        issues,
        unresolved_files,
        indexes_read,
    })
}

#[cfg(test)]
mod tests;

//! Pure grouping of explicit recording relationships and caller-provided file facts.
use qnc_source_contract::SourceReference;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CONTRACT_ID: &str = "qnc.source.groups";
pub const CONTRACT_VERSION: &str = "0.1.0";
pub const MAX_GROUPS: usize = 10_000;
pub const MAX_FILES: usize = 50_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexDocument {
    pub reference: SourceReference,
    pub text: String,
}

/// Stateless parser capability. An implementation receives data, never an I/O handle.
pub trait IndexReader: Send + Sync {
    fn reader_id(&self) -> &str;
    fn namespace(&self) -> &str;
    fn read(
        &self,
        root: &SourceReference,
        document: &IndexDocument,
    ) -> Result<Vec<GroupProposal>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupEvidence {
    pub reader_id: String,
    pub document: SourceReference,
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelatedReference {
    pub reference: SourceReference,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupProposal {
    pub root: SourceReference,
    pub recording_identity: String,
    pub evidence: GroupEvidence,
    pub original: SourceReference,
    pub proxies: Vec<SourceReference>,
    pub related: Vec<RelatedReference>,
}

impl GroupProposal {
    pub fn references(&self) -> impl Iterator<Item = &SourceReference> {
        std::iter::once(&self.original)
            .chain(&self.proxies)
            .chain(self.related.iter().map(|r| &r.reference))
            .chain(std::iter::once(&self.evidence.document))
    }

    pub fn validate(&self, source_uri: &str) -> Result<(), String> {
        self.root.validate().map_err(|e| e.to_string())?;
        if self.root.source_uri() != source_uri
            || self.proxies.len() > 64
            || self.related.len() > 128
        {
            return Err("invalid group source or size".into());
        }
        for text in [
            &self.recording_identity,
            &self.evidence.reader_id,
            &self.evidence.locator,
        ] {
            valid_text(text)?;
        }
        for related in &self.related {
            valid_text(&related.kind)?;
        }
        for reference in self.references() {
            if !reference.is_within(&self.root) || reference == &self.root {
                return Err("group file reference escapes recording root".into());
            }
        }
        Ok(())
    }
}

fn valid_text(text: &str) -> Result<(), String> {
    if text.trim().is_empty() || text.len() > 4096 || text.chars().any(char::is_control) {
        Err("invalid group text field".into())
    } else {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    File,
    Missing,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileFact {
    pub reference: SourceReference,
    pub state: FileState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupIssue {
    DuplicateIdentity,
    SharedMedia,
    ConflictingRoles,
    OriginalUnavailable,
    ProxyUnavailable,
    EvidenceUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockedGroup {
    pub proposal: GroupProposal,
    pub issues: Vec<GroupIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceGroup {
    pub proposal: GroupProposal,
    pub related_states: Vec<FileFact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupReport {
    pub contract_id: String,
    pub contract_version: String,
    pub source_uri: String,
    pub groups: Vec<SourceGroup>,
    pub blocked: Vec<BlockedGroup>,
}

/// No clip IDs, file access, media probing, or inferred filename relationships.
pub fn assemble(
    source_uri: &str,
    proposals: Vec<GroupProposal>,
    facts: &[FileFact],
) -> Result<GroupReport, String> {
    SourceReference::new(source_uri, ".").map_err(|e| e.to_string())?;
    if proposals.len() > MAX_GROUPS || facts.len() > MAX_FILES {
        return Err("grouping limit exceeded".into());
    }
    let mut states = BTreeMap::new();
    for fact in facts {
        fact.reference.validate().map_err(|e| e.to_string())?;
        if fact.reference.source_uri() != source_uri || fact.reference.relative_path() == "." {
            return Err("invalid file fact source".into());
        }
        if states.insert(fact.reference.uri(), fact.state).is_some() {
            return Err("duplicate file fact".into());
        }
    }
    let mut identities = BTreeMap::<(String, String), usize>::new();
    let mut media_owners = BTreeMap::<String, BTreeSet<usize>>::new();
    let mut support = BTreeSet::new();
    for (i, p) in proposals.iter().enumerate() {
        p.validate(source_uri)?;
        *identities
            .entry((p.root.uri(), p.recording_identity.clone()))
            .or_default() += 1;
        for file in std::iter::once(&p.original).chain(&p.proxies) {
            media_owners.entry(file.uri()).or_default().insert(i);
        }
        support.insert(p.evidence.document.uri());
        support.extend(p.related.iter().map(|r| r.reference.uri()));
    }
    let state = |r: &SourceReference| {
        states
            .get(&r.uri())
            .copied()
            .unwrap_or(FileState::Unavailable)
    };
    let mut report = GroupReport {
        contract_id: CONTRACT_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        source_uri: source_uri.into(),
        groups: vec![],
        blocked: vec![],
    };
    for p in proposals {
        let mut issues = vec![];
        if identities[&(p.root.uri(), p.recording_identity.clone())] > 1 {
            issues.push(GroupIssue::DuplicateIdentity);
        }
        let media: Vec<_> = std::iter::once(&p.original).chain(&p.proxies).collect();
        if media.iter().any(|r| media_owners[&r.uri()].len() > 1) {
            issues.push(GroupIssue::SharedMedia);
        }
        let unique_media: BTreeSet<_> = media.iter().map(|r| r.uri()).collect();
        let related: BTreeSet<_> = p.related.iter().map(|r| r.reference.uri()).collect();
        if unique_media.len() != media.len()
            || related.len() != p.related.len()
            || media.iter().any(|r| support.contains(&r.uri()))
            || related.iter().any(|r| media_owners.contains_key(r))
            || media_owners.contains_key(&p.evidence.document.uri())
        {
            issues.push(GroupIssue::ConflictingRoles);
        }
        if state(&p.original) != FileState::File {
            issues.push(GroupIssue::OriginalUnavailable);
        }
        if p.proxies.iter().any(|r| state(r) != FileState::File) {
            issues.push(GroupIssue::ProxyUnavailable);
        }
        if state(&p.evidence.document) != FileState::File {
            issues.push(GroupIssue::EvidenceUnavailable);
        }
        if issues.is_empty() {
            let related_states = p
                .related
                .iter()
                .map(|r| FileFact {
                    reference: r.reference.clone(),
                    state: state(&r.reference),
                })
                .collect();
            report.groups.push(SourceGroup {
                proposal: p,
                related_states,
            });
        } else {
            report.blocked.push(BlockedGroup {
                proposal: p,
                issues,
            });
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests;

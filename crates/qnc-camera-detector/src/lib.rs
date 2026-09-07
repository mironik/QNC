//! Catalog-driven location/role candidates. No media probe, XML parsing or clip creation.
use globset::{GlobBuilder, GlobMatcher};
use qnc_camera_patterns::{validate_expression, Catalog};
use qnc_source_reader::{
    DirectoryListing, EntryKind, MatchCase, SourceReader, SourceReference, MAX_DIRECTORY_ENTRIES,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceScope {
    CardRelative,
    RecordingRelative,
    ReelRelative,
}

impl SourceScope {
    fn id(self) -> &'static str {
        match self {
            Self::CardRelative => "card_relative",
            Self::RecordingRelative => "recording_relative",
            Self::ReelRelative => "reel_relative",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_depth: usize,
    pub max_directories: usize,
    pub max_matches: usize,
    pub max_steps: usize,
    pub max_entries_per_directory: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_depth: 16,
            max_directories: 512,
            max_matches: 50_000,
            max_steps: 1_000_000,
            max_entries_per_directory: MAX_DIRECTORY_ENTRIES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleCandidate {
    pub reference: SourceReference,
    pub role: String,
    pub rules: Vec<MatchedRule>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedRule {
    pub pattern: String,
    pub condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootFinding {
    pub pattern_id: String,
    pub root: SourceReference,
    pub files: Vec<RoleCandidate>,
}

impl RootFinding {
    pub fn has_original_candidates(&self) -> bool {
        self.files.iter().any(|f| f.role == "original_candidate")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exclusion {
    pub pattern_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub pattern_id: String,
    pub expression: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Claim {
    pub pattern_id: String,
    pub root_uri: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ambiguity {
    pub file_uri: String,
    pub claims: Vec<Claim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectionReport {
    pub catalog_uri: String,
    pub dataset_version: String,
    pub source_uri: String,
    pub scope: SourceScope,
    pub traversal_complete: bool,
    pub directories_listed: usize,
    pub roots: Vec<RootFinding>,
    pub exclusions: Vec<Exclusion>,
    pub issues: Vec<Issue>,
    pub ambiguities: Vec<Ambiguity>,
    pub coverage_gap_ids: Vec<String>,
}

pub fn detect(
    catalog: &Catalog,
    source: &SourceReader,
    scope: SourceScope,
    limits: Limits,
) -> Result<DetectionReport, String> {
    catalog.validate()?;
    if limits.max_depth == 0
        || limits.max_depth > 64
        || limits.max_directories == 0
        || limits.max_directories > 4096
        || limits.max_matches == 0
        || limits.max_matches > 200_000
        || limits.max_steps == 0
        || limits.max_steps > 2_000_000
        || limits.max_entries_per_directory == 0
        || limits.max_entries_per_directory > MAX_DIRECTORY_ENTRIES
    {
        return Err("invalid detector limits".into());
    }
    let mut walk = Walker {
        source,
        limits,
        listings: BTreeMap::new(),
        matchers: BTreeMap::new(),
        matches: 0,
        steps: 0,
    };
    let mut report = DetectionReport {
        catalog_uri: catalog.catalog_uri.clone(),
        dataset_version: catalog.dataset_version.clone(),
        source_uri: source.source_uri().into(),
        scope,
        traversal_complete: true,
        directories_listed: 0,
        roots: vec![],
        exclusions: vec![],
        issues: vec![],
        ambiguities: vec![],
        coverage_gap_ids: catalog.gaps.iter().map(|g| g.id.clone()).collect(),
    };
    for pattern in &catalog.patterns {
        if !pattern.analysis_candidate() || pattern.root_scope != scope.id() {
            let reason = if !pattern.analysis_candidate() {
                format!(
                    "{} / {} / {}",
                    pattern.status, pattern.evidence_level, pattern.root_scope
                )
            } else {
                "scope_not_selected".into()
            };
            report.exclusions.push(Exclusion {
                pattern_id: pattern.id.clone(),
                reason,
            });
            continue;
        }
        let mut roots = BTreeSet::new();
        for expression in &pattern.roots {
            match walk.expand(".", expression, EntryKind::Directory) {
                Ok(found) => roots.extend(found),
                Err(message) => report.issues.push(Issue {
                    pattern_id: pattern.id.clone(),
                    expression: expression.clone(),
                    message,
                }),
            }
        }
        for root in roots {
            let mut finding = RootFinding {
                pattern_id: pattern.id.clone(),
                root: source.reference(&root).map_err(|e| e.to_string())?,
                files: vec![],
            };
            let mut candidates = BTreeMap::<(String, String), RoleCandidate>::new();
            for rule in &pattern.files {
                match walk.expand(&root, &rule.path, EntryKind::File) {
                    Ok(paths) => {
                        for path in paths {
                            let reference = source.reference(&path).map_err(|e| e.to_string())?;
                            let candidate = candidates
                                .entry((path, rule.role.clone()))
                                .or_insert_with(|| RoleCandidate {
                                    reference,
                                    role: rule.role.clone(),
                                    rules: vec![],
                                });
                            candidate.rules.push(MatchedRule {
                                pattern: rule.path.clone(),
                                condition: rule.condition.clone(),
                            });
                        }
                    }
                    Err(message) => report.issues.push(Issue {
                        pattern_id: pattern.id.clone(),
                        expression: format!("{root}/{}", rule.path),
                        message,
                    }),
                }
            }
            finding.files = candidates.into_values().collect();
            report.roots.push(finding);
        }
    }
    let mut claims = BTreeMap::<String, BTreeSet<Claim>>::new();
    for root in &report.roots {
        for file in &root.files {
            claims
                .entry(file.reference.uri())
                .or_default()
                .insert(Claim {
                    pattern_id: root.pattern_id.clone(),
                    root_uri: root.root.uri(),
                    role: file.role.clone(),
                });
        }
    }
    report.ambiguities = claims
        .into_iter()
        .filter(|(_, claims)| claims.len() > 1)
        .map(|(file_uri, claims)| Ambiguity {
            file_uri,
            claims: claims.into_iter().collect(),
        })
        .collect();
    report.traversal_complete = report.issues.is_empty();
    report.directories_listed = walk.listings.len();
    Ok(report)
}

struct Walker<'a> {
    source: &'a SourceReader,
    limits: Limits,
    listings: BTreeMap<String, Result<DirectoryListing, String>>,
    matchers: BTreeMap<(String, bool), GlobMatcher>,
    matches: usize,
    steps: usize,
}

impl Walker<'_> {
    fn listing(&mut self, path: &str) -> Result<DirectoryListing, String> {
        if !self.listings.contains_key(path) {
            if self.listings.len() >= self.limits.max_directories {
                return Err("directory traversal limit exceeded".into());
            }
            let result = self
                .source
                .reference(path)
                .and_then(|r| self.source.list(&r, self.limits.max_entries_per_directory))
                .map_err(|e| format!("directory {path}: {e}"));
            self.listings.insert(path.into(), result);
        }
        self.listings[path].clone()
    }

    fn expand(
        &mut self,
        base: &str,
        expression: &str,
        kind: EntryKind,
    ) -> Result<BTreeSet<String>, String> {
        validate_expression(expression)?;
        let segments: Vec<_> = if expression == "." {
            vec![]
        } else {
            expression.split('/').collect()
        };
        let mut found = BTreeSet::new();
        self.visit(base, &segments, 0, kind, &mut BTreeSet::new(), &mut found)?;
        Ok(found)
    }

    fn visit(
        &mut self,
        directory: &str,
        segments: &[&str],
        position: usize,
        kind: EntryKind,
        visited: &mut BTreeSet<(String, usize)>,
        found: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        self.steps += 1;
        if self.steps > self.limits.max_steps {
            return Err("pattern traversal work limit exceeded".into());
        }
        if directory.split('/').count() > self.limits.max_depth {
            return Err("directory depth limit exceeded".into());
        }
        if !visited.insert((directory.into(), position)) {
            return Ok(());
        }
        if position == segments.len() {
            if kind == EntryKind::Directory {
                self.add(directory, found)?;
            }
            return Ok(());
        }
        let segment = segments[position];
        if segment == "**" {
            self.visit(directory, segments, position + 1, kind, visited, found)?;
        }
        let listing = self.listing(directory)?;
        let key = (
            segment.to_string(),
            listing.match_case == MatchCase::Insensitive,
        );
        if segment != "**" && !self.matchers.contains_key(&key) {
            let matcher = GlobBuilder::new(segment)
                .literal_separator(true)
                .backslash_escape(false)
                .case_insensitive(key.1)
                .build()
                .map_err(|_| "unsupported path expression")?
                .compile_matcher();
            self.matchers.insert(key.clone(), matcher);
        }
        for entry in listing.entries {
            self.steps += 1;
            if self.steps > self.limits.max_steps {
                return Err("pattern traversal work limit exceeded".into());
            }
            if segment != "**" && !self.matchers[&key].is_match(&entry.name) {
                continue;
            }
            if matches!(entry.kind, EntryKind::Link | EntryKind::Other) {
                return Err(format!(
                    "unresolved link/special entry: {}",
                    entry.reference.relative_path()
                ));
            }
            if segment == "**" {
                if entry.kind == EntryKind::Directory {
                    self.visit(
                        entry.reference.relative_path(),
                        segments,
                        position,
                        kind,
                        visited,
                        found,
                    )?;
                }
            } else if position + 1 == segments.len() {
                if entry.kind == kind {
                    self.add(entry.reference.relative_path(), found)?;
                }
            } else if entry.kind == EntryKind::Directory {
                self.visit(
                    entry.reference.relative_path(),
                    segments,
                    position + 1,
                    kind,
                    visited,
                    found,
                )?;
            }
        }
        Ok(())
    }

    fn add(&mut self, path: &str, found: &mut BTreeSet<String>) -> Result<(), String> {
        if found.insert(path.into()) {
            self.matches += 1;
            if self.matches > self.limits.max_matches {
                return Err("pattern match limit exceeded".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

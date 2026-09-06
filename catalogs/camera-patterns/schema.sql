PRAGMA foreign_keys = ON;
PRAGMA application_id = 1364083523;
PRAGMA user_version = 1;

CREATE TABLE catalog (
    catalog_id TEXT PRIMARY KEY CHECK (catalog_id = 'qnc.catalog.camera-patterns'),
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    dataset_version TEXT NOT NULL,
    reviewed_on TEXT NOT NULL,
    runtime_status TEXT NOT NULL CHECK (runtime_status = 'research_only')
) STRICT;

CREATE TABLE source (
    source_id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('manufacturer_document', 'card_observation')),
    publisher TEXT NOT NULL,
    title TEXT NOT NULL,
    locator TEXT NOT NULL,
    section TEXT NOT NULL,
    reviewed_on TEXT NOT NULL
) STRICT;

CREATE TABLE pattern (
    pattern_id TEXT PRIMARY KEY,
    manufacturer TEXT NOT NULL,
    family TEXT NOT NULL,
    applicability TEXT NOT NULL,
    naming_rule TEXT NOT NULL,
    evidence_level TEXT NOT NULL CHECK (evidence_level IN ('observed', 'documented', 'partial')),
    status TEXT NOT NULL CHECK (status IN ('enabled', 'disabled', 'incorrect')),
    status_reason TEXT NOT NULL,
    root_scope TEXT NOT NULL CHECK (root_scope IN ('card_relative', 'recording_relative', 'reel_relative', 'unknown')),
    grouping_method TEXT NOT NULL CHECK (grouping_method IN ('manifest_references', 'metadata_identity', 'directory_segments', 'paired_names', 'unresolved')),
    grouping_notes TEXT NOT NULL,
    limitations TEXT NOT NULL
) STRICT;

CREATE TABLE pattern_root (
    pattern_id TEXT NOT NULL REFERENCES pattern ON DELETE CASCADE,
    relative_pattern TEXT NOT NULL,
    PRIMARY KEY (pattern_id, relative_pattern)
) STRICT;

CREATE TABLE file_rule (
    pattern_id TEXT NOT NULL REFERENCES pattern ON DELETE CASCADE,
    relative_pattern TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('original_candidate', 'proxy_candidate', 'preview', 'thumbnail', 'metadata', 'index', 'audio_component', 'companion_data')),
    condition TEXT NOT NULL,
    PRIMARY KEY (pattern_id, relative_pattern, role)
) STRICT;

CREATE TABLE metadata_field (
    pattern_id TEXT NOT NULL REFERENCES pattern ON DELETE CASCADE,
    document_pattern TEXT NOT NULL,
    xml_namespace TEXT NOT NULL,
    selector TEXT NOT NULL,
    meaning TEXT NOT NULL,
    PRIMARY KEY (pattern_id, document_pattern, selector)
) STRICT;

CREATE TABLE evidence (
    pattern_id TEXT NOT NULL REFERENCES pattern ON DELETE CASCADE,
    source_id TEXT NOT NULL REFERENCES source,
    supports TEXT NOT NULL,
    PRIMARY KEY (pattern_id, source_id)
) STRICT;

CREATE TABLE coverage_gap (
    gap_id TEXT PRIMARY KEY,
    manufacturer TEXT NOT NULL,
    family TEXT NOT NULL,
    missing_evidence TEXT NOT NULL,
    source_id TEXT REFERENCES source,
    status TEXT NOT NULL CHECK (status = 'research_pending')
) STRICT;

CREATE VIEW public_catalog AS SELECT * FROM catalog;
CREATE VIEW public_sources AS SELECT * FROM source;
CREATE VIEW public_patterns AS SELECT * FROM pattern;
CREATE VIEW public_analysis_patterns AS
    SELECT * FROM pattern WHERE status = 'enabled'
    AND evidence_level IN ('observed', 'documented') AND root_scope <> 'unknown';
CREATE VIEW public_roots AS SELECT * FROM pattern_root;
CREATE VIEW public_file_rules AS SELECT * FROM file_rule;
CREATE VIEW public_metadata_fields AS SELECT * FROM metadata_field;
CREATE VIEW public_evidence AS SELECT * FROM evidence;
CREATE VIEW public_coverage_gaps AS SELECT * FROM coverage_gap;

CREATE TABLE change_log (
    change_id INTEGER PRIMARY KEY,
    dataset_version TEXT NOT NULL,
    changed_on TEXT NOT NULL,
    pattern_id TEXT NOT NULL,
    operation TEXT NOT NULL,
    reason TEXT NOT NULL,
    before_json TEXT CHECK (before_json IS NULL OR json_valid(before_json)),
    after_json TEXT CHECK (after_json IS NULL OR json_valid(after_json))
) STRICT;
CREATE VIEW public_changes AS SELECT * FROM change_log;

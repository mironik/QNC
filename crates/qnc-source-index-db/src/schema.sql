CREATE TABLE source_records (
    record_id TEXT PRIMARY KEY NOT NULL,
    source_uri TEXT NOT NULL,
    recording_root_uri TEXT NOT NULL,
    recording_identity TEXT NOT NULL,
    group_json TEXT NOT NULL,
    recorded_at_unix_ms INTEGER NOT NULL CHECK(recorded_at_unix_ms >= 0),
    UNIQUE(recording_root_uri, recording_identity)
);
CREATE TABLE media_references (
    media_uri TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL REFERENCES source_records(record_id),
    role TEXT NOT NULL CHECK(role IN ('original', 'proxy'))
);
CREATE TABLE support_references (
    media_uri TEXT NOT NULL,
    record_id TEXT NOT NULL REFERENCES source_records(record_id),
    role TEXT NOT NULL CHECK(role IN ('related', 'evidence')),
    kind TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('file', 'missing', 'unavailable')),
    PRIMARY KEY(media_uri, record_id, role)
);
CREATE TABLE write_receipts (
    batch_id TEXT PRIMARY KEY NOT NULL,
    payload BLOB NOT NULL,
    receipt_json TEXT NOT NULL
);
CREATE VIEW public_source_records AS
    SELECT record_id, source_uri, recording_root_uri, recording_identity,
           group_json, recorded_at_unix_ms FROM source_records;
CREATE VIEW public_source_media AS
    SELECT media_uri, record_id, role FROM media_references;
CREATE VIEW public_source_support AS
    SELECT media_uri, record_id, role, kind, state FROM support_references;

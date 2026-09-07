CREATE TABLE media_heads (
    clip_id TEXT PRIMARY KEY NOT NULL,
    source_index_uri TEXT NOT NULL,
    source_record_id TEXT NOT NULL,
    binding_json TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision IN (1, 2)),
    UNIQUE(source_index_uri, source_record_id),
    FOREIGN KEY(clip_id, revision) REFERENCES media_snapshots(clip_id, revision)
);
CREATE TABLE media_snapshots (
    clip_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision IN (1, 2)),
    phase TEXT NOT NULL CHECK(phase IN ('camera', 'final')),
    completeness TEXT NOT NULL CHECK(completeness IN ('partial', 'complete')),
    snapshot_json TEXT NOT NULL,
    PRIMARY KEY(clip_id, revision)
);
CREATE TABLE evidence_documents (
    document_uri TEXT PRIMARY KEY NOT NULL,
    media_type TEXT NOT NULL CHECK(media_type IN ('xml', 'json')),
    document_text TEXT NOT NULL
);
CREATE TABLE snapshot_documents (
    clip_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    document_uri TEXT NOT NULL REFERENCES evidence_documents(document_uri),
    PRIMARY KEY(clip_id, revision, document_uri),
    FOREIGN KEY(clip_id, revision) REFERENCES media_snapshots(clip_id, revision)
);
CREATE TABLE write_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    descriptor_json TEXT NOT NULL,
    receipt_json TEXT NOT NULL
);
CREATE TABLE media_acquisitions (
    media_uri TEXT PRIMARY KEY NOT NULL,
    attempt_id TEXT NOT NULL UNIQUE,
    document_uri TEXT NOT NULL UNIQUE,
    clip_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision = 1),
    acquisition_json TEXT NOT NULL,
    FOREIGN KEY(clip_id, revision) REFERENCES media_snapshots(clip_id, revision)
);
CREATE VIEW public_media_heads AS
    SELECT clip_id, source_index_uri, source_record_id, binding_json, revision FROM media_heads;
CREATE VIEW public_media_snapshots AS
    SELECT clip_id, revision, phase, completeness, snapshot_json FROM media_snapshots;
CREATE VIEW public_media_documents AS
    SELECT document_uri, media_type, document_text FROM evidence_documents;
CREATE VIEW public_media_snapshot_documents AS
    SELECT clip_id, revision, document_uri FROM snapshot_documents;
CREATE VIEW public_media_acquisitions AS
    SELECT media_uri, attempt_id, document_uri, clip_id, revision, acquisition_json FROM media_acquisitions;

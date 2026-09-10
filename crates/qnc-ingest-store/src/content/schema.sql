CREATE TABLE ingest_content_schema (version TEXT NOT NULL, project_id TEXT NOT NULL);
CREATE TABLE clips (
    clip_id TEXT PRIMARY KEY, source_uri TEXT NOT NULL, original_uri TEXT NOT NULL,
    name TEXT NOT NULL, created_at_utc TEXT, duration_seconds REAL,
    duration_frames INTEGER, fps_num INTEGER, fps_den INTEGER,
    thumbnail_uri TEXT, catalog_json TEXT NOT NULL, revision INTEGER NOT NULL, final INTEGER NOT NULL,
    selected INTEGER NOT NULL DEFAULT 0, import_status TEXT NOT NULL DEFAULT 'detected',
    import_error TEXT, imported_media_uri TEXT
);
CREATE TABLE clip_sources (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), source_uri TEXT NOT NULL,
    original_uri TEXT NOT NULL, original_container TEXT, original_codec TEXT,
    serial_number TEXT NOT NULL, volume_name TEXT NOT NULL, source_name TEXT NOT NULL
);
CREATE TABLE clip_proxy (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), proxy_uri TEXT,
    proxy_container TEXT, proxy_codec TEXT
);
CREATE TABLE probe_records (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), probe_json TEXT NOT NULL,
    probed_at_utc TEXT NOT NULL, record_db_uri TEXT NOT NULL, record_revision INTEGER NOT NULL
);
CREATE TABLE filmstrip_artifacts (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), frame_count INTEGER NOT NULL,
    artifact_uri TEXT NOT NULL, created_at_utc TEXT NOT NULL, frames_json TEXT NOT NULL
);
CREATE TABLE wave_artifacts (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), artifact_uri TEXT NOT NULL,
    created_at_utc TEXT NOT NULL, peaks_json TEXT NOT NULL
);
CREATE VIEW public_clips AS SELECT clip_id,source_uri,original_uri,name,created_at_utc,
    duration_seconds,duration_frames,fps_num,fps_den,selected,import_status,
    imported_media_uri,import_error,thumbnail_uri FROM clips;
CREATE VIEW public_clip_sources AS SELECT * FROM clip_sources;
CREATE VIEW public_clip_proxy AS SELECT * FROM clip_proxy;
CREATE VIEW public_probe_records AS SELECT * FROM probe_records;
CREATE VIEW public_filmstrip_artifacts AS SELECT * FROM filmstrip_artifacts;
CREATE VIEW public_wave_artifacts AS SELECT * FROM wave_artifacts;

-- schema of C:\Users\miron\Test projekt\2026_10_30_novi_af3f9d41ee714116b251dee2e4dae708\project.db

-- table clip_proxy: 146 rows
CREATE TABLE clip_proxy (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), proxy_uri TEXT,
    proxy_container TEXT, proxy_codec TEXT
);

-- table clip_shots: 146 rows
CREATE TABLE clip_shots (
                    clip_id TEXT PRIMARY KEY,
                    media_uri TEXT NOT NULL,
                    pictures INTEGER NOT NULL,
                    shots_json TEXT NOT NULL,
                    parts_json TEXT NOT NULL,
                    version INTEGER NOT NULL,
                    error TEXT NOT NULL,
                    took_ms INTEGER NOT NULL,
                    made_at_utc TEXT NOT NULL
                 );

-- table clip_sound: 1 rows
CREATE TABLE clip_sound (
                clip_id TEXT PRIMARY KEY,
                a1_channel INTEGER NOT NULL CHECK (a1_channel >= 0),
                a1_off INTEGER NOT NULL CHECK (a1_off IN (0, 1)),
                a1_level_db INTEGER NOT NULL,
                a2_channel INTEGER NOT NULL CHECK (a2_channel >= 0),
                a2_off INTEGER NOT NULL CHECK (a2_off IN (0, 1)),
                a2_level_db INTEGER NOT NULL,
                updated_at_utc TEXT NOT NULL
            );

-- table clip_sources: 146 rows
CREATE TABLE clip_sources (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), source_uri TEXT NOT NULL,
    original_uri TEXT NOT NULL, original_container TEXT, original_codec TEXT,
    serial_number TEXT NOT NULL, volume_name TEXT NOT NULL, source_name TEXT NOT NULL
);

-- table clip_transcripts: 5 rows
CREATE TABLE clip_transcripts (
                clip_id TEXT NOT NULL,
                from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
                to_frame INTEGER NOT NULL CHECK (to_frame > from_frame),
                language TEXT NOT NULL,
                engine TEXT NOT NULL,
                text TEXT NOT NULL,
                lines_json TEXT NOT NULL,
                made_at_utc TEXT NOT NULL,
                PRIMARY KEY (clip_id, from_frame, to_frame)
            );

-- table clips: 146 rows
CREATE TABLE clips (
    clip_id TEXT PRIMARY KEY, source_uri TEXT NOT NULL, original_uri TEXT NOT NULL,
    name TEXT NOT NULL, created_at_utc TEXT, duration_seconds REAL,
    duration_frames INTEGER, fps_num INTEGER, fps_den INTEGER,
    thumbnail_uri TEXT, catalog_json TEXT NOT NULL, revision INTEGER NOT NULL, final INTEGER NOT NULL,
    selected INTEGER NOT NULL DEFAULT 0, import_status TEXT NOT NULL DEFAULT 'detected',
    import_error TEXT, imported_media_uri TEXT
, import_claimed_at INTEGER, imported_copy_of TEXT CHECK (imported_copy_of IN ('original','proxy')), optimized_json TEXT);

-- table evidence_documents: 439 rows
CREATE TABLE evidence_documents (
    document_uri TEXT PRIMARY KEY NOT NULL,
    media_type TEXT NOT NULL CHECK(media_type IN ('xml', 'json')),
    document_text TEXT NOT NULL
);

-- table filmstrip_artifacts: 146 rows
CREATE TABLE filmstrip_artifacts (
            clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id),
            frame_count INTEGER NOT NULL,
            artifact_uri TEXT NOT NULL,
            created_at_utc TEXT NOT NULL,
            frames_json TEXT NOT NULL
        );

-- table filmstrip_frames: 1898 rows
CREATE TABLE filmstrip_frames (
            clip_id TEXT NOT NULL REFERENCES clips(clip_id),
            frame_index INTEGER NOT NULL,
            seek_sec REAL NOT NULL,
            artifact_uri TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL,
            PRIMARY KEY (clip_id, frame_index)
        );

-- table ingest_content_schema: 1 rows
CREATE TABLE ingest_content_schema (version TEXT NOT NULL, project_id TEXT NOT NULL);

-- table ingest_runtime: 6 rows
CREATE TABLE ingest_runtime(
            key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL);

-- table media_acquisitions: 292 rows
CREATE TABLE media_acquisitions (
    media_uri TEXT PRIMARY KEY NOT NULL,
    attempt_id TEXT NOT NULL UNIQUE,
    document_uri TEXT NOT NULL UNIQUE,
    clip_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision = 1),
    acquisition_json TEXT NOT NULL,
    FOREIGN KEY(clip_id, revision) REFERENCES media_snapshots(clip_id, revision)
);

-- table media_heads: 146 rows
CREATE TABLE media_heads (
    clip_id TEXT PRIMARY KEY NOT NULL,
    source_index_uri TEXT NOT NULL,
    source_record_id TEXT NOT NULL,
    binding_json TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision IN (1, 2)),
    UNIQUE(source_index_uri, source_record_id),
    FOREIGN KEY(clip_id, revision) REFERENCES media_snapshots(clip_id, revision)
);

-- table media_references: 292 rows
CREATE TABLE media_references (
    media_uri TEXT PRIMARY KEY NOT NULL,
    record_id TEXT NOT NULL REFERENCES source_records(record_id),
    role TEXT NOT NULL CHECK(role IN ('original', 'proxy'))
);

-- table media_snapshots: 292 rows
CREATE TABLE media_snapshots (
    clip_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision IN (1, 2)),
    phase TEXT NOT NULL CHECK(phase IN ('camera', 'final')),
    completeness TEXT NOT NULL CHECK(completeness IN ('partial', 'complete')),
    snapshot_json TEXT NOT NULL,
    PRIMARY KEY(clip_id, revision)
);

-- table media_write_receipts: 292 rows
CREATE TABLE media_write_receipts (
    request_id TEXT PRIMARY KEY NOT NULL,
    descriptor_json TEXT NOT NULL,
    receipt_json TEXT NOT NULL
);

-- table prerender_units: 2 rows
CREATE TABLE prerender_units (
                fingerprint TEXT PRIMARY KEY CHECK (length(fingerprint) = 16),
                file_uri TEXT NOT NULL,
                frames INTEGER NOT NULL CHECK (frames > 0),
                samples INTEGER NOT NULL CHECK (samples >= 0),
                encoder TEXT NOT NULL,
                key_json TEXT NOT NULL,
                media_json TEXT NOT NULL,
                rendered_at_utc TEXT NOT NULL
            );

-- table probe_records: 146 rows
CREATE TABLE probe_records (
    clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id), probe_json TEXT NOT NULL,
    probed_at_utc TEXT NOT NULL, record_db_uri TEXT NOT NULL, record_revision INTEGER NOT NULL
);

-- table project_data_revisions: 0 rows
CREATE TABLE project_data_revisions (
            scope TEXT PRIMARY KEY,
            revision INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL DEFAULT ''
        );

-- table project_members: 0 rows
CREATE TABLE project_members (
            project_id TEXT NOT NULL,
            user_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'editor',
            joined_at TEXT,
            last_seen_at TEXT,
            PRIMARY KEY(project_id, user_id)
        );

-- table project_origin: 1 rows
CREATE TABLE project_origin (
            project_id TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            identity_json TEXT NOT NULL CHECK(json_valid(identity_json))
        );

-- table project_settings: 1 rows
CREATE TABLE project_settings (
            project_id TEXT PRIMARY KEY,
            template_id TEXT,
            settings_json TEXT NOT NULL DEFAULT '{}',
            created_by TEXT,
            updated_by TEXT,
            created_at TEXT,
            updated_at TEXT
        );

-- table project_settings_kv: 0 rows
CREATE TABLE project_settings_kv (
            project_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (project_id, setting_key)
        );

-- table project_snapshot_kv: 0 rows
CREATE TABLE project_snapshot_kv (
            project_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (project_id, setting_key)
        );

-- table project_story_sheet: 1 rows
CREATE TABLE project_story_sheet (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            sheet_json TEXT NOT NULL CHECK (json_valid(sheet_json)),
            saved_at_utc TEXT NOT NULL
        );

-- table project_story_values: 4 rows
CREATE TABLE project_story_values (
            code TEXT PRIMARY KEY,
            value_json TEXT NOT NULL CHECK (json_valid(value_json))
        );

-- table project_template_snapshot: 1 rows
CREATE TABLE project_template_snapshot (
            project_id TEXT PRIMARY KEY,
            template_id TEXT,
            template_name TEXT NOT NULL DEFAULT '',
            template_version TEXT NOT NULL DEFAULT '',
            snapshot_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT
        );

-- table project_workflow_state: 1 rows
CREATE TABLE project_workflow_state (
            project_id TEXT PRIMARY KEY,
            active_step_id TEXT,
            entry_step_id TEXT,
            updated_at TEXT
        );

-- table project_workflow_step_kv: 0 rows
CREATE TABLE project_workflow_step_kv (
            step_id TEXT NOT NULL,
            setting_key TEXT NOT NULL,
            setting_value TEXT NOT NULL,
            PRIMARY KEY (step_id, setting_key)
        );

-- table project_workflow_steps: 6 rows
CREATE TABLE project_workflow_steps (
            step_id TEXT PRIMARY KEY,
            project_id TEXT NOT NULL,
            plugin_id TEXT NOT NULL,
            tab_id TEXT NOT NULL,
            label TEXT NOT NULL,
            position INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'locked',
            next_step_id TEXT,
            settings_json TEXT NOT NULL DEFAULT '{}'
        );

-- table script_blocks: 6 rows
CREATE TABLE script_blocks (block_id INTEGER PRIMARY KEY AUTOINCREMENT,
            position INTEGER NOT NULL,
            kind TEXT NOT NULL CHECK (length(kind) > 0),
            text TEXT NOT NULL,
            clip_id TEXT NOT NULL,
            clip_name TEXT NOT NULL,
            from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
            to_frame INTEGER NOT NULL CHECK (to_frame >= 0),
            start_tc TEXT NOT NULL,
            end_tc TEXT NOT NULL,
            length TEXT NOT NULL,
            seconds REAL NOT NULL CHECK (seconds >= 0),
            voice_uri TEXT NOT NULL DEFAULT '',
            voice_seconds REAL NOT NULL DEFAULT 0 CHECK (voice_seconds >= 0),
            voice_tc TEXT NOT NULL DEFAULT '',
            fields_json TEXT NOT NULL DEFAULT '{}',
            pieces_json TEXT NOT NULL DEFAULT '[]',
            updated_at_utc TEXT NOT NULL);

-- table script_profile: 1 rows
CREATE TABLE script_profile (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            profile_id TEXT NOT NULL,
            profile_json TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL);

-- table snapshot_documents: 876 rows
CREATE TABLE snapshot_documents (
    clip_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    document_uri TEXT NOT NULL REFERENCES evidence_documents(document_uri),
    PRIMARY KEY(clip_id, revision, document_uri),
    FOREIGN KEY(clip_id, revision) REFERENCES media_snapshots(clip_id, revision)
);

-- table source_records: 146 rows
CREATE TABLE source_records (
    record_id TEXT PRIMARY KEY NOT NULL,
    source_uri TEXT NOT NULL,
    recording_root_uri TEXT NOT NULL,
    recording_identity TEXT NOT NULL,
    group_json TEXT NOT NULL,
    recorded_at_unix_ms INTEGER NOT NULL CHECK(recorded_at_unix_ms >= 0),
    UNIQUE(recording_root_uri, recording_identity)
);

-- table source_write_receipts: 5 rows
CREATE TABLE source_write_receipts (
    batch_id TEXT PRIMARY KEY NOT NULL,
    payload BLOB NOT NULL,
    receipt_json TEXT NOT NULL
);

-- table story_covers: 0 rows
CREATE TABLE story_covers (
            cover_id TEXT PRIMARY KEY,
            slot_id TEXT NOT NULL DEFAULT '',
            timeline_start_frame INTEGER NOT NULL DEFAULT 0,
            timeline_end_frame INTEGER NOT NULL DEFAULT 0,
            timeline_start_sec REAL NOT NULL DEFAULT 0,
            timeline_end_sec REAL NOT NULL DEFAULT 0,
            slot_signature TEXT NOT NULL DEFAULT '',
            slot_index INTEGER NOT NULL DEFAULT 0,
            clip_id TEXT NOT NULL DEFAULT '',
            virtual_shot_id TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            note TEXT NOT NULL DEFAULT '',
            in_tc TEXT NOT NULL DEFAULT '',
            out_tc TEXT NOT NULL DEFAULT '',
            in_seconds REAL,
            out_seconds REAL,
            source_in_frame INTEGER NOT NULL DEFAULT 0,
            source_out_frame INTEGER NOT NULL DEFAULT 0,
            source_fps REAL NOT NULL DEFAULT 0,
            source_fps_num INTEGER NOT NULL DEFAULT 0,
            source_fps_den INTEGER NOT NULL DEFAULT 1,
            sort_index INTEGER NOT NULL DEFAULT 0,
            a2_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a2_source_channel >= 0),
            a2_level_db INTEGER NOT NULL DEFAULT 0,
            a2_off INTEGER NOT NULL DEFAULT 0,
            a2_lead_frames INTEGER NOT NULL DEFAULT 0 CHECK (a2_lead_frames >= 0),
            a2_tail_frames INTEGER NOT NULL DEFAULT 0 CHECK (a2_tail_frames >= 0),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );

-- table story_marker_slots: 1 rows
CREATE TABLE story_marker_slots (
            slot_id TEXT PRIMARY KEY,
            slot_index INTEGER NOT NULL,
            start_frame INTEGER NOT NULL DEFAULT 0,
            end_frame INTEGER NOT NULL DEFAULT 0,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            start_sec REAL NOT NULL,
            end_sec REAL NOT NULL,
            duration_sec REAL NOT NULL DEFAULT 0,
            start_marker_id TEXT NOT NULL DEFAULT '',
            end_marker_id TEXT NOT NULL DEFAULT '',
            slot_signature TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
         );

-- table story_markers: 2 rows
CREATE TABLE story_markers (
            marker_id TEXT PRIMARY KEY,
            timeline_frame INTEGER NOT NULL DEFAULT 0,
            timeline_sec REAL NOT NULL DEFAULT 0,
            tc TEXT NOT NULL DEFAULT '',
            label TEXT NOT NULL DEFAULT '',
            sort_index INTEGER NOT NULL DEFAULT 0,
            system_role TEXT NOT NULL DEFAULT '',
            origin_part_id TEXT NOT NULL DEFAULT '',
            origin_local_frame INTEGER,
            origin_local_sec REAL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );

-- table story_object_history: 1 rows
CREATE TABLE story_object_history (
            object_type TEXT NOT NULL,
            object_id TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT '',
            snapshot_json TEXT NOT NULL DEFAULT '{}',
            updated_at TEXT NOT NULL,
            PRIMARY KEY (object_type, object_id)
         );

-- table story_parts: 4 rows
CREATE TABLE story_parts (
            part_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL CHECK (kind IN ('tonovi', 'offovi')),
            sort_index INTEGER NOT NULL,
            title TEXT NOT NULL DEFAULT '',
            text TEXT NOT NULL DEFAULT '',
            clip_id TEXT NOT NULL DEFAULT '',
            virtual_shot_id TEXT NOT NULL DEFAULT '',
            in_tc TEXT NOT NULL DEFAULT '',
            out_tc TEXT NOT NULL DEFAULT '',
            in_seconds REAL,
            out_seconds REAL,
            fps REAL NOT NULL DEFAULT 0,
            source_fps_num INTEGER NOT NULL DEFAULT 0,
            source_fps_den INTEGER NOT NULL DEFAULT 1,
            in_frame INTEGER NOT NULL DEFAULT 0,
            out_frame INTEGER NOT NULL DEFAULT 0,
            duration_frames INTEGER NOT NULL DEFAULT 0,
            duration_label TEXT NOT NULL DEFAULT '',
            duration_color_key TEXT NOT NULL DEFAULT '',
            active INTEGER NOT NULL DEFAULT 1,
            a1_source_channel INTEGER NOT NULL DEFAULT 0 CHECK (a1_source_channel >= 0),
            a1_level_db INTEGER NOT NULL DEFAULT 0,
            a1_off INTEGER NOT NULL DEFAULT 0,
            v_lead_frames INTEGER NOT NULL DEFAULT 0 CHECK (v_lead_frames >= 0),
            a1_lead_frames INTEGER NOT NULL DEFAULT 0 CHECK (a1_lead_frames >= 0),
            a1_tail_frames INTEGER NOT NULL DEFAULT 0 CHECK (a1_tail_frames >= 0),
            v_tail_frames INTEGER NOT NULL DEFAULT 0 CHECK (v_tail_frames >= 0),
            lead TEXT NOT NULL DEFAULT '' CHECK (lead IN ('', 'in', 'out')),
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
         );

-- table story_revision: 1 rows
CREATE TABLE story_revision (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            revision INTEGER NOT NULL
         );

-- table story_state: 1 rows
CREATE TABLE story_state (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            selected_part_id TEXT NOT NULL DEFAULT '',
            selected_shot_id TEXT NOT NULL DEFAULT '',
            selected_slot_id TEXT NOT NULL DEFAULT '',
            selected_cover_id TEXT NOT NULL DEFAULT '',
            draft_updated_at TEXT,
            committed_at TEXT,
            updated_at TEXT,
            story_fps_num INTEGER NOT NULL DEFAULT 0,
            story_fps_den INTEGER NOT NULL DEFAULT 1
         );

-- table story_undo: 43 rows
CREATE TABLE story_undo (
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            stack TEXT NOT NULL CHECK (stack IN ('undo', 'redo')),
            snapshot_json TEXT NOT NULL,
            created_at TEXT NOT NULL
         );

-- table support_references: 438 rows
CREATE TABLE support_references (
    media_uri TEXT NOT NULL,
    record_id TEXT NOT NULL REFERENCES source_records(record_id),
    role TEXT NOT NULL CHECK(role IN ('related', 'evidence')),
    kind TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('file', 'missing', 'unavailable')),
    PRIMARY KEY(media_uri, record_id, role)
);

-- table transcript_queue: 0 rows
CREATE TABLE transcript_queue (
                clip_id TEXT PRIMARY KEY,
                from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
                to_frame INTEGER NOT NULL CHECK (to_frame > from_frame),
                quality TEXT NOT NULL CHECK (quality IN ('fast', 'best')),
                state TEXT NOT NULL CHECK (state IN ('waiting', 'running', 'done', 'failed')),
                error TEXT NOT NULL,
                took_ms INTEGER NOT NULL DEFAULT 0,
                asked_at_utc TEXT NOT NULL
            );

-- table transcript_quotes: 0 rows
CREATE TABLE transcript_quotes (
                id INTEGER PRIMARY KEY,
                clip_id TEXT NOT NULL,
                clip_name TEXT NOT NULL,
                from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
                to_frame INTEGER NOT NULL CHECK (to_frame > from_frame),
                start_tc TEXT NOT NULL,
                end_tc TEXT NOT NULL,
                length TEXT NOT NULL,
                text TEXT NOT NULL,
                added_at_utc TEXT NOT NULL
            );

-- table virtual_shot_numbers: 10 rows
CREATE TABLE virtual_shot_numbers (
            clip_id TEXT NOT NULL,
            class TEXT NOT NULL,
            last INTEGER NOT NULL,
            PRIMARY KEY (clip_id, class)
        );

-- table virtual_shots: 14 rows
CREATE TABLE virtual_shots (
            shot_id TEXT PRIMARY KEY,
            clip_id TEXT NOT NULL,
            class TEXT NOT NULL CHECK (class IN ('source', 'short', 'b_roll')),
            in_frame INTEGER NOT NULL,
            out_frame INTEGER NOT NULL,
            source_shot_id TEXT,
            name TEXT NOT NULL,
            created_at_utc INTEGER NOT NULL,
            in_still_uri TEXT,
            out_still_uri TEXT,
            still_status TEXT NOT NULL DEFAULT 'pending'
                CHECK (still_status IN ('pending', 'ready', 'failed')),
            still_error TEXT
        , cover_pending INTEGER NOT NULL DEFAULT 0);

-- table wave_artifacts: 146 rows
CREATE TABLE wave_artifacts (
            clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id),
            artifact_uri TEXT NOT NULL,
            created_at_utc TEXT NOT NULL,
            peaks_json TEXT NOT NULL
        );
CREATE INDEX idx_story_markers_frame ON story_markers(timeline_frame);
CREATE INDEX idx_story_parts_sort ON story_parts(sort_index);
CREATE VIEW public_b_roll_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'b_roll' AND cover_pending = 0;
CREATE VIEW public_clip_proxy AS SELECT * FROM clip_proxy;
CREATE VIEW public_clip_shots AS
                    SELECT clip_id, media_uri, pictures, shots_json, parts_json, version, error, took_ms, made_at_utc FROM clip_shots;
CREATE VIEW public_clip_sound AS
             SELECT clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db, updated_at_utc
             FROM clip_sound;
CREATE VIEW public_clip_sources AS SELECT * FROM clip_sources;
CREATE VIEW public_clip_transcripts AS
             SELECT clip_id, from_frame, to_frame, language, engine, text, lines_json, made_at_utc
             FROM clip_transcripts;
CREATE VIEW public_clips AS SELECT clip_id,source_uri,original_uri,name,created_at_utc,
    duration_seconds,duration_frames,fps_num,fps_den,selected,import_status,
    imported_media_uri,import_error,thumbnail_uri,final FROM clips;
CREATE VIEW public_filmstrip_artifacts AS
            SELECT * FROM filmstrip_artifacts;
CREATE VIEW public_filmstrip_frames AS
            SELECT * FROM filmstrip_frames;
CREATE VIEW public_ingest_runtime AS SELECT key,value,updated_at FROM ingest_runtime;
CREATE VIEW public_media_acquisitions AS
    SELECT media_uri, attempt_id, document_uri, clip_id, revision, acquisition_json FROM media_acquisitions;
CREATE VIEW public_media_documents AS
    SELECT document_uri, media_type, document_text FROM evidence_documents;
CREATE VIEW public_media_heads AS
    SELECT clip_id, source_index_uri, source_record_id, binding_json, revision FROM media_heads;
CREATE VIEW public_media_snapshot_documents AS
    SELECT clip_id, revision, document_uri FROM snapshot_documents;
CREATE VIEW public_media_snapshots AS
    SELECT clip_id, revision, phase, completeness, snapshot_json FROM media_snapshots;
CREATE VIEW public_prerender_units AS
             SELECT fingerprint, file_uri, frames, samples, encoder, key_json, media_json, rendered_at_utc
             FROM prerender_units;
CREATE VIEW public_probe_records AS SELECT * FROM probe_records;
CREATE VIEW public_project_application_sequence AS
            SELECT project_id, step_id, plugin_id AS application_id, tab_id, label, position,
                json_extract(settings_json, '$.priority_group') AS priority_group,
                status, next_step_id
            FROM project_workflow_steps;
CREATE VIEW public_project_data_revisions AS
            SELECT scope, revision, updated_at FROM project_data_revisions;
CREATE VIEW public_project_members AS
            SELECT project_id, user_id, role, joined_at, last_seen_at FROM project_members;
CREATE VIEW public_project_origin AS
            SELECT project_id, project_name, created_at,
                json_extract(identity_json, '$.contract_version') AS identity_contract_version,
                json_extract(identity_json, '$.workstation_name') AS workstation_name,
                json_extract(identity_json, '$.user_name') AS user_name,
                json_extract(identity_json, '$.hardware_serial.value') AS serial_number,
                json_extract(identity_json, '$.hardware_serial.kind') AS serial_kind,
                json_extract(identity_json, '$.hardware_serial.source') AS serial_source,
                identity_json
            FROM project_origin;
CREATE VIEW public_project_settings AS
            SELECT project_id, template_id, created_at, updated_at, settings_json FROM project_settings;
CREATE VIEW public_project_story_sheet AS
            SELECT sheet_json, saved_at_utc FROM project_story_sheet;
CREATE VIEW public_project_story_values AS
            SELECT code, value_json FROM project_story_values;
CREATE VIEW public_project_template_snapshot AS
            SELECT project_id, template_id, template_name, template_version, created_at
            FROM project_template_snapshot;
CREATE VIEW public_project_workflow_state AS
            SELECT project_id, active_step_id, entry_step_id, updated_at
            FROM project_workflow_state;
CREATE VIEW public_project_workflow_steps AS
            SELECT step_id, project_id, plugin_id, tab_id, label, position, status, next_step_id
            FROM project_workflow_steps;
CREATE VIEW public_script_blocks AS
            SELECT block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, voice_uri, voice_seconds, voice_tc, fields_json, pieces_json, updated_at_utc
            FROM script_blocks;
CREATE VIEW public_script_profile AS
            SELECT profile_id, profile_json, updated_at_utc FROM script_profile;
CREATE VIEW public_short_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'short';
CREATE VIEW public_source_media AS
    SELECT media_uri, record_id, role FROM media_references;
CREATE VIEW public_source_records AS
    SELECT record_id, source_uri, recording_root_uri, recording_identity,
           group_json, recorded_at_unix_ms FROM source_records;
CREATE VIEW public_source_support AS
    SELECT media_uri, record_id, role, kind, state FROM support_references;
CREATE VIEW public_story_covers AS
         SELECT cover_id, slot_id, slot_signature, slot_index, timeline_start_frame,
                timeline_end_frame, clip_id, virtual_shot_id, source_in_frame, source_out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, a2_source_channel,
                a2_level_db, a2_off, a2_lead_frames, a2_tail_frames
         FROM story_covers;
CREATE VIEW public_story_marker_slots AS
         SELECT s.slot_id, s.slot_index, s.start_frame, s.end_frame, s.duration_frames,
                s.start_marker_id, s.end_marker_id, s.slot_signature, s.updated_at,
                EXISTS(SELECT 1 FROM story_covers c WHERE c.slot_id = s.slot_id) AS has_cover
         FROM story_marker_slots s;
CREATE VIEW public_story_markers AS
         SELECT marker_id, timeline_frame AS program_frame, system_role
         FROM story_markers;
CREATE VIEW public_story_parts AS
         SELECT part_id AS segment_id, kind, sort_index, clip_id, in_frame, out_frame,
                source_fps_num AS fps_num, source_fps_den AS fps_den, active,
                duration_frames, duration_label, duration_color_key, a1_source_channel,
                a1_level_db, a1_off, v_lead_frames, lead, a1_lead_frames, a1_tail_frames, v_tail_frames
         FROM story_parts;
CREATE VIEW public_story_state AS
         SELECT selected_part_id, selected_shot_id, selected_slot_id, selected_cover_id,
                draft_updated_at, committed_at, updated_at, story_fps_num, story_fps_den
         FROM story_state;
CREATE VIEW public_transcript_queue AS
             SELECT clip_id, from_frame, to_frame, quality, state, error, took_ms, asked_at_utc FROM transcript_queue;
CREATE VIEW public_transcript_quotes AS
             SELECT id, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, text, added_at_utc FROM transcript_quotes;
CREATE VIEW public_wave_artifacts AS
            SELECT * FROM wave_artifacts;

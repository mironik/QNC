//! The artifact tables of a project database and their requests (moved unchanged
//! from the former project content store).

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 3] = ["filmstrip_artifacts", "filmstrip_frames", "wave_artifacts"];

/// Largest stored filmstrip record (as before: the content page budget).
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024 / 64;
/// Largest stored wave record: it carries the loudness of every 100 ms of each channel
/// (2026-10-24), about 90 KB a minute of four channels, so an hour-long clip fits; half of
/// the largest message of the project database transport (2026-10-25: a 4.8 min clip's full
/// wave was refused again and again at the old 256 KB, its A1 never measured).
const MAX_WAVE_RECORD_BYTES: usize = qnc_db_broker::MAX_BYTES / 2;

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the artifact tables if missing (read-write) and serves them; a
    /// read-only connection of a project without them reads none.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            ensure_filmstrip_schema(&tx)?;
            ensure_wave_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "table", "filmstrip_artifacts")?
            && object_exists(&conn, "table", "filmstrip_frames")?
            && object_exists(&conn, "table", "wave_artifacts")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        if !self.schema_ready {
            return match operation {
                Operation::ReadFilmstrip { clip_id } | Operation::ReadWave { clip_id } => {
                    qnc_media_records::valid_id(clip_id).map_err(err)?;
                    Ok(if matches!(operation, Operation::ReadWave { .. }) {
                        Data::Wave(None)
                    } else {
                        Data::Filmstrip(None)
                    })
                }
                _ => Err(qnc_ui_text::text("artifact.project_artifacts_initialised").into()),
            };
        }
        match operation {
            Operation::PublishFilmstrip(artifact) => self.publish_filmstrip(artifact),
            Operation::ReadFilmstrip { clip_id } => self.read_filmstrip(clip_id),
            Operation::PublishWave(artifact) => self.publish_wave(artifact),
            Operation::ReadWave { clip_id } => self.read_wave(clip_id),
            Operation::ForgetClips { clip_ids } => self.forget_clips(clip_ids),
        }
    }

    /// The artifacts of clips about to leave the catalog; a clip that is queued or
    /// imported keeps its artifacts (the catalog never removes it either).
    fn forget_clips(&mut self, clip_ids: &[String]) -> Result<Data> {
        if clip_ids.len() > 4096 {
            return Err(qnc_ui_text::text("db.too_many_clips_one_command").into());
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn)
            .map_err(err)?;
        let mut forgotten = Vec::new();
        for clip_id in clip_ids {
            qnc_media_records::valid_id(clip_id).map_err(err)?;
            let status = qnc_content_read::import_status_on(&tx, clip_id)?;
            if !matches!(status.as_deref(), Some("detected") | Some("failed")) {
                continue;
            }
            for table in OWNED {
                tx.execute(&format!("DELETE FROM {table} WHERE clip_id=?1"), [clip_id])
                    .map_err(err)?;
            }
            forgotten.push(clip_id.clone());
        }
        tx.commit().map_err(err)?;
        Ok(Data::Forgotten(forgotten))
    }

    fn publish_filmstrip(&mut self, artifact: &FilmstripArtifactRecord) -> Result<Data> {
        validate_filmstrip_artifact(artifact)?;
        let clip_exists =
            qnc_content_read::import_status_on(&self.conn, &artifact.clip_id)?.is_some();
        if !clip_exists {
            return Err(qnc_ui_text::text("db.clip_project_database").into());
        }
        let json = serde_json::to_string(artifact).map_err(err)?;
        if json.len() > MAX_RECORD_BYTES {
            return Err(qnc_ui_text::text("artifact.filmstrip_record_too_large").into());
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn)
            .map_err(err)?;
        let now = utc_stamp();
        tx.execute(
            "INSERT INTO filmstrip_artifacts
                (clip_id, frame_count, artifact_uri, created_at_utc, frames_json)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(clip_id) DO UPDATE SET
                frame_count=excluded.frame_count,
                artifact_uri=excluded.artifact_uri,
                created_at_utc=excluded.created_at_utc,
                frames_json=excluded.frames_json",
            params![
                artifact.clip_id,
                artifact.frame_count as i64,
                artifact.artifact_uri,
                now,
                json,
            ],
        )
        .map_err(err)?;
        tx.execute(
            "DELETE FROM filmstrip_frames WHERE clip_id=?1",
            [&artifact.clip_id],
        )
        .map_err(err)?;
        for frame in &artifact.frames {
            tx.execute(
                "INSERT INTO filmstrip_frames
                    (clip_id, frame_index, seek_sec, artifact_uri, updated_at_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    artifact.clip_id,
                    frame.index as i64,
                    parse_seconds(&frame.seek_sec)?,
                    frame.artifact_uri,
                    now,
                ],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_filmstrip(&self, clip_id: &str) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        let Some(mut artifact) = self
            .conn
            .query_row(
                "SELECT frames_json FROM filmstrip_artifacts WHERE clip_id=?1",
                [clip_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(err)?
            .map(|json| serde_json::from_str::<FilmstripArtifactRecord>(&json).map_err(err))
            .transpose()?
        else {
            return Ok(Data::Filmstrip(None));
        };
        let mut statement = self
            .conn
            .prepare(
                "SELECT frame_index, seek_sec, artifact_uri
                 FROM filmstrip_frames
                 WHERE clip_id=?1
                 ORDER BY frame_index",
            )
            .map_err(err)?;
        let frames = statement
            .query_map([clip_id], |row| {
                Ok(FilmstripFrameRecord {
                    index: row.get::<_, i64>(0)? as usize,
                    seek_sec: format!("{:.2}", row.get::<_, f64>(1)?),
                    artifact_uri: row.get(2)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        if !frames.is_empty() {
            artifact.frames = frames;
            artifact.frame_count = artifact.frames.len();
        }
        validate_filmstrip_artifact(&artifact)?;
        Ok(Data::Filmstrip(Some(artifact)))
    }

    fn publish_wave(&mut self, artifact: &WaveArtifactRecord) -> Result<Data> {
        validate_wave_artifact(artifact)?;
        let stored_source = qnc_content_read::media_uris_on(&self.conn, &artifact.clip_id)?;
        let Some((original_uri, proxy_uri)) = stored_source else {
            return Err(qnc_ui_text::text("db.clip_project_database").into());
        };
        if artifact.source_uri != original_uri
            && proxy_uri.as_deref() != Some(artifact.source_uri.as_str())
        {
            return Err(qnc_ui_text::text("artifact.wave_record_does_belong_stored_original").into());
        }
        let json = serde_json::to_string(artifact).map_err(err)?;
        if json.len() > MAX_WAVE_RECORD_BYTES {
            return Err(qnc_ui_text::text("artifact.wave_record_too_large").into());
        }
        let now = utc_stamp();
        self.conn
            .execute(
                "INSERT INTO wave_artifacts
                    (clip_id, artifact_uri, created_at_utc, peaks_json)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(clip_id) DO UPDATE SET
                    artifact_uri=excluded.artifact_uri,
                    created_at_utc=excluded.created_at_utc,
                    peaks_json=excluded.peaks_json",
                params![artifact.clip_id, artifact.artifact_uri, now, json],
            )
            .map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_wave(&self, clip_id: &str) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        let row = self
            .conn
            .query_row(
                "SELECT artifact_uri,peaks_json FROM wave_artifacts WHERE clip_id=?1",
                [clip_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(err)?;
        let Some((artifact_uri, json)) = row else {
            return Ok(Data::Wave(None));
        };
        let artifact = serde_json::from_str::<WaveArtifactRecord>(&json).map_err(err)?;
        if artifact.clip_id != clip_id || artifact.artifact_uri != artifact_uri {
            return Err(qnc_ui_text::text("artifact.wave_record_does_match_requested_clip").into());
        }
        validate_wave_artifact(&artifact)?;
        Ok(Data::Wave(Some(artifact)))
    }
}

fn ensure_filmstrip_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "filmstrip_artifacts")? {
        conn.execute_batch(
            "CREATE TABLE filmstrip_artifacts (
            clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id),
            frame_count INTEGER NOT NULL,
            artifact_uri TEXT NOT NULL,
            created_at_utc TEXT NOT NULL,
            frames_json TEXT NOT NULL
        );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_filmstrip_artifacts")? {
        conn.execute_batch(
            "CREATE VIEW public_filmstrip_artifacts AS
            SELECT * FROM filmstrip_artifacts;",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "table", "filmstrip_frames")? {
        conn.execute_batch(
            "CREATE TABLE filmstrip_frames (
            clip_id TEXT NOT NULL REFERENCES clips(clip_id),
            frame_index INTEGER NOT NULL,
            seek_sec REAL NOT NULL,
            artifact_uri TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL,
            PRIMARY KEY (clip_id, frame_index)
        );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_filmstrip_frames")? {
        conn.execute_batch(
            "CREATE VIEW public_filmstrip_frames AS
            SELECT * FROM filmstrip_frames;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn ensure_wave_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "wave_artifacts")? {
        conn.execute_batch(
            "CREATE TABLE wave_artifacts (
            clip_id TEXT PRIMARY KEY REFERENCES clips(clip_id),
            artifact_uri TEXT NOT NULL,
            created_at_utc TEXT NOT NULL,
            peaks_json TEXT NOT NULL
        );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_wave_artifacts")? {
        conn.execute_batch(
            "CREATE VIEW public_wave_artifacts AS
            SELECT * FROM wave_artifacts;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn validate_filmstrip_artifact(artifact: &FilmstripArtifactRecord) -> Result<()> {
    qnc_media_records::valid_id(&artifact.clip_id).map_err(err)?;
    qnc_media_records::validate_resource_uri(&artifact.artifact_uri).map_err(err)?;
    if !matches!(
        artifact.status.as_str(),
        "missing" | "building" | "ready" | "error"
    ) {
        return Err("Neispravan filmstrip status.".into());
    }
    parse_seconds(&artifact.duration_sec)?;
    if artifact.frame_count == 0
        || artifact.frame_count > 64
        || artifact.frames.len() != artifact.frame_count
    {
        return Err("Neispravan broj filmstrip slicica.".into());
    }
    for (expected, frame) in artifact.frames.iter().enumerate() {
        if frame.index != expected {
            return Err(qnc_ui_text::text("artifact.filmstrip_pictures_order").into());
        }
        parse_seconds(&frame.seek_sec)?;
        qnc_media_records::validate_resource_uri(&frame.artifact_uri).map_err(err)?;
        if !frame
            .artifact_uri
            .starts_with(&format!("{}/", artifact.artifact_uri.trim_end_matches('/')))
        {
            return Err("Filmstrip slicica ne pripada artifact rootu.".into());
        }
    }
    Ok(())
}

fn validate_wave_artifact(artifact: &WaveArtifactRecord) -> Result<()> {
    qnc_wave::validate_artifact(artifact)
}

fn parse_seconds(raw: &str) -> Result<f64> {
    let value = raw
        .parse::<f64>()
        .map_err(|_| "Neispravan filmstrip seek.")?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err("Neispravan filmstrip seek.".into())
    }
}

fn utc_stamp() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("unix_ms:{millis}")
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)",
        params![kind, name],
        |r| r.get(0),
    )
    .map_err(err)
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl Store {
    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

impl qnc_db_broker::OwnedTables for Store {
    const ID: &'static str = MODULE_ID;
    type Operation = Operation;
    type Data = Data;
    fn is_write(operation: &Operation) -> bool {
        operation.is_write()
    }
    fn attach(connection: Connection, access: Access) -> Result<Self> {
        Store::attach(connection, access)
    }
    fn execute(&mut self, operation: &Operation) -> Result<Data> {
        Store::execute(self, operation)
    }
    fn connection(&self) -> &Connection {
        Store::connection(self)
    }
}

//! Read-only reader of the project content DB contract. It reads only the
//! public views of `contracts/databases/ingest-content.database.json`
//! (`public_clips`, `public_probe_records`, `public_filmstrip_*`,
//! `public_wave_artifacts`), so any form or application can show the clips of
//! the active project without knowing which application produced them.
//!
//! Local project databases only for now: a database that resolves to a LAN or
//! intranet endpoint is refused with a controlled error, never read another
//! way. Nothing here writes, migrates, scans or probes.

use std::{path::PathBuf, time::Duration};

use qnc_filmstrip::{FilmstripArtifactRecord, FilmstripFrameRecord};
use qnc_transport_resolver::ResolvedEndpoint;
use qnc_wave::WaveArtifactRecord;
use qnc_work_settings::{SettingsReader, WorkSettings};
use rusqlite::{Connection, OpenFlags, OptionalExtension};

pub const MODULE_ID: &str = "qnc.module.content-read";
pub const VERSION: &str = "0.1.0";

const MAX_CLIPS: usize = 100_000;

#[derive(Debug, Clone, PartialEq)]
pub struct ClipSummary {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    /// Import finished (`imported` or `done`).
    pub imported: bool,
    /// Where the poster is: the project poster when it was copied, else the poster on the
    /// source (link). `None` when the catalog has no poster for the clip.
    pub thumbnail_uri: Option<String>,
}

/// What a player needs beyond the media record: display name, imported media
/// and where the full media record lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipHead {
    pub name: String,
    pub imported_media_uri: Option<String>,
    pub record_db_uri: String,
    pub record_revision: u32,
}

/// Lightweight catalog signature: equal signatures mean the displayed clip
/// list is still current, so the clips need not be loaded again.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogSignature {
    pub clip_count: u64,
    pub name_bytes: u64,
    pub frame_sum: i64,
    pub latest_created: String,
}

#[derive(Debug, Clone)]
pub struct ContentReader {
    file: PathBuf,
    project_id: String,
}

impl ContentReader {
    /// Resolves the project database of `settings` through the owner binding.
    pub fn for_project(reader: &SettingsReader, settings: &WorkSettings) -> Result<Self, String> {
        let owner = reader
            .workspace_binding(settings)
            .map_err(|error| error.to_string())?;
        let resolved = owner
            .resolver
            .resolve(&settings.workspace_db_uri)
            .map_err(|error| error.to_string())?;
        match resolved.endpoint {
            ResolvedEndpoint::LocalPath(file) => Ok(Self {
                file,
                project_id: settings.project_id.clone(),
            }),
            ResolvedEndpoint::NetworkEndpoint { .. } => Err(
                "Citanje projektnog sadrzaja preko LAN/Intranet transporta jos nije podrzano."
                    .to_string(),
            ),
        }
    }

    fn open(&self) -> Result<Connection, String> {
        if !self.file.is_file() {
            return Err("Projektna baza vise ne postoji.".to_string());
        }
        let conn = Connection::open_with_flags(&self.file, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| error.to_string())?;
        conn.busy_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        if has_view(&conn, "public_project_settings")? {
            let matches: bool = conn
                .query_row(
                    "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                    [&self.project_id],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if !matches {
                return Err("Projektna baza pripada drugom projektu.".to_string());
            }
        }
        Ok(conn)
    }

    /// The imported clips of the project, by name: what Uvezi wrote for import and the
    /// import finished. A clip that is only detected, selected or queued is not shown.
    /// Empty when the project has no clip catalog yet (nothing was ingested).
    pub fn summaries(&self) -> Result<Vec<ClipSummary>, String> {
        let conn = self.open()?;
        if !has_view(&conn, "public_clips")? {
            return Ok(Vec::new());
        }
        let poster = if has_column(&conn, "public_clips", "thumbnail_uri")? {
            "thumbnail_uri"
        } else {
            "NULL"
        };
        let mut statement = conn
            .prepare(&format!(
                "SELECT clip_id, name, duration_seconds, import_status IN ('imported', 'done'), {poster} FROM public_clips
                 WHERE import_status IN ('imported', 'done')
                 ORDER BY name, clip_id LIMIT ?1"
            ))
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([MAX_CLIPS as i64], |row| {
                Ok(ClipSummary {
                    clip_id: row.get(0)?,
                    name: row.get(1)?,
                    duration_seconds: row.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                    imported: row.get(3)?,
                    thumbnail_uri: row.get(4)?,
                })
            })
            .map_err(|error| error.to_string())?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| error.to_string())
    }

    pub fn signature(&self) -> Result<CatalogSignature, String> {
        let conn = self.open()?;
        if !has_view(&conn, "public_clips")? {
            return Ok(CatalogSignature::default());
        }
        conn.query_row(
            "SELECT count(*), coalesce(sum(length(name)),0),
                    coalesce(sum(coalesce(duration_frames,0)),0),
                    coalesce(max(coalesce(created_at_utc,'')),'')
             FROM public_clips
             WHERE import_status IN ('imported', 'done')",
            [],
            |row| {
                Ok(CatalogSignature {
                    clip_count: row.get::<_, i64>(0)? as u64,
                    name_bytes: row.get::<_, i64>(1)? as u64,
                    frame_sum: row.get(2)?,
                    latest_created: row.get(3)?,
                })
            },
        )
        .map_err(|error| error.to_string())
    }

    pub fn clip_head(&self, clip_id: &str) -> Result<Option<ClipHead>, String> {
        qnc_media_records::valid_id(clip_id).map_err(|error| error.to_string())?;
        let conn = self.open()?;
        if !has_view(&conn, "public_clips")? || !has_view(&conn, "public_probe_records")? {
            return Ok(None);
        }
        conn.query_row(
            "SELECT c.name, c.imported_media_uri, p.record_db_uri, p.record_revision
             FROM public_clips c JOIN public_probe_records p ON p.clip_id = c.clip_id
             WHERE c.clip_id = ?1",
            [clip_id],
            |row| {
                Ok(ClipHead {
                    name: row.get(0)?,
                    imported_media_uri: row.get(1)?,
                    record_db_uri: row.get(2)?,
                    record_revision: row.get::<_, i64>(3)? as u32,
                })
            },
        )
        .optional()
        .map_err(|error| error.to_string())
    }

    pub fn filmstrip(&self, clip_id: &str) -> Result<Option<FilmstripArtifactRecord>, String> {
        qnc_media_records::valid_id(clip_id).map_err(|error| error.to_string())?;
        let conn = self.open()?;
        if !has_view(&conn, "public_filmstrip_artifacts")?
            || !has_view(&conn, "public_filmstrip_frames")?
        {
            return Ok(None);
        }
        let Some(json) = conn
            .query_row(
                "SELECT frames_json FROM public_filmstrip_artifacts WHERE clip_id = ?1",
                [clip_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let mut artifact: FilmstripArtifactRecord =
            serde_json::from_str(&json).map_err(|error| error.to_string())?;
        let mut statement = conn
            .prepare(
                "SELECT frame_index, seek_sec, artifact_uri FROM public_filmstrip_frames
                 WHERE clip_id = ?1 ORDER BY frame_index",
            )
            .map_err(|error| error.to_string())?;
        let frames = statement
            .query_map([clip_id], |row| {
                Ok(FilmstripFrameRecord {
                    index: row.get::<_, i64>(0)? as usize,
                    seek_sec: format!("{:.2}", row.get::<_, f64>(1)?),
                    artifact_uri: row.get(2)?,
                })
            })
            .map_err(|error| error.to_string())?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|error| error.to_string())?;
        if !frames.is_empty() {
            artifact.frame_count = frames.len();
            artifact.frames = frames;
        }
        Ok(Some(artifact))
    }

    pub fn wave(&self, clip_id: &str) -> Result<Option<WaveArtifactRecord>, String> {
        qnc_media_records::valid_id(clip_id).map_err(|error| error.to_string())?;
        let conn = self.open()?;
        if !has_view(&conn, "public_wave_artifacts")? {
            return Ok(None);
        }
        conn.query_row(
            "SELECT peaks_json FROM public_wave_artifacts WHERE clip_id = ?1",
            [clip_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .map(|json| serde_json::from_str(&json).map_err(|error| error.to_string()))
        .transpose()
    }
}

fn has_view(conn: &Connection, name: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = ?1)",
        [name],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The public views of the content DB contract, over minimal tables.
    const SCHEMA: &str = "
        CREATE TABLE clips (clip_id TEXT PRIMARY KEY, name TEXT, created_at_utc TEXT, import_status TEXT DEFAULT 'detected',
            duration_seconds REAL, duration_frames INTEGER, imported_media_uri TEXT);
        CREATE TABLE probe_records (clip_id TEXT, record_db_uri TEXT, record_revision INTEGER);
        CREATE TABLE filmstrip_artifacts (clip_id TEXT, frames_json TEXT);
        CREATE TABLE filmstrip_frames (clip_id TEXT, frame_index INTEGER, seek_sec REAL, artifact_uri TEXT);
        CREATE TABLE wave_artifacts (clip_id TEXT, peaks_json TEXT);
        CREATE VIEW public_clips AS SELECT clip_id,name,created_at_utc,duration_seconds,
            duration_frames,imported_media_uri,import_status FROM clips;
        CREATE VIEW public_probe_records AS SELECT * FROM probe_records;
        CREATE VIEW public_filmstrip_artifacts AS SELECT * FROM filmstrip_artifacts;
        CREATE VIEW public_filmstrip_frames AS SELECT * FROM filmstrip_frames;
        CREATE VIEW public_wave_artifacts AS SELECT * FROM wave_artifacts;
        INSERT INTO clips (clip_id,name,created_at_utc,duration_seconds,duration_frames,imported_media_uri,import_status) VALUES ('clip-b','Beta','2026-09-01',20.5,1000,NULL,'imported');
        INSERT INTO clips (clip_id,name,created_at_utc,duration_seconds,duration_frames,imported_media_uri) VALUES ('clip-a','Alfa','2026-09-02',NULL,NULL,'qnc://local/x/a.mp4');
        INSERT INTO probe_records VALUES ('clip-a','qnc://local/db/media_records',3);
        INSERT INTO filmstrip_artifacts VALUES ('clip-a',
            '{\"clip_id\":\"clip-a\",\"status\":\"ready\",\"duration_sec\":\"5.00\",\"frame_count\":1,\"artifact_uri\":\"qnc://local/f\",\"frames\":[]}');
        INSERT INTO filmstrip_frames VALUES ('clip-a',1,2.5,'qnc://local/f/2.jpg');
        INSERT INTO filmstrip_frames VALUES ('clip-a',0,0.0,'qnc://local/f/1.jpg');
    ";

    fn reader_for(setup: &str) -> (tempfile::TempDir, ContentReader) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("project.db");
        let conn = Connection::open(&file).unwrap();
        conn.execute_batch(setup).unwrap();
        drop(conn);
        (
            dir,
            ContentReader {
                file,
                project_id: "project-1".into(),
            },
        )
    }

    #[test]
    fn only_imported_clips_are_listed_by_name() {
        let (dir, reader) = reader_for(SCHEMA);
        // `Alfa` was only detected: Uvezi did not write it for import.
        let clips = reader.summaries().unwrap();
        assert_eq!(clips.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Beta"]);
        assert_eq!(clips[0].duration_seconds, 20.5);
        assert!(clips[0].imported);
        Connection::open(dir.path().join("project.db"))
            .unwrap()
            .execute("UPDATE clips SET import_status='imported' WHERE clip_id='clip-a'", [])
            .unwrap();
        let clips = reader.summaries().unwrap();
        assert_eq!(clips.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Alfa", "Beta"]);
        assert_eq!(clips[0].duration_seconds, 0.0, "missing duration is zero");
    }

    #[test]
    fn the_poster_address_comes_from_the_public_view_when_the_catalog_has_it() {
        let (dir, reader) = reader_for(SCHEMA);
        // A catalog without poster addresses lists the clip without one.
        assert_eq!(reader.summaries().unwrap()[0].thumbnail_uri, None);
        let conn = Connection::open(dir.path().join("project.db")).unwrap();
        conn.execute_batch(
            "ALTER TABLE clips ADD COLUMN thumbnail_uri TEXT;
             DROP VIEW public_clips;
             CREATE VIEW public_clips AS SELECT clip_id,name,created_at_utc,duration_seconds,
                duration_frames,imported_media_uri,import_status,thumbnail_uri FROM clips;
             UPDATE clips SET thumbnail_uri='qnc://local/source/card/file/Thmbnl/b.JPG' WHERE clip_id='clip-b';",
        )
        .unwrap();
        let clips = reader.summaries().unwrap();
        assert_eq!(
            clips[0].thumbnail_uri.as_deref(),
            Some("qnc://local/source/card/file/Thmbnl/b.JPG")
        );
    }

    #[test]
    fn a_queued_or_selected_clip_is_not_listed_until_it_is_imported() {
        let (dir, reader) = reader_for(SCHEMA);
        Connection::open(dir.path().join("project.db"))
            .unwrap()
            .execute("UPDATE clips SET import_status='queued' WHERE clip_id='clip-a'", [])
            .unwrap();
        assert_eq!(reader.summaries().unwrap().len(), 1);
    }

    #[test]
    fn a_project_without_a_clip_catalog_is_empty_not_an_error() {
        let (_dir, reader) = reader_for("CREATE TABLE unrelated (x INTEGER);");
        assert!(reader.summaries().unwrap().is_empty());
        assert_eq!(reader.signature().unwrap(), CatalogSignature::default());
        assert!(reader.clip_head("clip-a").unwrap().is_none());
    }

    #[test]
    fn signature_changes_when_the_catalog_changes() {
        let (dir, reader) = reader_for(SCHEMA);
        let before = reader.signature().unwrap();
        assert_eq!(before.clip_count, 1, "only the imported clip counts");
        Connection::open(dir.path().join("project.db"))
            .unwrap()
            .execute("INSERT INTO clips (clip_id,name,created_at_utc,duration_seconds,duration_frames,import_status) VALUES ('clip-c','Gama',NULL,1.0,25,'imported')", [])
            .unwrap();
        assert_ne!(reader.signature().unwrap(), before);
    }

    #[test]
    fn clip_head_joins_the_public_probe_record() {
        let (_dir, reader) = reader_for(SCHEMA);
        let head = reader.clip_head("clip-a").unwrap().unwrap();
        assert_eq!(head.name, "Alfa");
        assert_eq!(head.imported_media_uri.as_deref(), Some("qnc://local/x/a.mp4"));
        assert_eq!(head.record_db_uri, "qnc://local/db/media_records");
        assert_eq!(head.record_revision, 3);
        // Beta has no probe record: nothing to play.
        assert!(reader.clip_head("clip-b").unwrap().is_none());
        assert!(reader.clip_head("not a valid id!").is_err());
    }

    #[test]
    fn filmstrip_frames_come_from_the_public_frame_view_in_order() {
        let (_dir, reader) = reader_for(SCHEMA);
        let record = reader.filmstrip("clip-a").unwrap().unwrap();
        assert_eq!(record.frame_count, 2);
        assert_eq!(record.frames[0].artifact_uri, "qnc://local/f/1.jpg");
        assert_eq!(record.frames[1].seek_sec, "2.50");
        assert!(reader.filmstrip("clip-b").unwrap().is_none());
    }

    #[test]
    fn a_database_of_another_project_is_refused() {
        let (_dir, reader) = reader_for(&format!(
            "{SCHEMA} CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings VALUES ('other-project');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;"
        ));
        assert!(reader.summaries().unwrap_err().contains("drugom projektu"));
    }

    #[test]
    fn a_missing_database_is_a_controlled_error() {
        let reader = ContentReader {
            file: PathBuf::from("/definitely/not/here.db"),
            project_id: "p".into(),
        };
        assert!(reader.summaries().is_err());
    }
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(|error| error.to_string())?;
    let mut rows = statement.query([]).map_err(|error| error.to_string())?;
    while let Some(row) = rows.next().map_err(|error| error.to_string())? {
        if row.get::<_, String>(1).map_err(|error| error.to_string())? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

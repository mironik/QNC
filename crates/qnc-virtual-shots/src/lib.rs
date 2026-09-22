//! Writes one virtual short into the active project database.
//!
//! The caller passes the project database file already resolved from the
//! active project. This module does not scan, probe, or copy fps. IN and OUT
//! are source frames the Broadcast Player already confirmed.

use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

pub const MODULE_ID: &str = "qnc.module.virtual-shots";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedShort {
    pub shot_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
}

/// One short already stored for the Virtual tab. Ordered by creation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortClip {
    pub shot_id: String,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub name: String,
    pub in_still_uri: Option<String>,
    pub out_still_uri: Option<String>,
    pub still_status: String,
}

/// Shorts for the Virtual tab, oldest first. An absent table is an empty list.
pub fn list_shorts(database_file: &Path) -> Result<Vec<ShortClip>, String> {
    if !database_file.is_file() {
        return Ok(Vec::new());
    }
    let conn = Connection::open_with_flags(
        database_file,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| error.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    let present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'view' AND name = 'public_short_clips'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if present == 0 {
        return Ok(Vec::new());
    }
    if short_view_has_column(&conn, "still_status")? {
        let mut statement = conn
            .prepare(
                "SELECT shot_id, clip_id, in_frame, out_frame, name,
                        in_still_uri, out_still_uri, still_status
                 FROM public_short_clips
                 ORDER BY created_at_utc, shot_id",
            )
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| {
                Ok(ShortClip {
                    shot_id: row.get(0)?,
                    clip_id: row.get(1)?,
                    in_frame: row.get::<_, i64>(2)?.max(0) as u64,
                    out_frame: row.get::<_, i64>(3)?.max(0) as u64,
                    name: row.get(4)?,
                    in_still_uri: row.get(5)?,
                    out_still_uri: row.get(6)?,
                    still_status: row.get(7)?,
                })
            })
            .map_err(|error| error.to_string())?;
        return rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string());
    }
    let mut statement = conn
        .prepare(
            "SELECT shot_id, clip_id, in_frame, out_frame, name
             FROM public_short_clips
             ORDER BY created_at_utc, shot_id",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok(ShortClip {
                shot_id: row.get(0)?,
                clip_id: row.get(1)?,
                in_frame: row.get::<_, i64>(2)?.max(0) as u64,
                out_frame: row.get::<_, i64>(3)?.max(0) as u64,
                name: row.get(4)?,
                in_still_uri: None,
                out_still_uri: None,
                still_status: "pending".into(),
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

/// Stores one short. OUT must be at least one frame after IN. The clip must
/// already be imported. A source row for the same clip, when it exists, is
/// linked and is not created here.
pub fn save_short(
    database_file: &Path,
    project_id: &str,
    clip_id: &str,
    clip_name: &str,
    in_frame: u64,
    out_frame: u64,
) -> Result<SavedShort, String> {
    if project_id.trim().is_empty() {
        return Err("Nema aktivnog projekta.".into());
    }
    if clip_id.is_empty() || clip_id.contains(['/', '\\']) {
        return Err("Klip nije pronadjen.".into());
    }
    if out_frame <= in_frame {
        return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
    }
    if !database_file.is_file() {
        return Err("Projektna baza nije dostupna za upis.".into());
    }

    let conn = Connection::open_with_flags(database_file, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|error| error.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|error| error.to_string())?;
    let saved = (|| {
        ensure_schema(&conn)?;
        require_project(&conn, project_id)?;
        require_imported(&conn, clip_id)?;
        let source_shot_id = source_row(&conn, clip_id)?;
        let index = next_index(&conn, clip_id)?;
        let shot_id = format!("{clip_id}_shot_{index:03}");
        let name = format!("{} {index:03}", clip_name.trim());
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        conn.execute(
            "INSERT INTO virtual_shots (
                shot_id, clip_id, class, in_frame, out_frame, source_shot_id, name,
                created_at_utc, still_status
             ) VALUES (?1, ?2, 'short', ?3, ?4, ?5, ?6, ?7, 'pending')",
            rusqlite::params![
                shot_id,
                clip_id,
                in_frame as i64,
                out_frame as i64,
                source_shot_id,
                name,
                created
            ],
        )
        .map_err(|error| error.to_string())?;
        Ok(SavedShort {
            shot_id,
            in_frame,
            out_frame,
        })
    })();
    match saved {
        Ok(saved) => {
            conn.execute_batch("COMMIT")
                .map_err(|error| error.to_string())?;
            Ok(saved)
        }
        Err(error) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

/// Publishes the still artifacts of a short that was already inserted by
/// `save_short`. The caller is responsible for writing the files first.
pub fn mark_stills_ready(
    database_file: &Path,
    shot_id: &str,
    in_uri: &str,
    out_uri: &str,
) -> Result<(), String> {
    update_stills(
        database_file,
        shot_id,
        "ready",
        Some(in_uri),
        Some(out_uri),
        None,
    )
}

/// Records that the fast still path failed; the saved short remains valid and a
/// later background worker can retry from the DB row.
pub fn mark_stills_failed(database_file: &Path, shot_id: &str, error: &str) -> Result<(), String> {
    update_stills(database_file, shot_id, "failed", None, None, Some(error))
}

fn update_stills(
    database_file: &Path,
    shot_id: &str,
    status: &str,
    in_uri: Option<&str>,
    out_uri: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    if shot_id.trim().is_empty() {
        return Err("Virtualni kadar nije pronadjen.".into());
    }
    if !database_file.is_file() {
        return Err("Projektna baza nije dostupna za upis.".into());
    }
    let conn = Connection::open_with_flags(database_file, OpenFlags::SQLITE_OPEN_READ_WRITE)
        .map_err(|error| error.to_string())?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|error| error.to_string())?;
    ensure_schema(&conn)?;
    let changed = conn
        .execute(
            "UPDATE virtual_shots
             SET still_status = ?2,
                 in_still_uri = COALESCE(?3, in_still_uri),
                 out_still_uri = COALESCE(?4, out_still_uri),
                 still_error = ?5
             WHERE shot_id = ?1 AND class = 'short'",
            rusqlite::params![shot_id, status, in_uri, out_uri, error],
        )
        .map_err(|error| error.to_string())?;
    if changed == 0 {
        Err("Virtualni kadar nije pronadjen.".into())
    } else {
        Ok(())
    }
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS virtual_shots (
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
        );",
    )
    .map_err(|error| error.to_string())?;
    add_column_if_missing(conn, "in_still_uri", "TEXT")?;
    add_column_if_missing(conn, "out_still_uri", "TEXT")?;
    add_column_if_missing(
        conn,
        "still_status",
        "TEXT NOT NULL DEFAULT 'pending' CHECK (still_status IN ('pending', 'ready', 'failed'))",
    )?;
    add_column_if_missing(conn, "still_error", "TEXT")?;
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_short_clips;
        CREATE VIEW public_short_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'short'",
    )
    .map_err(|error| error.to_string())
}

fn add_column_if_missing(conn: &Connection, column: &str, definition: &str) -> Result<(), String> {
    let mut statement = conn
        .prepare("PRAGMA table_info(virtual_shots)")
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    conn.execute_batch(&format!(
        "ALTER TABLE virtual_shots ADD COLUMN {column} {definition}"
    ))
    .map_err(|error| error.to_string())
}

fn short_view_has_column(conn: &Connection, column: &str) -> Result<bool, String> {
    let mut statement = conn
        .prepare("PRAGMA table_info(public_short_clips)")
        .map_err(|error| error.to_string())?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(columns.iter().any(|existing| existing == column))
}

fn require_project(conn: &Connection, project_id: &str) -> Result<(), String> {
    let matches: bool = conn
        .query_row(
            "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
            [project_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    if matches {
        Ok(())
    } else {
        Err("Projektna baza pripada drugom projektu.".into())
    }
}

fn require_imported(conn: &Connection, clip_id: &str) -> Result<(), String> {
    let status: Option<String> = conn
        .query_row(
            "SELECT import_status FROM public_clips WHERE clip_id = ?1",
            [clip_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())?;
    match status.as_deref() {
        Some("imported") | Some("done") => Ok(()),
        Some(_) => Err(format!("Klip '{clip_id}' nije uvezen.")),
        None => Err("Klip nije pronadjen.".into()),
    }
}

fn source_row(conn: &Connection, clip_id: &str) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT shot_id FROM virtual_shots WHERE clip_id = ?1 AND class = 'source' LIMIT 1",
        [clip_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(|error| error.to_string())
}

fn next_index(conn: &Connection, clip_id: &str) -> Result<u32, String> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM virtual_shots WHERE clip_id = ?1 AND class = 'short'",
            [clip_id],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    Ok(u32::try_from(count).unwrap_or(0).saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("project.db");
        let conn = Connection::open(&file).unwrap();
        conn.execute_batch(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings (project_id) VALUES ('p1');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;
             CREATE TABLE clips (clip_id TEXT, import_status TEXT);
             INSERT INTO clips (clip_id, import_status) VALUES ('clip-a', 'imported');
             CREATE VIEW public_clips AS SELECT clip_id, import_status FROM clips;",
        )
        .unwrap();
        (dir, file)
    }

    #[test]
    fn short_stores_the_player_range() {
        let (_dir, file) = database();
        let saved = save_short(&file, "p1", "clip-a", "Mironik", 10, 40).unwrap();
        assert_eq!(saved.shot_id, "clip-a_shot_001");
        assert_eq!((saved.in_frame, saved.out_frame), (10, 40));
        let conn = Connection::open(&file).unwrap();
        let (class, source, name): (String, Option<String>, String) = conn
            .query_row(
                "SELECT class, source_shot_id, name FROM virtual_shots WHERE shot_id = ?1",
                [&saved.shot_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(class, "short");
        assert_eq!(source, None);
        assert_eq!(name, "Mironik 001");
    }

    #[test]
    fn short_links_an_existing_source_row() {
        let (_dir, file) = database();
        save_short(&file, "p1", "clip-a", "Mironik", 0, 5).unwrap();
        let conn = Connection::open(&file).unwrap();
        conn.execute(
            "INSERT INTO virtual_shots (
                shot_id, clip_id, class, in_frame, out_frame, source_shot_id, name, created_at_utc
             ) VALUES ('root_clip-a', 'clip-a', 'source', 0, 100, NULL, 'Mironik', 1)",
            [],
        )
        .unwrap();
        drop(conn);
        let saved = save_short(&file, "p1", "clip-a", "Mironik", 12, 20).unwrap();
        let conn = Connection::open(&file).unwrap();
        let source: String = conn
            .query_row(
                "SELECT source_shot_id FROM virtual_shots WHERE shot_id = ?1",
                [&saved.shot_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(source, "root_clip-a");
        assert_eq!(saved.shot_id, "clip-a_shot_002");
    }

    #[test]
    fn refuses_a_clip_that_is_not_imported_and_an_empty_range() {
        let (_dir, file) = database();
        let conn = Connection::open(&file).unwrap();
        conn.execute(
            "INSERT INTO clips (clip_id, import_status) VALUES ('clip-b', 'queued')",
            [],
        )
        .unwrap();
        drop(conn);
        assert!(save_short(&file, "p1", "clip-b", "B", 0, 2)
            .unwrap_err()
            .contains("nije uvezen"));
        assert!(save_short(&file, "p1", "clip-a", "A", 5, 5).is_err());
    }

    #[test]
    fn virtual_tab_lists_shorts_oldest_first_and_skips_a_database_without_them() {
        let (_dir, file) = database();
        assert!(list_shorts(&file).unwrap().is_empty());
        save_short(&file, "p1", "clip-a", "Mironik", 10, 40).unwrap();
        save_short(&file, "p1", "clip-a", "Mironik", 40, 70).unwrap();
        let shorts = list_shorts(&file).unwrap();
        assert_eq!(
            shorts
                .iter()
                .map(|shot| shot.shot_id.as_str())
                .collect::<Vec<_>>(),
            vec!["clip-a_shot_001", "clip-a_shot_002"]
        );
        assert_eq!(shorts[0].name, "Mironik 001");
        assert_eq!((shorts[0].in_frame, shorts[0].out_frame), (10, 40));
        assert_eq!(shorts[0].still_status, "pending");
    }

    #[test]
    fn still_uris_are_published_after_the_short_exists() {
        let (_dir, file) = database();
        let saved = save_short(&file, "p1", "clip-a", "Mironik", 10, 40).unwrap();
        mark_stills_ready(
            &file,
            &saved.shot_id,
            "qnc://local/project/p1/virtual_shorts/clip-a_shot_001/in.jpg",
            "qnc://local/project/p1/virtual_shorts/clip-a_shot_001/out.jpg",
        )
        .unwrap();
        let shorts = list_shorts(&file).unwrap();
        assert_eq!(shorts[0].still_status, "ready");
        assert_eq!(
            shorts[0].in_still_uri.as_deref(),
            Some("qnc://local/project/p1/virtual_shorts/clip-a_shot_001/in.jpg")
        );
    }

    #[test]
    fn list_shorts_reads_the_old_view_without_writing_schema() {
        let (_dir, file) = database();
        let conn = Connection::open(&file).unwrap();
        conn.execute_batch(
            "CREATE TABLE virtual_shots (
                shot_id TEXT PRIMARY KEY,
                clip_id TEXT NOT NULL,
                class TEXT NOT NULL,
                in_frame INTEGER NOT NULL,
                out_frame INTEGER NOT NULL,
                source_shot_id TEXT,
                name TEXT NOT NULL,
                created_at_utc INTEGER NOT NULL
             );
             INSERT INTO virtual_shots (
                shot_id, clip_id, class, in_frame, out_frame, source_shot_id, name, created_at_utc
             ) VALUES ('clip-a_shot_001', 'clip-a', 'short', 10, 40, NULL, 'Mironik 001', 1);
             CREATE VIEW public_short_clips AS
             SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc
             FROM virtual_shots
             WHERE class = 'short';",
        )
        .unwrap();
        drop(conn);
        let shorts = list_shorts(&file).unwrap();
        assert_eq!(shorts[0].shot_id, "clip-a_shot_001");
        assert_eq!(shorts[0].still_status, "pending");
        assert!(shorts[0].in_still_uri.is_none());
    }
}

//! The virtual shot table of a project database and its requests (moved unchanged
//! from the former project content store; v5 `virtual_shots`).

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the virtual shot table if missing (read-write) and serves it; a
    /// read-only connection of a project without one reads no shots.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            ensure_virtual_shots_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "table", "virtual_shots")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
            use rusqlite::hooks::{AuthAction as A, Authorization as R};
            match ctx.action {
                A::Insert { table_name }
                | A::Delete { table_name }
                | A::Update { table_name, .. }
                    if access == Access::ReadWrite && table_name == "virtual_shots" =>
                {
                    R::Allow
                }
                A::Read { .. }
                | A::Select
                | A::Transaction { .. }
                | A::Savepoint { .. }
                | A::Function { .. }
                | A::Recursive => R::Allow,
                _ => R::Deny,
            }
        }));
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        if !self.schema_ready {
            return match operation {
                Operation::ListShorts | Operation::ListBroll => Ok(Data::ShortClips(Vec::new())),
                _ => Err("Virtualni kadrovi projekta nisu inicijalizirani.".into()),
            };
        }
        match operation {
            Operation::SaveShort {
                project_id,
                clip_id,
                clip_name,
                in_frame,
                out_frame,
            } => self.save_short(project_id, clip_id, clip_name, *in_frame, *out_frame),
            Operation::CreateCoverShot {
                project_id,
                clip_id,
                clip_name,
                in_frame,
                out_frame,
            } => self.create_cover_shot(project_id, clip_id, clip_name, (*in_frame, *out_frame)),
            Operation::ListShorts => self.list_shorts(),
            Operation::ListBroll => self.list_b_roll(),
            Operation::MarkShortStills {
                shot_id,
                status,
                in_uri,
                out_uri,
                error,
            } => self.mark_short_stills(
                shot_id,
                status,
                in_uri.as_deref(),
                out_uri.as_deref(),
                error.as_deref(),
            ),
        }
    }

    fn save_short(
        &mut self,
        project_id: &str,
        clip_id: &str,
        clip_name: &str,
        in_frame: u64,
        out_frame: u64,
    ) -> Result<Data> {
        if project_id.trim().is_empty() {
            return Err("Nema aktivnog projekta.".into());
        }
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
        }
        let project_matches: bool = self
            .conn
            .query_row(
                "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                [project_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        if !project_matches {
            return Err("Projektna baza pripada drugom projektu.".into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let source_shot_id = source_row(&tx, clip_id)?;
        let index = next_short_index(&tx, clip_id)?;
        let shot_id = format!("{clip_id}_shot_{index:03}");
        let name = format!("{} {index:03}", clip_name.trim());
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        tx.execute(
            "INSERT INTO virtual_shots (
                shot_id, clip_id, class, in_frame, out_frame, source_shot_id, name,
                created_at_utc, still_status
             ) VALUES (?1, ?2, 'short', ?3, ?4, ?5, ?6, ?7, 'pending')",
            params![
                shot_id,
                clip_id,
                in_frame as i64,
                out_frame as i64,
                source_shot_id,
                name,
                created
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::SavedShort(SavedShort {
            shot_id,
            in_frame,
            out_frame,
        }))
    }

    fn list_shorts(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_short_clips")? {
            return Ok(Data::ShortClips(Vec::new()));
        }
        let current_sql = "SELECT shot_id, clip_id, in_frame, out_frame, name,
                in_still_uri, out_still_uri, still_status
            FROM public_short_clips
            ORDER BY created_at_utc, shot_id";
        if let Ok(mut statement) = self.conn.prepare(current_sql) {
            let rows = statement
                .query_map([], short_clip_row)
                .map_err(err)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(err)?;
            return Ok(Data::ShortClips(rows));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT shot_id, clip_id, in_frame, out_frame, name
                 FROM public_short_clips
                 ORDER BY created_at_utc, shot_id",
            )
            .map_err(err)?;
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
                    b_roll: false,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::ShortClips(rows))
    }

    /// v5 B-roll tab: the virtual shots of the covers.
    fn list_b_roll(&self) -> Result<Data> {
        if !object_exists(&self.conn, "view", "public_b_roll_clips")? {
            return Ok(Data::ShortClips(Vec::new()));
        }
        let mut statement = self
            .conn
            .prepare(
                "SELECT shot_id, clip_id, in_frame, out_frame, name,
                    in_still_uri, out_still_uri, still_status
                 FROM public_b_roll_clips
                 ORDER BY created_at_utc, shot_id",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                short_clip_row(row).map(|shot| ShortClip {
                    b_roll: true,
                    ..shot
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::ShortClips(rows))
    }

    fn mark_short_stills(
        &mut self,
        shot_id: &str,
        status: &str,
        in_uri: Option<&str>,
        out_uri: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<Data> {
        if shot_id.trim().is_empty() {
            return Err("Virtualni kadar nije pronadjen.".into());
        }
        if !matches!(status, "ready" | "failed" | "pending") {
            return Err("Neispravan status slicica virtualnog kadra.".into());
        }
        if let Some(uri) = in_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(err)?;
        }
        if let Some(uri) = out_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(err)?;
        }
        if error_message.is_some_and(|value| value.len() > 4096) {
            return Err("Prevelika poruka greske.".into());
        }
        let changed = self
            .conn
            .execute(
                "UPDATE virtual_shots
                 SET still_status = ?2,
                     in_still_uri = COALESCE(?3, in_still_uri),
                     out_still_uri = COALESCE(?4, out_still_uri),
                     still_error = ?5
                 WHERE shot_id = ?1 AND class IN ('short', 'b_roll')",
                params![shot_id, status, in_uri, out_uri, error_message],
            )
            .map_err(err)?;
        if changed == 0 {
            Err("Virtualni kadar nije pronadjen.".into())
        } else {
            Ok(Data::Changed)
        }
    }
    /// v5 `add_virtual_shot_from_frames` for a cover (category `cover`): the source
    /// IN/OUT becomes a B-roll virtual shot; the Story module then puts it in a slot.
    fn create_cover_shot(
        &mut self,
        project_id: &str,
        clip_id: &str,
        clip_name: &str,
        (in_frame, out_frame): (u64, u64),
    ) -> Result<Data> {
        qnc_media_records::valid_id(clip_id).map_err(err)?;
        if out_frame <= in_frame {
            return Err("OUT mora biti najmanje jedan frame nakon IN.".into());
        }
        let project_matches: bool = self
            .conn
            .query_row(
                "SELECT count(*)=1 AND min(project_id)=?1 FROM public_project_settings",
                [project_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        if !project_matches {
            return Err("Projektna baza pripada drugom projektu.".into());
        }
        require_imported_clip(&self.conn, clip_id)?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let source_shot_id = source_row(&tx, clip_id)?;
        let count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM virtual_shots WHERE clip_id = ?1 AND class = 'b_roll'",
                [clip_id],
                |row| row.get(0),
            )
            .map_err(err)?;
        let index = count + 1;
        let shot_id = format!("{clip_id}_broll_{index:03}");
        let name = format!("{} B{index:03}", clip_name.trim());
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        tx.execute(
            "INSERT INTO virtual_shots (
                shot_id, clip_id, class, in_frame, out_frame, source_shot_id, name,
                created_at_utc, still_status
             ) VALUES (?1, ?2, 'b_roll', ?3, ?4, ?5, ?6, ?7, 'pending')",
            params![
                shot_id,
                clip_id,
                in_frame as i64,
                out_frame as i64,
                source_shot_id,
                name,
                created as i64
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Created(shot_id))
    }
}

fn ensure_virtual_shots_schema(conn: &Connection) -> Result<()> {
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
    .map_err(err)?;
    add_virtual_column_if_missing(conn, "in_still_uri", "TEXT")?;
    add_virtual_column_if_missing(conn, "out_still_uri", "TEXT")?;
    add_virtual_column_if_missing(
        conn,
        "still_status",
        "TEXT NOT NULL DEFAULT 'pending' CHECK (still_status IN ('pending', 'ready', 'failed'))",
    )?;
    add_virtual_column_if_missing(conn, "still_error", "TEXT")?;
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_short_clips;
        CREATE VIEW public_short_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'short';
        DROP VIEW IF EXISTS public_b_roll_clips;
        CREATE VIEW public_b_roll_clips AS
        SELECT shot_id, clip_id, in_frame, out_frame, source_shot_id, name, created_at_utc,
               in_still_uri, out_still_uri, still_status, still_error
        FROM virtual_shots
        WHERE class = 'b_roll';",
    )
    .map_err(err)
}

fn add_virtual_column_if_missing(conn: &Connection, column: &str, definition: &str) -> Result<()> {
    let mut statement = conn
        .prepare("PRAGMA table_info(virtual_shots)")
        .map_err(err)?;
    let columns = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    if columns.iter().any(|existing| existing == column) {
        return Ok(());
    }
    conn.execute_batch(&format!(
        "ALTER TABLE virtual_shots ADD COLUMN {column} {definition}"
    ))
    .map_err(err)
}

fn source_row(conn: &Connection, clip_id: &str) -> Result<Option<String>> {
    conn.query_row(
        "SELECT shot_id FROM virtual_shots WHERE clip_id = ?1 AND class = 'source' LIMIT 1",
        [clip_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(err)
}

fn next_short_index(conn: &Connection, clip_id: &str) -> Result<u32> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM virtual_shots WHERE clip_id = ?1 AND class = 'short'",
            [clip_id],
            |row| row.get(0),
        )
        .map_err(err)?;
    Ok(u32::try_from(count).unwrap_or(0).saturating_add(1))
}

fn short_clip_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ShortClip> {
    Ok(ShortClip {
        shot_id: row.get(0)?,
        clip_id: row.get(1)?,
        in_frame: row.get::<_, i64>(2)?.max(0) as u64,
        out_frame: row.get::<_, i64>(3)?.max(0) as u64,
        name: row.get(4)?,
        in_still_uri: row.get(5)?,
        out_still_uri: row.get(6)?,
        still_status: row.get(7)?,
        b_roll: false,
    })
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)",
        params![kind, name],
        |r| r.get(0),
    )
    .map_err(err)
}

/// Only an imported clip gets a virtual shot, read through the one public reader of
/// the clip catalog.
fn require_imported_clip(conn: &Connection, clip_id: &str) -> Result<()> {
    match qnc_content_read::imported_on(conn, clip_id)? {
        Some(true) => Ok(()),
        Some(false) => Err(format!("Klip '{clip_id}' nije uvezen.")),
        None => Err("Klip nije pronadjen.".into()),
    }
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

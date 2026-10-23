//! The parts tables of a project database and their requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 2] = ["ingest_parts", "ingest_part_numbers"];
/// Parts listed at most (a card has some hundreds of clips).
const MAX_PARTS: usize = 4096;

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the tables if missing (read-write) and serves them; a project without them
    /// has no part.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "table", "ingest_parts")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
            use rusqlite::hooks::{AuthAction as A, Authorization as R};
            match ctx.action {
                A::Insert { table_name } | A::Delete { table_name } | A::Update { table_name, .. }
                    if access == Access::ReadWrite && OWNED.contains(&table_name) =>
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
        match operation {
            Operation::List if !self.schema_ready => Ok(Data::Parts(Vec::new())),
            _ if !self.schema_ready => Err(qnc_ui_text::text("parts.not_ready").into()),
            Operation::Save { clip_id, clip_name, in_frame, out_frame } => self.save(clip_id, clip_name, *in_frame, *out_frame),
            Operation::Delete { part_id } => {
                let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
                tx.execute("DELETE FROM ingest_parts WHERE part_id = ?1", params![part_id]).map_err(err)?;
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::List => self.list(),
            Operation::Select { part_id, selected } => {
                let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
                tx.execute("UPDATE ingest_parts SET selected = ?2 WHERE part_id = ?1", params![part_id, *selected]).map_err(err)?;
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
            Operation::MarkCut { part_id, cut_name } => {
                if cut_name.trim().is_empty() || cut_name.contains(['/', '\\']) {
                    return Err(qnc_ui_text::text("parts.needs_in_and_out").into());
                }
                let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
                tx.execute("UPDATE ingest_parts SET cut_name = ?2 WHERE part_id = ?1", params![part_id, cut_name]).map_err(err)?;
                tx.commit().map_err(err)?;
                Ok(Data::Changed)
            }
        }
    }

    /// A part of a clip of the catalog, inside its stored length; its id is never reused.
    fn save(&mut self, clip_id: &str, clip_name: &str, in_frame: u64, out_frame: u64) -> Result<Data> {
        if out_frame <= in_frame {
            return Err(qnc_ui_text::text("parts.needs_in_and_out").into());
        }
        // The one reader of the catalog (C11): the clip and its stored length.
        let frames = qnc_content_read::duration_frames_on(&self.conn, clip_id)?
            .ok_or_else(|| qnc_ui_text::text("parts.clip_not_in_catalog").to_string())?;
        if out_frame > frames {
            return Err(qnc_ui_text::fill("parts.past_clip_end", &[("frames", &frames.to_string())]));
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let last: Option<i64> = tx
            .query_row("SELECT last FROM ingest_part_numbers WHERE clip_id = ?1", [clip_id], |row| row.get(0))
            .optional()
            .map_err(err)?;
        let number = last.unwrap_or(0) + 1;
        tx.execute(
            "INSERT INTO ingest_part_numbers (clip_id, last) VALUES (?1, ?2)
             ON CONFLICT(clip_id) DO UPDATE SET last = excluded.last",
            params![clip_id, number],
        )
        .map_err(err)?;
        let part = Part {
            part_id: format!("{clip_id}-part-{number:02}"),
            clip_id: clip_id.into(),
            name: format!("{clip_name} P{number:02}"),
            in_frame,
            out_frame,
            cut_name: None,
            selected: true,
            seconds: 0.0,
        };
        tx.execute(
            "INSERT INTO ingest_parts (part_id, clip_id, name, in_frame, out_frame, created_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![part.part_id, part.clip_id, part.name, i64::try_from(in_frame).map_err(err)?, i64::try_from(out_frame).map_err(err)?],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Saved(part))
    }

    fn list(&mut self) -> Result<Data> {
        let mut statement = self
            .conn
            .prepare_cached(&format!(
                "SELECT part_id, clip_id, name, in_frame, out_frame, cut_name, selected FROM public_ingest_parts ORDER BY created_at_utc, part_id LIMIT {MAX_PARTS}"
            ))
            .map_err(err)?;
        let mut rows = statement.query([]).map_err(err)?;
        let mut parts = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            let frame = |index| -> Result<u64> { u64::try_from(row.get::<_, i64>(index).map_err(err)?).map_err(err) };
            parts.push(Part {
                part_id: row.get(0).map_err(err)?,
                clip_id: row.get(1).map_err(err)?,
                name: row.get(2).map_err(err)?,
                in_frame: frame(3)?,
                out_frame: frame(4)?,
                cut_name: row.get(5).map_err(err)?,
                selected: row.get(6).map_err(err)?,
                seconds: 0.0,
            });
        }
        // The length of each part at the stored frame rate of its clip (C11: the one reader).
        for part in &mut parts {
            if let Ok(Some((_, (num, den)))) = qnc_content_read::clip_timing_on(&self.conn, &part.clip_id) {
                part.seconds = (part.out_frame - part.in_frame) as f64 * f64::from(den) / f64::from(num.max(1));
            }
        }
        Ok(Data::Parts(parts))
    }

    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    // A development table of an earlier form is removed, never converted (no migrations).
    if object_exists(conn, "table", "ingest_parts")?
        && conn.query_row("SELECT COUNT(*) FROM pragma_table_info('ingest_parts') WHERE name='selected'", [], |r| r.get::<_, i64>(0)).map_err(err)? == 0
    {
        conn.execute_batch("DROP VIEW IF EXISTS public_ingest_parts; DROP TABLE ingest_parts;").map_err(err)?;
    }
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS ingest_part_numbers (
            clip_id TEXT PRIMARY KEY,
            last INTEGER NOT NULL CHECK (last > 0)
        );
        CREATE TABLE IF NOT EXISTS ingest_parts (
            part_id TEXT PRIMARY KEY,
            clip_id TEXT NOT NULL,
            name TEXT NOT NULL,
            in_frame INTEGER NOT NULL CHECK (in_frame >= 0),
            out_frame INTEGER NOT NULL,
            created_at_utc TEXT NOT NULL,
            cut_name TEXT,
            selected INTEGER NOT NULL DEFAULT 1,
            CHECK (out_frame > in_frame)
        );
        CREATE VIEW IF NOT EXISTS public_ingest_parts AS
        SELECT part_id, clip_id, name, in_frame, out_frame, created_at_utc, cut_name, selected FROM ingest_parts;",
    )
    .map_err(err)
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)", params![kind, name], |r| r.get(0))
        .map_err(err)
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

//! The transcript table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["clip_transcripts"];

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection of a
    /// project without it reads no transcript.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "view", "public_clip_transcripts")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::ReadClip { .. } if !self.schema_ready => Ok(Data::Transcripts(Vec::new())),
            Operation::ReadClip { clip_id } => self.read_clip(clip_id),
            Operation::Save(transcript) => self.save(transcript),
        }
    }

    fn save(&mut self, transcript: &ClipTranscript) -> Result<Data> {
        valid_clip(&transcript.clip_id)?;
        let in_range = |from: u64, to: u64| transcript.from_frame <= from && from <= to && to <= transcript.to_frame;
        let lines_fit = transcript.lines.iter().all(|line| in_range(line.from, line.to) && line.words.iter().all(|w| in_range(w.from, w.to)));
        if transcript.from_frame >= transcript.to_frame || !lines_fit || transcript.language.is_empty() {
            return Err(qnc_ui_text::text("db.transcript_invalid").into());
        }
        let lines = serde_json::to_string(&transcript.lines).map_err(err)?;
        let text = transcript.lines.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join(" ");
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO clip_transcripts (clip_id, from_frame, to_frame, language, engine, text, lines_json, made_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(clip_id, from_frame, to_frame) DO UPDATE SET language=excluded.language, engine=excluded.engine,
               text=excluded.text, lines_json=excluded.lines_json, made_at_utc=excluded.made_at_utc",
            params![
                transcript.clip_id,
                transcript.from_frame as i64,
                transcript.to_frame as i64,
                transcript.language,
                transcript.engine,
                text,
                lines
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_clip(&mut self, clip_id: &str) -> Result<Data> {
        valid_clip(clip_id)?;
        let mut statement = self
            .conn
            .prepare(
                "SELECT clip_id, from_frame, to_frame, language, engine, lines_json FROM public_clip_transcripts
                 WHERE clip_id = ?1 ORDER BY from_frame, to_frame",
            )
            .map_err(err)?;
        let rows = statement
            .query_map(params![clip_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?, row.get(3)?, row.get(4)?, row.get::<_, String>(5)?))
            })
            .map_err(err)?;
        let mut transcripts = Vec::new();
        for row in rows {
            let (clip_id, from, to, language, engine, lines) = row.map_err(err)?;
            let lines = serde_json::from_str(&lines).map_err(err)?;
            transcripts.push(ClipTranscript { clip_id, from_frame: from as u64, to_frame: to as u64, language, engine, lines });
        }
        Ok(Data::Transcripts(transcripts))
    }

    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn valid_clip(clip_id: &str) -> Result<()> {
    if clip_id.is_empty() || clip_id.len() > 200 || clip_id.chars().any(char::is_control) {
        return Err(qnc_ui_text::text("db.transcript_invalid").into());
    }
    Ok(())
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "clip_transcripts")? {
        conn.execute_batch(
            "CREATE TABLE clip_transcripts (
                clip_id TEXT NOT NULL,
                from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
                to_frame INTEGER NOT NULL CHECK (to_frame > from_frame),
                language TEXT NOT NULL,
                engine TEXT NOT NULL,
                text TEXT NOT NULL,
                lines_json TEXT NOT NULL,
                made_at_utc TEXT NOT NULL,
                PRIMARY KEY (clip_id, from_frame, to_frame)
            );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_clip_transcripts")? {
        conn.execute_batch(
            "CREATE VIEW public_clip_transcripts AS
             SELECT clip_id, from_frame, to_frame, language, engine, text, lines_json, made_at_utc
             FROM clip_transcripts;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)", params![kind, name], |r| r.get(0))
        .map_err(err)
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
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

//! The transcript table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 3] = ["clip_transcripts", "transcript_queue", "transcript_quotes"];

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
    queue_ready: bool,
    queue_took: bool,
    quotes_ready: bool,
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
        let queue_ready = object_exists(&conn, "view", "public_transcript_queue")?;
        let quotes_ready = object_exists(&conn, "view", "public_transcript_quotes")?;
        let queue_took = queue_ready
            && conn
                .query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('public_transcript_queue') WHERE name = 'took_ms')", [], |r| r.get(0))
                .map_err(err)?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready, queue_ready, queue_took, quotes_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::ReadClip { .. } if !self.schema_ready => Ok(Data::Transcripts(Vec::new())),
            Operation::ReadClip { clip_id } => self.read_clip(clip_id),
            Operation::Save(transcript) => self.save(transcript),
            Operation::ReadQueue if !self.queue_ready => Ok(Data::Queue(Vec::new())),
            Operation::ReadQueue => self.read_queue(),
            Operation::Enqueue(ranges) => self.enqueue(ranges),
            Operation::SetQueued { clip_id, state, error, took_ms } => self.set_queued(clip_id, *state, error, *took_ms),
            Operation::ReadQuotes if !self.quotes_ready => Ok(Data::Quotes(Vec::new())),
            Operation::ReadQuotes => self.read_quotes(),
            Operation::AddQuote(quote) => self.add_quote(quote),
            Operation::DeleteQuote { id } => self.delete_quote(*id),
            Operation::ReadTranscribed if !self.schema_ready => Ok(Data::Clips(Vec::new())),
            Operation::ReadTranscribed => self.read_transcribed(),
            Operation::ClearDone => self.clear_done(),
        }
    }

    fn read_transcribed(&mut self) -> Result<Data> {
        let mut statement = self.conn.prepare("SELECT DISTINCT clip_id FROM public_clip_transcripts ORDER BY clip_id").map_err(err)?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0)).map_err(err)?;
        Ok(Data::Clips(rows.collect::<std::result::Result<_, _>>().map_err(err)?))
    }

    fn clear_done(&mut self) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute("DELETE FROM transcript_queue WHERE state = 'done'", []).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn enqueue(&mut self, ranges: &[Queued]) -> Result<Data> {
        for range in ranges {
            valid_clip(&range.clip_id)?;
            if range.from_frame >= range.to_frame {
                return Err(qnc_ui_text::text("db.transcript_invalid").into());
            }
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        for range in ranges {
            tx.execute(
                "INSERT INTO transcript_queue (clip_id, from_frame, to_frame, quality, state, error, took_ms, asked_at_utc)
                 VALUES (?1, ?2, ?3, ?4, 'waiting', '', 0, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                 ON CONFLICT(clip_id) DO UPDATE SET from_frame=excluded.from_frame, to_frame=excluded.to_frame,
                   quality=excluded.quality, state='waiting', error='', took_ms=0, asked_at_utc=excluded.asked_at_utc",
                params![range.clip_id, range.from_frame as i64, range.to_frame as i64, quality_name(range.quality)],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn set_queued(&mut self, clip_id: &str, state: QueueState, error: &str, took_ms: u64) -> Result<Data> {
        valid_clip(clip_id)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "UPDATE transcript_queue SET state = ?2, error = ?3, took_ms = ?4 WHERE clip_id = ?1",
            params![clip_id, state_name(state), error, took_ms as i64],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_queue(&mut self) -> Result<Data> {
        let mut statement = self
            .conn
            .prepare(if self.queue_took {
                "SELECT clip_id, from_frame, to_frame, quality, state, error, took_ms FROM public_transcript_queue ORDER BY asked_at_utc, clip_id"
            } else {
                // A view of the first development form, until its owner next writes.
                "SELECT clip_id, from_frame, to_frame, quality, state, error, 0 FROM public_transcript_queue ORDER BY asked_at_utc, clip_id"
            })
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    (row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)?),
                    (row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get::<_, String>(5)?, row.get::<_, i64>(6)?),
                ))
            })
            .map_err(err)?;
        let mut queue = Vec::new();
        for row in rows {
            let ((clip_id, from, to), (quality, state, error, took_ms)) = row.map_err(err)?;
            queue.push(Queued {
                clip_id,
                from_frame: from as u64,
                to_frame: to as u64,
                quality: if quality == "best" { QueueQuality::Best } else { QueueQuality::Fast },
                state: match state.as_str() {
                    "running" => QueueState::Running,
                    "done" => QueueState::Done,
                    "failed" => QueueState::Failed,
                    _ => QueueState::Waiting,
                },
                error,
                took_ms: took_ms.max(0) as u64,
            });
        }
        Ok(Data::Queue(queue))
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
        // The newest transcript is what was said over its range (user 2026-10-26: "some text
        // repeats"): an older one of the clip loses its lines there, and goes when its range is
        // inside the new one.
        let older: Vec<(i64, i64, String)> = {
            let mut statement = tx
                .prepare(
                    "SELECT from_frame, to_frame, lines_json FROM clip_transcripts
                     WHERE clip_id = ?1 AND from_frame < ?3 AND to_frame > ?2 AND NOT (from_frame = ?2 AND to_frame = ?3)",
                )
                .map_err(err)?;
            let rows = statement
                .query_map(params![transcript.clip_id, transcript.from_frame as i64, transcript.to_frame as i64], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .map_err(err)?;
            rows.collect::<std::result::Result<_, _>>().map_err(err)?
        };
        for (from, to, lines_json) in older {
            if transcript.from_frame <= from as u64 && to as u64 <= transcript.to_frame {
                tx.execute("DELETE FROM clip_transcripts WHERE clip_id = ?1 AND from_frame = ?2 AND to_frame = ?3", params![transcript.clip_id, from, to])
                    .map_err(err)?;
                continue;
            }
            let lines: Vec<Line> = serde_json::from_str(&lines_json).map_err(err)?;
            let kept: Vec<Line> = lines.into_iter().filter(|line| !(transcript.from_frame..transcript.to_frame).contains(&((line.from + line.to) / 2))).collect();
            let text = kept.iter().map(|line| line.text.as_str()).collect::<Vec<_>>().join(" ");
            tx.execute(
                "UPDATE clip_transcripts SET lines_json = ?4, text = ?5 WHERE clip_id = ?1 AND from_frame = ?2 AND to_frame = ?3",
                params![transcript.clip_id, from, to, serde_json::to_string(&kept).map_err(err)?, text],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn add_quote(&mut self, quote: &Quote) -> Result<Data> {
        valid_clip(&quote.clip_id)?;
        if quote.from_frame >= quote.to_frame || quote.text.trim().is_empty() {
            return Err(qnc_ui_text::text("db.transcript_invalid").into());
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO transcript_quotes (clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, text, added_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![quote.clip_id, quote.clip_name, quote.from_frame as i64, quote.to_frame as i64, quote.start_tc, quote.end_tc, quote.length, quote.text],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn delete_quote(&mut self, id: i64) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute("DELETE FROM transcript_quotes WHERE id = ?1", params![id]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_quotes(&mut self) -> Result<Data> {
        let mut statement = self
            .conn
            .prepare("SELECT id, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, text FROM public_transcript_quotes ORDER BY id")
            .map_err(err)?;
        let rows = statement
            .query_map([], |row| {
                Ok(Quote {
                    id: row.get(0)?,
                    clip_id: row.get(1)?,
                    clip_name: row.get(2)?,
                    from_frame: row.get::<_, i64>(3)?.max(0) as u64,
                    to_frame: row.get::<_, i64>(4)?.max(0) as u64,
                    start_tc: row.get(5)?,
                    end_tc: row.get(6)?,
                    length: row.get(7)?,
                    text: row.get(8)?,
                })
            })
            .map_err(err)?;
        Ok(Data::Quotes(rows.collect::<std::result::Result<_, _>>().map_err(err)?))
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

fn quality_name(quality: QueueQuality) -> &'static str {
    match quality {
        QueueQuality::Fast => "fast",
        QueueQuality::Best => "best",
    }
}

fn state_name(state: QueueState) -> &'static str {
    match state {
        QueueState::Waiting => "waiting",
        QueueState::Running => "running",
        QueueState::Done => "done",
        QueueState::Failed => "failed",
    }
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "transcript_queue")? {
        conn.execute_batch(
            "CREATE TABLE transcript_queue (
                clip_id TEXT PRIMARY KEY,
                from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
                to_frame INTEGER NOT NULL CHECK (to_frame > from_frame),
                quality TEXT NOT NULL CHECK (quality IN ('fast', 'best')),
                state TEXT NOT NULL CHECK (state IN ('waiting', 'running', 'done', 'failed')),
                error TEXT NOT NULL,
                took_ms INTEGER NOT NULL DEFAULT 0,
                asked_at_utc TEXT NOT NULL
            );",
        )
        .map_err(err)?;
    }
    // A queue of the first development form (without the time each clip took) gets the column
    // and its view anew; its entries stay.
    let has_took: bool = conn
        .query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('transcript_queue') WHERE name = 'took_ms')", [], |r| r.get(0))
        .map_err(err)?;
    if !has_took {
        conn.execute_batch(
            "ALTER TABLE transcript_queue ADD COLUMN took_ms INTEGER NOT NULL DEFAULT 0;
             DROP VIEW IF EXISTS public_transcript_queue;",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_transcript_queue")? {
        conn.execute_batch(
            "CREATE VIEW public_transcript_queue AS
             SELECT clip_id, from_frame, to_frame, quality, state, error, took_ms, asked_at_utc FROM transcript_queue;",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "table", "transcript_quotes")? {
        conn.execute_batch(
            "CREATE TABLE transcript_quotes (
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
            CREATE VIEW IF NOT EXISTS public_transcript_quotes AS
             SELECT id, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, text, added_at_utc FROM transcript_quotes;",
        )
        .map_err(err)?;
    }
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

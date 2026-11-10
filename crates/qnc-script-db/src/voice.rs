//! The voice records of the script's blocks (user 2026-10-30): one record per block of a
//! recorded kind, its takes in the order recorded, one of them used. The block keeps the clip,
//! length and timecode of the take used, as before, so every reader of a block sees it.

use crate::store::{err, view_exists, Store};
use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The most takes a record has (the channels of its clip).
pub const MAX_TAKES: u16 = 99;

pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS script_sessions (
            session_id INTEGER PRIMARY KEY AUTOINCREMENT,
            wav_uri TEXT NOT NULL CHECK (length(wav_uri) > 0),
            seconds REAL NOT NULL CHECK (seconds >= 0),
            tc TEXT NOT NULL,
            marks_json TEXT NOT NULL,
            recorded_at_utc TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS script_voice (
            block_id INTEGER PRIMARY KEY,
            name TEXT NOT NULL CHECK (length(name) > 0),
            clip_uri TEXT NOT NULL CHECK (length(clip_uri) > 0),
            chosen INTEGER NOT NULL CHECK (chosen >= 1),
            updated_at_utc TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS script_voice_takes (
            block_id INTEGER NOT NULL,
            take INTEGER NOT NULL CHECK (take >= 1),
            wav_uri TEXT NOT NULL CHECK (length(wav_uri) > 0),
            seconds REAL NOT NULL CHECK (seconds >= 0),
            tc TEXT NOT NULL,
            text TEXT NOT NULL,
            recorded_at_utc TEXT NOT NULL,
            heard_json TEXT,
            source_json TEXT,
            PRIMARY KEY (block_id, take));
        CREATE VIEW IF NOT EXISTS public_script_voice AS
            SELECT block_id, name, clip_uri, chosen, updated_at_utc FROM script_voice;",
    )
    .map_err(err)?;
    // Takes recorded before the words heard in them are kept: they get the column, empty.
    if !has_column_of(conn, "script_voice_takes", "heard_json")? {
        conn.execute_batch("ALTER TABLE script_voice_takes ADD COLUMN heard_json TEXT").map_err(err)?;
    }
    if !has_column_of(conn, "script_voice_takes", "source_json")? {
        conn.execute_batch("ALTER TABLE script_voice_takes ADD COLUMN source_json TEXT").map_err(err)?;
    }
    if view_exists(conn, "public_script_voice_takes")? && !has_column_of(conn, "public_script_voice_takes", "source_json")? {
        conn.execute_batch("DROP VIEW public_script_voice_takes").map_err(err)?;
    }
    conn.execute_batch(
        "CREATE VIEW IF NOT EXISTS public_script_voice_takes AS
            SELECT block_id, take, wav_uri, seconds, tc, text, recorded_at_utc, heard_json, source_json FROM script_voice_takes;",
    )
    .map_err(err)
}

fn has_column_of(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let sql: Option<String> = conn.query_row("SELECT sql FROM sqlite_master WHERE name = ?1", params![table], |r| r.get(0)).optional().map_err(err)?;
    Ok(sql.is_some_and(|sql| sql.contains(column)))
}

/// A removed block takes its voice record with it (its files stay: the project folder lets
/// none be deleted).
pub(crate) fn forget(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM script_voice_takes WHERE block_id = ?1", params![id]).map_err(err)?;
    conn.execute("DELETE FROM script_voice WHERE block_id = ?1", params![id]).map_err(err)?;
    Ok(())
}

impl Store {
    pub(crate) fn voices(&self) -> Result<Vec<Voice>> {
        let conn = self.connection();
        if !view_exists(conn, "public_script_voice")? {
            return Ok(Vec::new());
        }
        let mut voices: Vec<Voice> = {
            let mut statement = conn.prepare("SELECT block_id, name, clip_uri, chosen FROM public_script_voice ORDER BY block_id").map_err(err)?;
            let rows = statement
                .query_map([], |r| Ok(Voice { block_id: r.get(0)?, name: r.get(1)?, clip_uri: r.get(2)?, chosen: r.get::<_, i64>(3)?.clamp(1, u16::MAX as i64) as u16, takes: Vec::new() }))
                .map_err(err)?;
            rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)?
        };
        // A view made before the words heard reads without them until the owner brings it up.
        let heard = if has_column_of(conn, "public_script_voice_takes", "heard_json")? { "heard_json" } else { "NULL" };
        let source = if has_column_of(conn, "public_script_voice_takes", "source_json")? { "source_json" } else { "NULL" };
        let mut statement = conn.prepare(&format!("SELECT block_id, take, wav_uri, seconds, tc, text, {heard}, {source} FROM public_script_voice_takes ORDER BY block_id, take")).map_err(err)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, VoiceTake { take: r.get::<_, i64>(1)?.clamp(1, u16::MAX as i64) as u16, wav_uri: r.get(2)?, seconds: r.get(3)?, tc: r.get(4)?, text: r.get(5)?, heard: None, source: None }, r.get::<_, Option<String>>(6)?, r.get::<_, Option<String>>(7)?)))
            .map_err(err)?;
        for row in rows {
            let (block, mut take, heard, source) = row.map_err(err)?;
            take.heard = heard.map(|heard| serde_json::from_str(&heard).map_err(err)).transpose()?;
            take.source = source.map(|source| serde_json::from_str(&source).map_err(err)).transpose()?;
            if let Some(voice) = voices.iter_mut().find(|voice| voice.block_id == block) {
                voice.takes.push(take);
            }
        }
        Ok(voices)
    }

    /// Take `take` of block `id`: the next one (1 for the first), at most `MAX_TAKES`; the
    /// record keeps the name and clip given at its first take. The take becomes the one used,
    /// and the block keeps its clip, length and timecode.
    pub(crate) fn add_voice_take(&mut self, id: i64, name: &str, clip_uri: &str, take: &VoiceTake) -> Result<Data> {
        let kind = self.kind_spec(&self.kind_of_block(id)?)?;
        if !kind.recorded {
            return Err(qnc_ui_text::fill("db.script_not_recorded", &[("kind", &kind.label)]));
        }
        if name.trim().is_empty() || clip_uri.trim().is_empty() || take.wav_uri.trim().is_empty() || !(take.seconds.is_finite() && take.seconds > 0.0) {
            return Err(qnc_ui_text::text("db.script_voice_invalid").into());
        }
        let tx = qnc_db_broker::begin_write(self.connection_mut()).map_err(err)?;
        let kept: Option<(String, String)> = tx.query_row("SELECT name, clip_uri FROM script_voice WHERE block_id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))).optional().map_err(err)?;
        let last: i64 = tx.query_row("SELECT COALESCE(MAX(take), 0) FROM script_voice_takes WHERE block_id = ?1", params![id], |r| r.get(0)).map_err(err)?;
        if i64::from(take.take) != last + 1 {
            return Err(qnc_ui_text::fill("db.script_voice_take_order", &[("take", &take.take.to_string()), ("next", &(last + 1).to_string())]));
        }
        if take.take > MAX_TAKES {
            return Err(qnc_ui_text::fill("db.script_voice_full", &[("most", &MAX_TAKES.to_string())]));
        }
        if kept.as_ref().is_some_and(|(kept_name, _)| kept_name != name) {
            return Err(qnc_ui_text::text("db.script_voice_renamed").into());
        }
        // The words heard and where a found reading is go in with the take (a reading found in
        // another recording is heard already; the background need not hear it again).
        let heard = take.heard.as_ref().map(serde_json::to_string).transpose().map_err(err)?;
        let source = take.source.as_ref().map(serde_json::to_string).transpose().map_err(err)?;
        if take.source.as_ref().is_some_and(|source| source.clip_id.trim().is_empty() || source.to_frame <= source.from_frame || source.fps_num == 0 || source.fps_den == 0) {
            return Err(qnc_ui_text::text("db.script_voice_invalid").into());
        }
        tx.execute(
            "INSERT INTO script_voice_takes (block_id, take, wav_uri, seconds, tc, text, recorded_at_utc, heard_json, source_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?7, ?8)",
            params![id, take.take, take.wav_uri, take.seconds, take.tc, take.text, heard, source],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO script_voice (block_id, name, clip_uri, chosen, updated_at_utc) VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(block_id) DO UPDATE SET chosen = excluded.chosen, clip_uri = excluded.clip_uri, updated_at_utc = excluded.updated_at_utc",
            params![id, name, clip_uri, take.take],
        )
        .map_err(err)?;
        // The block uses this reading: its own clip, length and timecode.
        let clip_tc = &take.tc;
        tx.execute(
            "UPDATE script_blocks SET voice_uri = ?2, voice_seconds = ?3, voice_tc = ?4, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1",
            params![id, clip_uri, take.seconds, clip_tc],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// One recording of the whole story (no step of undo: a recording stays).
    pub(crate) fn add_session(&mut self, wav_uri: &str, seconds: f64, tc: &str, marks: &[f64]) -> Result<Data> {
        let marks = serde_json::to_string(marks).map_err(err)?;
        self.connection()
            .execute("INSERT INTO script_sessions (wav_uri, seconds, tc, marks_json, recorded_at_utc) VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))", params![wav_uri, seconds, tc, marks])
            .map_err(err)?;
        Ok(Data::Changed)
    }

    /// The words heard in take `take` of block `id` (no step of undo: the background made them).
    pub(crate) fn set_take_heard(&mut self, id: i64, take: u16, heard: &[HeardWord]) -> Result<Data> {
        let json = serde_json::to_string(heard).map_err(err)?;
        let changed = self.connection().execute("UPDATE script_voice_takes SET heard_json = ?3 WHERE block_id = ?1 AND take = ?2", params![id, take, json]).map_err(err)?;
        if changed == 0 {
            return Err(qnc_ui_text::fill("db.script_voice_no_take", &[("take", &take.to_string())]));
        }
        Ok(Data::Changed)
    }

    /// Take `take` of block `id` becomes the one used; the block keeps its length.
    pub(crate) fn set_voice_chosen(&mut self, id: i64, take: u16) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(self.connection_mut()).map_err(err)?;
        let (seconds, wav_uri, tc, source): (f64, String, String, Option<String>) = tx
            .query_row("SELECT seconds, wav_uri, tc, source_json FROM script_voice_takes WHERE block_id = ?1 AND take = ?2", params![id, take], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()
            .map_err(err)?
            .ok_or_else(|| qnc_ui_text::fill("db.script_voice_no_take", &[("take", &take.to_string())]))?;
        // A reading found in another recording plays from that clip's media.
        let clip_uri = if source.is_some() { wav_uri } else { take_clip_uri(&wav_uri) };
        tx.execute("UPDATE script_voice SET chosen = ?2, clip_uri = ?3, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, take, clip_uri]).map_err(err)?;
        tx.execute("UPDATE script_blocks SET voice_seconds = ?2, voice_uri = ?3, voice_tc = ?4, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, seconds, clip_uri, tc]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }
}

//! The voice records of the script's blocks (user 2026-10-30): one record per block of a
//! recorded kind, its takes in the order recorded, one of them used. The block keeps the clip,
//! length and timecode of the take used, as before, so every reader of a block sees it.

use crate::store::{err, view_exists, Store};
use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The most takes a record has (the channels of its clip).
pub const MAX_TAKES: u16 = 8;

pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS script_voice (
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
            PRIMARY KEY (block_id, take));
        CREATE VIEW IF NOT EXISTS public_script_voice_takes AS
            SELECT block_id, take, wav_uri, seconds, tc, text, recorded_at_utc FROM script_voice_takes;",
    )
    .map_err(err)?;
    // The part of the take used and its A1 (user 2026-10-31: an OFF is trimmed as a SubSegment
    // in MA Video); a record made before gets them as the whole take, as recorded.
    let columns: Vec<String> = conn
        .prepare("PRAGMA table_info(script_voice)")
        .and_then(|mut s| s.query_map([], |r| r.get::<_, String>(1))?.collect())
        .map_err(err)?;
    for (column, kind) in [("range_from", "INTEGER NOT NULL DEFAULT 0"), ("range_to", "INTEGER NOT NULL DEFAULT 0"), ("a1_level_db", "INTEGER NOT NULL DEFAULT 0"), ("a1_off", "INTEGER NOT NULL DEFAULT 0")] {
        if !columns.iter().any(|have| have == column) {
            conn.execute_batch(&format!("ALTER TABLE script_voice ADD COLUMN {column} {kind};")).map_err(err)?;
        }
    }
    conn.execute_batch(
        "DROP VIEW IF EXISTS public_script_voice;
        CREATE VIEW public_script_voice AS
            SELECT block_id, name, clip_uri, chosen, range_from, range_to, a1_level_db, a1_off, updated_at_utc FROM script_voice;",
    )
    .map_err(err)
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
            let mut statement = conn.prepare("SELECT block_id, name, clip_uri, chosen, range_from, range_to, a1_level_db, a1_off FROM public_script_voice ORDER BY block_id").map_err(err)?;
            let rows = statement
                .query_map([], |r| {
                    Ok(Voice {
                        block_id: r.get(0)?,
                        name: r.get(1)?,
                        clip_uri: r.get(2)?,
                        chosen: r.get::<_, i64>(3)?.clamp(1, u16::MAX as i64) as u16,
                        takes: Vec::new(),
                        from_frame: r.get::<_, i64>(4)?.max(0) as u64,
                        to_frame: r.get::<_, i64>(5)?.max(0) as u64,
                        a1_level_db: r.get::<_, i64>(6)?.clamp(-40, 12) as i8,
                        a1_off: r.get::<_, i64>(7)? != 0,
                    })
                })
                .map_err(err)?;
            rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)?
        };
        let mut statement = conn.prepare("SELECT block_id, take, wav_uri, seconds, tc, text FROM public_script_voice_takes ORDER BY block_id, take").map_err(err)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, VoiceTake { take: r.get::<_, i64>(1)?.clamp(1, u16::MAX as i64) as u16, wav_uri: r.get(2)?, seconds: r.get(3)?, tc: r.get(4)?, text: r.get(5)? })))
            .map_err(err)?;
        for row in rows {
            let (block, take) = row.map_err(err)?;
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
        if let Some((kept_name, kept_clip)) = &kept {
            if kept_name != name || kept_clip != clip_uri {
                return Err(qnc_ui_text::text("db.script_voice_renamed").into());
            }
        }
        tx.execute(
            "INSERT INTO script_voice_takes (block_id, take, wav_uri, seconds, tc, text, recorded_at_utc) VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![id, take.take, take.wav_uri, take.seconds, take.tc, take.text],
        )
        .map_err(err)?;
        tx.execute(
            "INSERT INTO script_voice (block_id, name, clip_uri, chosen, updated_at_utc) VALUES (?1, ?2, ?3, ?4, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(block_id) DO UPDATE SET chosen = excluded.chosen, range_from = 0, range_to = 0, updated_at_utc = excluded.updated_at_utc",
            params![id, name, clip_uri, take.take],
        )
        .map_err(err)?;
        // The clip starts where its first take started; its length used is the take's.
        let clip_tc: String = tx.query_row("SELECT tc FROM script_voice_takes WHERE block_id = ?1 AND take = 1", params![id], |r| r.get(0)).map_err(err)?;
        tx.execute(
            "UPDATE script_blocks SET voice_uri = ?2, voice_seconds = ?3, voice_tc = ?4, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1",
            params![id, clip_uri, take.seconds, clip_tc],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Take `take` of block `id` becomes the one used; the block keeps its length.
    pub(crate) fn set_voice_chosen(&mut self, id: i64, take: u16) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(self.connection_mut()).map_err(err)?;
        let seconds: f64 = tx
            .query_row("SELECT seconds FROM script_voice_takes WHERE block_id = ?1 AND take = ?2", params![id, take], |r| r.get(0))
            .optional()
            .map_err(err)?
            .ok_or_else(|| qnc_ui_text::fill("db.script_voice_no_take", &[("take", &take.to_string())]))?;
        // Another take is used whole (its trim belonged to the take before).
        tx.execute("UPDATE script_voice SET chosen = ?2, range_from = 0, range_to = 0, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, take]).map_err(err)?;
        tx.execute("UPDATE script_blocks SET voice_seconds = ?2, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, seconds]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }
}

impl Store {
    /// The part of the take used by block `id` and its A1 (user 2026-10-31: an OFF trimmed as a
    /// SubSegment in MA Video): clip frames `[from, to)` of at least one frame (the caller keeps it within the take);
    /// the level from -40 to +12 dB.
    pub(crate) fn set_voice_part(&mut self, id: i64, (from, to): (u64, u64), level_db: i8, off: bool) -> Result<Data> {
        if to <= from || !(-40..=12).contains(&level_db) {
            return Err(qnc_ui_text::text("db.script_voice_invalid").into());
        }
        let tx = qnc_db_broker::begin_write(self.connection_mut()).map_err(err)?;
        let changed = tx
            .execute(
                "UPDATE script_voice SET range_from = ?2, range_to = ?3, a1_level_db = ?4, a1_off = ?5, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1",
                params![id, from as i64, to as i64, i64::from(level_db), off],
            )
            .map_err(err)?;
        if changed == 0 {
            return Err(qnc_ui_text::fill("db.script_voice_no_take", &[("take", "1")]));
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }
}

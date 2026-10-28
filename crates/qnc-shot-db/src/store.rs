//! The shot table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 1] = ["clip_shots"];

pub(crate) struct Store {
    conn: Connection,
    ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection of a
    /// project without it reads no analysis.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            // A table of an earlier development form (without usable parts) is removed: its
            // clips are analysed again.
            let old: bool = tx
                .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='clip_shots') AND NOT EXISTS(SELECT 1 FROM pragma_table_info('clip_shots') WHERE name='parts_json')", [], |r| r.get(0))
                .map_err(err)?;
            if old {
                tx.execute_batch("DROP VIEW IF EXISTS public_clip_shots; DROP TABLE clip_shots;").map_err(err)?;
            }
            tx.execute_batch(
                "CREATE TABLE IF NOT EXISTS clip_shots (
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
                 CREATE VIEW IF NOT EXISTS public_clip_shots AS
                    SELECT clip_id, media_uri, pictures, shots_json, parts_json, version, error, took_ms, made_at_utc FROM clip_shots;",
            )
            .map_err(err)?;
            tx.commit().map_err(err)?;
        }
        let ready = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('public_clip_shots') WHERE name='parts_json')", [], |r| r.get(0))
            .map_err(err)?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::ReadClip { .. } if !self.ready => Ok(Data::Clip(None)),
            Operation::ReadAnalysed if !self.ready => Ok(Data::Analysed(Vec::new())),
            Operation::ReadClip { clip_id } => self.read_clip(clip_id),
            Operation::ReadAnalysed => self.read_analysed(),
            Operation::Save(shots) => self.save(shots),
        }
    }

    fn save(&mut self, shots: &ClipShots) -> Result<Data> {
        let valid = !shots.clip_id.is_empty()
            && shots.clip_id.len() <= 200
            && !shots.clip_id.chars().any(char::is_control)
            && shots.media_uri.starts_with("qnc://")
            && shots.shots.iter().all(|shot| shot.from < shot.to && shot.from <= shot.key_frame && shot.key_frame < shot.to)
            && shots.shots.windows(2).all(|pair| pair[0].to == pair[1].from)
            && shots.parts.iter().all(|part| part.from < part.to && part.from <= part.key_frame && part.key_frame <= part.to)
            && shots.parts.iter().all(|part| matches!(part.camera.as_str(), "steady" | "pan"))
            && (shots.error.is_empty() || shots.shots.is_empty() && shots.parts.is_empty());
        if !valid {
            return Err(qnc_ui_text::text("db.shot_invalid").into());
        }
        let json = serde_json::to_string(&shots.shots).map_err(err)?;
        let parts = serde_json::to_string(&shots.parts).map_err(err)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO clip_shots (clip_id, media_uri, pictures, shots_json, parts_json, version, error, took_ms, made_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?7, ?8, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(clip_id) DO UPDATE SET media_uri=excluded.media_uri, pictures=excluded.pictures,
               shots_json=excluded.shots_json, parts_json=excluded.parts_json, version=excluded.version, error=excluded.error, took_ms=excluded.took_ms, made_at_utc=excluded.made_at_utc",
            params![shots.clip_id, shots.media_uri, shots.pictures, json, shots.error, shots.took_ms as i64, parts, ANALYSIS_VERSION],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read_clip(&mut self, clip_id: &str) -> Result<Data> {
        let row = self
            .conn
            .query_row(
                "SELECT media_uri, pictures, shots_json, error, took_ms, parts_json FROM public_clip_shots WHERE clip_id = ?1 AND version = ?2",
                params![clip_id, ANALYSIS_VERSION],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?)),
            )
            .optional()
            .map_err(err)?;
        let Some((media_uri, pictures, json, error, took_ms, parts)) = row else {
            return Ok(Data::Clip(None));
        };
        let shots = serde_json::from_str(&json).map_err(err)?;
        let parts = serde_json::from_str(&parts).map_err(err)?;
        Ok(Data::Clip(Some(ClipShots { clip_id: clip_id.into(), media_uri, pictures, shots, parts, error, took_ms: took_ms.max(0) as u64 })))
    }

    fn read_analysed(&mut self) -> Result<Data> {
        let mut statement = self.conn.prepare("SELECT clip_id FROM public_clip_shots WHERE version = ?1 ORDER BY clip_id").map_err(err)?;
        let clips = statement.query_map(params![ANALYSIS_VERSION], |r| r.get(0)).map_err(err)?.collect::<std::result::Result<Vec<String>, _>>().map_err(err)?;
        Ok(Data::Analysed(clips))
    }

    fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn err(error: impl std::fmt::Display) -> String {
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

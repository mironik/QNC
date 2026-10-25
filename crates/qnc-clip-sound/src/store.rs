//! The clip sound table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["clip_sound"];
const MIN_LEVEL_DB: i8 = -40;
const MAX_LEVEL_DB: i8 = 12;

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection of a
    /// project without it reads no sound.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "view", "public_clip_sound")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::Read { .. } if !self.schema_ready => Ok(Data::Sound(None)),
            Operation::Read { clip_id } => self.read(clip_id),
            Operation::Set(sound) => self.set(sound),
        }
    }

    fn set(&mut self, sound: &ClipSound) -> Result<Data> {
        valid_clip(&sound.clip_id)?;
        for lane in [sound.a1, sound.a2] {
            if !(MIN_LEVEL_DB..=MAX_LEVEL_DB).contains(&lane.level_db) || lane.channel > 63 {
                return Err(qnc_ui_text::text("db.clip_sound_invalid").into());
            }
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO clip_sound (clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db, updated_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(clip_id) DO UPDATE SET a1_channel=excluded.a1_channel, a1_off=excluded.a1_off,
               a1_level_db=excluded.a1_level_db, a2_channel=excluded.a2_channel, a2_off=excluded.a2_off,
               a2_level_db=excluded.a2_level_db, updated_at_utc=excluded.updated_at_utc",
            params![
                sound.clip_id,
                sound.a1.channel,
                sound.a1.off,
                sound.a1.level_db,
                sound.a2.channel,
                sound.a2.off,
                sound.a2.level_db
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read(&mut self, clip_id: &str) -> Result<Data> {
        valid_clip(clip_id)?;
        let sound = self
            .conn
            .query_row(
                "SELECT clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db FROM public_clip_sound WHERE clip_id = ?1",
                params![clip_id],
                |row| {
                    let lane = |at: usize| -> rusqlite::Result<LaneSound> {
                        Ok(LaneSound { channel: row.get(at)?, off: row.get(at + 1)?, level_db: row.get(at + 2)? })
                    };
                    Ok(ClipSound { clip_id: row.get(0)?, a1: lane(1)?, a2: lane(4)? })
                },
            )
            .optional()
            .map_err(err)?;
        Ok(Data::Sound(sound))
    }

    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn valid_clip(clip_id: &str) -> Result<()> {
    if clip_id.is_empty() || clip_id.len() > 200 || clip_id.chars().any(char::is_control) {
        return Err(qnc_ui_text::text("db.clip_sound_invalid").into());
    }
    Ok(())
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "clip_sound")? {
        conn.execute_batch(
            "CREATE TABLE clip_sound (
                clip_id TEXT PRIMARY KEY,
                a1_channel INTEGER NOT NULL CHECK (a1_channel >= 0),
                a1_off INTEGER NOT NULL CHECK (a1_off IN (0, 1)),
                a1_level_db INTEGER NOT NULL,
                a2_channel INTEGER NOT NULL CHECK (a2_channel >= 0),
                a2_off INTEGER NOT NULL CHECK (a2_off IN (0, 1)),
                a2_level_db INTEGER NOT NULL,
                updated_at_utc TEXT NOT NULL
            );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_clip_sound")? {
        conn.execute_batch(
            "CREATE VIEW public_clip_sound AS
             SELECT clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db, updated_at_utc
             FROM clip_sound;",
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

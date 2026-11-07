//! The clip sound table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["clip_sound"];
const MIN_LEVEL_DB: i8 = -40;
const MAX_LEVEL_DB: i8 = 12;
/// The points of a curve: at most this many, each from -60 to +12 dB (tenths), as a story's.
const MAX_KEYS: usize = 256;
const MIN_KEY_DB10: i16 = -600;
const MAX_KEY_DB10: i16 = 120;

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
    /// The view has the level curves (a database read-only before its owner added them reads
    /// the sounds without curves).
    keys_ready: bool,
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
        let keys_ready = schema_ready && has_column(&conn, "public_clip_sound", "a1_keys")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready, keys_ready })
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
        for keys in [&sound.a1_keys, &sound.a2_keys] {
            if keys.len() > MAX_KEYS || keys.iter().any(|(_, db10)| !(MIN_KEY_DB10..=MAX_KEY_DB10).contains(db10)) || keys.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
                return Err(qnc_ui_text::text("db.clip_sound_invalid").into());
            }
        }
        let keys = |keys: &Vec<(u64, i16)>| serde_json::to_string(keys).map_err(err);
        let (a1_keys, a2_keys) = (keys(&sound.a1_keys)?, keys(&sound.a2_keys)?);
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO clip_sound (clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db, a1_keys, a2_keys, updated_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(clip_id) DO UPDATE SET a1_channel=excluded.a1_channel, a1_off=excluded.a1_off,
               a1_level_db=excluded.a1_level_db, a2_channel=excluded.a2_channel, a2_off=excluded.a2_off,
               a2_level_db=excluded.a2_level_db, a1_keys=excluded.a1_keys, a2_keys=excluded.a2_keys,
               updated_at_utc=excluded.updated_at_utc",
            params![
                sound.clip_id,
                sound.a1.channel,
                sound.a1.off,
                sound.a1.level_db,
                sound.a2.channel,
                sound.a2.off,
                sound.a2.level_db,
                a1_keys,
                a2_keys
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read(&mut self, clip_id: &str) -> Result<Data> {
        valid_clip(clip_id)?;
        let sql = if self.keys_ready {
            "SELECT clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db, a1_keys, a2_keys FROM public_clip_sound WHERE clip_id = ?1"
        } else {
            "SELECT clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db, '[]', '[]' FROM public_clip_sound WHERE clip_id = ?1"
        };
        let sound = self
            .conn
            .query_row(
                sql,
                params![clip_id],
                |row| {
                    let lane = |at: usize| -> rusqlite::Result<LaneSound> {
                        Ok(LaneSound { channel: row.get(at)?, off: row.get(at + 1)?, level_db: row.get(at + 2)? })
                    };
                    // A curve the database cannot read is none (it is the user's to draw again).
                    let keys = |at: usize| -> rusqlite::Result<Vec<(u64, i16)>> {
                        let text: String = row.get(at)?;
                        Ok(serde_json::from_str(&text).unwrap_or_default())
                    };
                    Ok(ClipSound { clip_id: row.get(0)?, a1: lane(1)?, a2: lane(4)?, a1_keys: keys(7)?, a2_keys: keys(8)? })
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
                updated_at_utc TEXT NOT NULL,
                a1_keys TEXT NOT NULL DEFAULT '[]',
                a2_keys TEXT NOT NULL DEFAULT '[]'
            );",
        )
        .map_err(err)?;
    }
    // A table made before the level curves (user 2026-11-07) gets their columns; its rows stay
    // (no curve), and its view is made again with them.
    if !has_column(conn, "clip_sound", "a1_keys")? {
        conn.execute_batch(
            "ALTER TABLE clip_sound ADD COLUMN a1_keys TEXT NOT NULL DEFAULT '[]';
             ALTER TABLE clip_sound ADD COLUMN a2_keys TEXT NOT NULL DEFAULT '[]';
             DROP VIEW IF EXISTS public_clip_sound;",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_clip_sound")? {
        conn.execute_batch(
            "CREATE VIEW public_clip_sound AS
             SELECT clip_id, a1_channel, a1_off, a1_level_db, a2_channel, a2_off, a2_level_db, updated_at_utc, a1_keys, a2_keys
             FROM clip_sound;",
        )
        .map_err(err)?;
    }
    Ok(())
}

fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info(?1) WHERE name = ?2)", params![table, column], |r| r.get(0)).map_err(err)
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

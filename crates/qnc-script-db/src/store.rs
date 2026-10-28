//! The script table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["script_blocks"];

pub(crate) struct Store {
    conn: Connection,
    ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection of a
    /// project without it reads an empty script.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let ready: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='view' AND name='public_script_blocks')", [], |r| r.get(0))
            .map_err(err)?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::Read if !self.ready => Ok(Data::Blocks(Vec::new())),
            Operation::Read => self.read().map(Data::Blocks),
            Operation::Add { after, block } => self.add(*after, block),
            Operation::SetText { id, text } => self.set_text(*id, text),
            Operation::Move { id, up } => self.move_block(*id, *up),
            Operation::Remove { id } => self.remove(*id),
        }
    }

    fn read(&self) -> Result<Vec<Block>> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT block_id, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds
                 FROM public_script_blocks ORDER BY position",
            )
            .map_err(err)?;
        let rows = statement
            .query_map([], |r| {
                Ok(Block {
                    id: r.get(0)?,
                    kind: if r.get::<_, String>(1)? == "sot" { BlockKind::Sot } else { BlockKind::Off },
                    text: r.get(2)?,
                    clip_id: r.get(3)?,
                    clip_name: r.get(4)?,
                    from_frame: r.get::<_, i64>(5)?.max(0) as u64,
                    to_frame: r.get::<_, i64>(6)?.max(0) as u64,
                    start_tc: r.get(7)?,
                    end_tc: r.get(8)?,
                    length: r.get(9)?,
                    seconds: r.get(10)?,
                })
            })
            .map_err(err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)
    }

    fn add(&mut self, after: Option<i64>, block: &Block) -> Result<Data> {
        let sot = block.kind == BlockKind::Sot;
        if sot && (block.clip_id.is_empty() || block.to_frame <= block.from_frame) {
            return Err(qnc_ui_text::text("db.script_sot_range").into());
        }
        if !sot && block.text.trim().is_empty() {
            return Err(qnc_ui_text::text("db.script_off_empty").into());
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let at: i64 = match after {
            Some(after) => {
                tx.query_row("SELECT position FROM script_blocks WHERE block_id = ?1", params![after], |r| r.get::<_, i64>(0))
                    .optional()
                    .map_err(err)?
                    .ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?
                    + 1
            }
            None => tx.query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM script_blocks", [], |r| r.get(0)).map_err(err)?,
        };
        tx.execute("UPDATE script_blocks SET position = position + 1 WHERE position >= ?1", params![at]).map_err(err)?;
        tx.execute(
            "INSERT INTO script_blocks (position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, updated_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![
                at,
                if sot { "sot" } else { "off" },
                block.text,
                block.clip_id,
                block.clip_name,
                block.from_frame as i64,
                block.to_frame as i64,
                block.start_tc,
                block.end_tc,
                block.length,
                block.seconds
            ],
        )
        .map_err(err)?;
        let id = tx.last_insert_rowid();
        renumber(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Added(id))
    }

    fn set_text(&mut self, id: i64, text: &str) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let kind: Option<String> = tx.query_row("SELECT kind FROM script_blocks WHERE block_id = ?1", params![id], |r| r.get(0)).optional().map_err(err)?;
        match kind.as_deref() {
            None => return Err(qnc_ui_text::text("db.script_no_block").into()),
            Some("off") if text.trim().is_empty() => return Err(qnc_ui_text::text("db.script_off_empty").into()),
            _ => {}
        }
        tx.execute("UPDATE script_blocks SET text = ?2, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, text]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn move_block(&mut self, id: i64, up: bool) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let at: i64 = tx
            .query_row("SELECT position FROM script_blocks WHERE block_id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(err)?
            .ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let other = if up { at - 1 } else { at + 1 };
        let neighbour: Option<i64> = tx.query_row("SELECT block_id FROM script_blocks WHERE position = ?1", params![other], |r| r.get(0)).optional().map_err(err)?;
        if let Some(neighbour) = neighbour {
            tx.execute("UPDATE script_blocks SET position = ?2 WHERE block_id = ?1", params![neighbour, at]).map_err(err)?;
            tx.execute("UPDATE script_blocks SET position = ?2 WHERE block_id = ?1", params![id, other]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn remove(&mut self, id: i64) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute("DELETE FROM script_blocks WHERE block_id = ?1", params![id]).map_err(err)?;
        renumber(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

/// Positions 0, 1, 2... in reading order.
fn renumber(conn: &Connection) -> Result<()> {
    let ids: Vec<i64> = {
        let mut statement = conn.prepare("SELECT block_id FROM script_blocks ORDER BY position, block_id").map_err(err)?;
        let rows = statement.query_map([], |r| r.get(0)).map_err(err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)?
    };
    for (index, id) in ids.iter().enumerate() {
        conn.execute("UPDATE script_blocks SET position = ?2 WHERE block_id = ?1", params![id, index as i64]).map_err(err)?;
    }
    Ok(())
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS script_blocks (
            block_id INTEGER PRIMARY KEY AUTOINCREMENT,
            position INTEGER NOT NULL,
            kind TEXT NOT NULL CHECK (kind IN ('off', 'sot')),
            text TEXT NOT NULL,
            clip_id TEXT NOT NULL,
            clip_name TEXT NOT NULL,
            from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
            to_frame INTEGER NOT NULL CHECK (to_frame >= 0),
            start_tc TEXT NOT NULL,
            end_tc TEXT NOT NULL,
            length TEXT NOT NULL,
            seconds REAL NOT NULL CHECK (seconds >= 0),
            updated_at_utc TEXT NOT NULL
        );
        CREATE VIEW IF NOT EXISTS public_script_blocks AS
            SELECT block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, updated_at_utc
            FROM script_blocks;",
    )
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

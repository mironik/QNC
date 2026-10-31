//! The skeleton table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["skeleton_elements"];
const MIN_LEVEL_DB: i8 = -40;
const MAX_LEVEL_DB: i8 = 12;

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection of a
    /// project without it reads an empty skeleton.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "view", "public_skeleton_elements")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::Read if !self.schema_ready => Ok(Data::Elements(Vec::new())),
            Operation::Read => self.read(),
            Operation::Append { elements } => self.append(elements),
            Operation::Update { element_id, in_frame, out_frame, a1, gap_before, cut } => self.update(*element_id, (*in_frame, *out_frame), *a1, (*gap_before, *cut)),
            Operation::Move { element_id, earlier } => self.relocate(*element_id, *earlier),
            Operation::Remove { element_id } => self.remove(*element_id),
        }
    }

    fn update(&mut self, element_id: i64, (in_frame, out_frame): (u64, u64), a1: Lane, (gap, cut): (u64, bool)) -> Result<Data> {
        if out_frame <= in_frame || !(MIN_LEVEL_DB..=MAX_LEVEL_DB).contains(&a1.level_db) || a1.channel > 63 {
            return Err(qnc_ui_text::text("db.skeleton_invalid").into());
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let changed = tx
            .execute(
                "UPDATE skeleton_elements SET in_frame = ?2, out_frame = ?3, a1_channel = ?4, a1_level_db = ?5, a1_off = ?6,
                   gap_before = ?7, cut = ?8 WHERE element_id = ?1",
                params![element_id, in_frame as i64, out_frame as i64, a1.channel, a1.level_db, a1.off, gap as i64, cut],
            )
            .map_err(err)?;
        if changed != 1 {
            return Err(qnc_ui_text::text("db.skeleton_invalid").into());
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Swaps the place of an element with its neighbour; at the end nothing moves.
    fn relocate(&mut self, element_id: i64, earlier: bool) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let at: i64 = tx.query_row("SELECT sort_index FROM skeleton_elements WHERE element_id = ?1", params![element_id], |row| row.get(0)).map_err(err)?;
        let neighbour = if earlier {
            "SELECT element_id, sort_index FROM skeleton_elements WHERE sort_index < ?1 ORDER BY sort_index DESC LIMIT 1"
        } else {
            "SELECT element_id, sort_index FROM skeleton_elements WHERE sort_index > ?1 ORDER BY sort_index LIMIT 1"
        };
        let found: Option<(i64, i64)> = tx.query_row(neighbour, params![at], |row| Ok((row.get(0)?, row.get(1)?))).ok();
        if let Some((other, other_at)) = found {
            // Through a free place: the sort index is unique.
            tx.execute("UPDATE skeleton_elements SET sort_index = -1 WHERE element_id = ?1", params![element_id]).map_err(err)?;
            tx.execute("UPDATE skeleton_elements SET sort_index = ?2 WHERE element_id = ?1", params![other, at]).map_err(err)?;
            tx.execute("UPDATE skeleton_elements SET sort_index = ?2 WHERE element_id = ?1", params![element_id, other_at]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn remove(&mut self, element_id: i64) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute("DELETE FROM skeleton_elements WHERE element_id = ?1", params![element_id]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn append(&mut self, elements: &[NewElement]) -> Result<Data> {
        for element in elements {
            valid(element)?;
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let mut next: i64 = tx.query_row("SELECT COALESCE(MAX(sort_index), -1) + 1 FROM skeleton_elements", [], |row| row.get(0)).map_err(err)?;
        for element in elements {
            tx.execute(
                "INSERT INTO skeleton_elements (sort_index, kind, clip_id, in_frame, out_frame,
                   a1_channel, a1_level_db, a1_off, a2_channel, a2_level_db, a2_off,
                   a1_lead_frames, a1_tail_frames, gap_before, cut, origin, created_at_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, 0, 0, 0, ?12, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                params![
                    next,
                    element.kind.as_str(),
                    element.clip_id,
                    element.in_frame as i64,
                    element.out_frame as i64,
                    element.a1.channel,
                    element.a1.level_db,
                    element.a1.off,
                    element.a2.channel,
                    element.a2.level_db,
                    element.a2.off,
                    element.origin
                ],
            )
            .map_err(err)?;
            next += 1;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Appended(elements.len()))
    }

    fn read(&mut self) -> Result<Data> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT element_id, sort_index, kind, clip_id, in_frame, out_frame,
                   a1_channel, a1_level_db, a1_off, a2_channel, a2_level_db, a2_off,
                   a1_lead_frames, a1_tail_frames, gap_before, cut, origin
                 FROM public_skeleton_elements ORDER BY sort_index",
            )
            .map_err(err)?;
        let elements = statement
            .query_map([], |row| {
                let lane = |at: usize| -> rusqlite::Result<Lane> {
                    Ok(Lane { channel: row.get(at)?, level_db: row.get(at + 1)?, off: row.get(at + 2)? })
                };
                let kind: String = row.get(2)?;
                Ok(Element {
                    element_id: row.get(0)?,
                    sort_index: row.get(1)?,
                    kind: if kind == "off" { ElementKind::Off } else { ElementKind::Ton },
                    clip_id: row.get(3)?,
                    in_frame: row.get::<_, i64>(4)? as u64,
                    out_frame: row.get::<_, i64>(5)? as u64,
                    a1: lane(6)?,
                    a2: lane(9)?,
                    a1_lead_frames: row.get::<_, i64>(12)? as u64,
                    a1_tail_frames: row.get::<_, i64>(13)? as u64,
                    gap_before: row.get::<_, i64>(14)? as u64,
                    cut: row.get(15)?,
                    origin: row.get(16)?,
                })
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Elements(elements))
    }

    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn valid(element: &NewElement) -> Result<()> {
    let clip = &element.clip_id;
    let lanes_ok = [element.a1, element.a2]
        .iter()
        .all(|lane| (MIN_LEVEL_DB..=MAX_LEVEL_DB).contains(&lane.level_db) && lane.channel <= 63);
    if clip.is_empty() || clip.len() > 200 || clip.chars().any(char::is_control) || element.out_frame <= element.in_frame || !lanes_ok {
        return Err(qnc_ui_text::text("db.skeleton_invalid").into());
    }
    Ok(())
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    if !object_exists(conn, "table", "skeleton_elements")? {
        conn.execute_batch(
            "CREATE TABLE skeleton_elements (
                element_id INTEGER PRIMARY KEY AUTOINCREMENT,
                sort_index INTEGER NOT NULL UNIQUE,
                kind TEXT NOT NULL CHECK (kind IN ('ton', 'off')),
                clip_id TEXT NOT NULL,
                in_frame INTEGER NOT NULL CHECK (in_frame >= 0),
                out_frame INTEGER NOT NULL CHECK (out_frame > in_frame),
                a1_channel INTEGER NOT NULL CHECK (a1_channel >= 0),
                a1_level_db INTEGER NOT NULL,
                a1_off INTEGER NOT NULL CHECK (a1_off IN (0, 1)),
                a2_channel INTEGER NOT NULL CHECK (a2_channel >= 0),
                a2_level_db INTEGER NOT NULL,
                a2_off INTEGER NOT NULL CHECK (a2_off IN (0, 1)),
                a1_lead_frames INTEGER NOT NULL CHECK (a1_lead_frames >= 0),
                a1_tail_frames INTEGER NOT NULL CHECK (a1_tail_frames >= 0),
                gap_before INTEGER NOT NULL CHECK (gap_before >= 0),
                cut INTEGER NOT NULL CHECK (cut IN (0, 1)),
                origin TEXT NOT NULL,
                created_at_utc TEXT NOT NULL
            );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_skeleton_elements")? {
        conn.execute_batch(
            "CREATE VIEW public_skeleton_elements AS
             SELECT element_id, sort_index, kind, clip_id, in_frame, out_frame,
               a1_channel, a1_level_db, a1_off, a2_channel, a2_level_db, a2_off,
               a1_lead_frames, a1_tail_frames, gap_before, cut, origin, created_at_utc
             FROM skeleton_elements;",
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

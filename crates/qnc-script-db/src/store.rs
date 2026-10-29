//! The script tables of a project database and their requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 2] = ["script_blocks", "script_profile"];

pub(crate) struct Store {
    conn: Connection,
    ready: bool,
    /// A table made on the first day of the script (read only until the owner brings it up):
    /// its blocks are read without recordings.
    legacy: bool,
    /// A table made before block fields: its blocks are read without them.
    no_fields: bool,
    /// A table made before blocks of several parts: its blocks are read as one range each.
    no_pieces: bool,
}

impl Store {
    /// Creates the tables if missing (read-write) and serves them; a read-only connection of a
    /// project without them reads an empty script and no profile.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let ready = view_exists(&conn, "public_script_blocks")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        let legacy = ready && !has_voice(&conn)?;
        let no_fields = ready && !has_column(&conn, "fields_json")?;
        let no_pieces = ready && !has_column(&conn, "pieces_json")?;
        Ok(Self { conn, ready, legacy, no_fields, no_pieces })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::Read if !self.ready => Ok(Data::Blocks(Vec::new())),
            Operation::Read => self.read().map(Data::Blocks),
            Operation::ReadProfile => profile(&self.conn).map(Data::Profile),
            Operation::SetProfile { profile } => self.set_profile(profile),
            Operation::Add { after, first, block } => self.add(*after, *first, block),
            Operation::SetText { id, text } => self.set_text(*id, text),
            Operation::Move { id, up } => self.move_block(*id, *up),
            Operation::Remove { id } => self.remove(*id),
            Operation::SetVoice { id, uri, seconds, tc } => self.set_voice(*id, uri, *seconds, tc),
            Operation::SetFields { id, fields } => self.set_fields(*id, fields),
            Operation::AddPiece { id, piece } => self.add_piece(*id, piece),
            Operation::SetJoin { id, index, gap_before, cut } => self.set_join(*id, *index, *gap_before, *cut),
            Operation::SetPiece { id, index, piece } => self.set_piece(*id, *index, piece),
            Operation::MovePiece { id, index, earlier } => self.move_piece(*id, *index, *earlier),
            Operation::RemovePiece { id, index } => self.remove_piece(*id, *index),
        }
    }

    fn read(&self) -> Result<Vec<Block>> {
        let voice = if self.legacy { "'', 0.0, ''" } else { "voice_uri, voice_seconds, voice_tc" };
        let fields = if self.no_fields { "'{}'" } else { "fields_json" };
        let pieces = if self.no_pieces { "'[]'" } else { "pieces_json" };
        let mut statement = self
            .conn
            .prepare(&format!(
                "SELECT block_id, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, {voice}, {fields}, {pieces}
                 FROM public_script_blocks ORDER BY position"
            ))
            .map_err(err)?;
        let rows = statement
            .query_map([], |r| {
                Ok(Block {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    text: r.get(2)?,
                    clip_id: r.get(3)?,
                    clip_name: r.get(4)?,
                    from_frame: r.get::<_, i64>(5)?.max(0) as u64,
                    to_frame: r.get::<_, i64>(6)?.max(0) as u64,
                    start_tc: r.get(7)?,
                    end_tc: r.get(8)?,
                    length: r.get(9)?,
                    seconds: r.get(10)?,
                    voice_uri: r.get(11)?,
                    voice_seconds: r.get(12)?,
                    voice_tc: r.get(13)?,
                    fields: serde_json::from_str(&r.get::<_, String>(14)?).unwrap_or_default(),
                    pieces: serde_json::from_str(&r.get::<_, String>(15)?).unwrap_or_default(),
                })
            })
            .map_err(err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)
    }

    /// The project's script is written with `profile`; every kind its blocks have must be one
    /// of it.
    fn set_profile(&mut self, new: &ScriptProfile) -> Result<Data> {
        new.check()?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let used: Vec<String> = {
            let mut statement = tx.prepare("SELECT DISTINCT kind FROM script_blocks ORDER BY kind").map_err(err)?;
            let rows = statement.query_map([], |r| r.get(0)).map_err(err)?;
            rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)?
        };
        if let Some(missing) = used.iter().find(|kind| new.kind(kind).is_none()) {
            return Err(qnc_ui_text::fill("db.script_profile_lacks", &[("kind", missing), ("profile", &new.name)]));
        }
        let json = serde_json::to_string(new).map_err(err)?;
        tx.execute(
            "INSERT INTO script_profile (singleton, profile_id, profile_json, updated_at_utc) VALUES (1, ?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(singleton) DO UPDATE SET profile_id = excluded.profile_id, profile_json = excluded.profile_json, updated_at_utc = excluded.updated_at_utc",
            params![new.profile_id, json],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// The kind `code` of the project's profile (a write needs both).
    fn kind_spec(&self, code: &str) -> Result<KindSpec> {
        let profile = profile(&self.conn)?.ok_or_else(|| qnc_ui_text::text("db.script_no_profile").to_string())?;
        profile.kind(code).cloned().ok_or_else(|| qnc_ui_text::fill("db.script_unknown_kind", &[("kind", code), ("profile", &profile.name)]))
    }

    fn add(&mut self, after: Option<i64>, first: bool, block: &Block) -> Result<Data> {
        let kind = self.kind_spec(&block.kind)?;
        if kind.role == Role::Clip && (block.clip_id.is_empty() || block.to_frame <= block.from_frame) {
            return Err(qnc_ui_text::fill("db.script_clip_range", &[("kind", &kind.label)]));
        }
        if kind.role != Role::Clip && block.text.trim().is_empty() {
            return Err(qnc_ui_text::fill("db.script_text_empty", &[("kind", &kind.label)]));
        }
        if !block.pieces.is_empty() && (kind.role != Role::Clip || block.pieces.iter().any(|piece| piece.clip_id.is_empty() || piece.to_frame <= piece.from_frame)) {
            return Err(qnc_ui_text::fill("db.script_clip_range", &[("kind", &kind.label)]));
        }
        let fields = self.fields_json(&kind, &block.fields)?;
        let pieces = serde_json::to_string(&block.pieces).map_err(err)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let at: i64 = match after {
            Some(after) => {
                tx.query_row("SELECT position FROM script_blocks WHERE block_id = ?1", params![after], |r| r.get::<_, i64>(0))
                    .optional()
                    .map_err(err)?
                    .ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?
                    + 1
            }
            None if first => 0,
            None => tx.query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM script_blocks", [], |r| r.get(0)).map_err(err)?,
        };
        tx.execute("UPDATE script_blocks SET position = position + 1 WHERE position >= ?1", params![at]).map_err(err)?;
        tx.execute(
            "INSERT INTO script_blocks (position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, fields_json, pieces_json, updated_at_utc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![
                at,
                block.kind,
                block.text,
                block.clip_id,
                block.clip_name,
                block.from_frame as i64,
                block.to_frame as i64,
                block.start_tc,
                block.end_tc,
                block.length,
                block.seconds,
                fields,
                pieces
            ],
        )
        .map_err(err)?;
        let id = tx.last_insert_rowid();
        renumber(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Added(id))
    }

    /// The kind code of block `id`.
    fn kind_of_block(&self, id: i64) -> Result<String> {
        self.conn
            .query_row("SELECT kind FROM script_blocks WHERE block_id = ?1", params![id], |r| r.get(0))
            .optional()
            .map_err(err)?
            .ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())
    }

    fn set_text(&mut self, id: i64, text: &str) -> Result<Data> {
        let kind = self.kind_spec(&self.kind_of_block(id)?)?;
        if kind.role != Role::Clip && text.trim().is_empty() {
            return Err(qnc_ui_text::fill("db.script_text_empty", &[("kind", &kind.label)]));
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
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

    fn set_voice(&mut self, id: i64, uri: &str, seconds: f64, tc: &str) -> Result<Data> {
        let kind = self.kind_spec(&self.kind_of_block(id)?)?;
        if !kind.recorded {
            return Err(qnc_ui_text::fill("db.script_not_recorded", &[("kind", &kind.label)]));
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        let seconds = if uri.is_empty() { 0.0 } else { seconds.max(0.0) };
        tx.execute(
            "UPDATE script_blocks SET voice_uri = ?2, voice_seconds = ?3, voice_tc = ?4, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1",
            params![id, uri, seconds, tc],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// The fields of `kind` as the table keeps them: only fields the kind has, a choice only one
    /// of its choices, an empty value left out.
    fn fields_json(&self, kind: &KindSpec, fields: &std::collections::BTreeMap<String, String>) -> Result<String> {
        let options = profile(&self.conn)?.map(|profile| profile.options).unwrap_or_default();
        let mut kept = std::collections::BTreeMap::new();
        for (code, value) in fields {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            let Some(spec) = kind.fields.iter().find(|field| field.path == *code) else {
                return Err(qnc_ui_text::fill("db.script_unknown_field", &[("field", code), ("kind", &kind.label)]));
            };
            if let Some(set) = &spec.options {
                if spec.kind == qnc_field_spec::FieldKind::Choice && !options.get(set).is_some_and(|choices| choices.iter().any(|(id, _)| id == value)) {
                    return Err(qnc_ui_text::fill("db.script_bad_choice", &[("field", &spec.label), ("value", value)]));
                }
            }
            kept.insert(code.clone(), value.to_string());
        }
        serde_json::to_string(&kept).map_err(err)
    }

    fn set_fields(&mut self, id: i64, fields: &std::collections::BTreeMap<String, String>) -> Result<Data> {
        let kind = self.kind_spec(&self.kind_of_block(id)?)?;
        let json = self.fields_json(&kind, fields)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute("UPDATE script_blocks SET fields_json = ?2, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, json]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// Clip block `id` gets `piece` as its next part; its range, length and words follow its parts.
    fn add_piece(&mut self, id: i64, piece: &Piece) -> Result<Data> {
        let kind = self.kind_spec(&self.kind_of_block(id)?)?;
        if kind.role != Role::Clip || piece.clip_id.is_empty() || piece.to_frame <= piece.from_frame {
            return Err(qnc_ui_text::fill("db.script_clip_range", &[("kind", &kind.label)]));
        }
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        if block.pieces.is_empty() {
            block.pieces.push(Piece::of(&block));
        }
        block.pieces.push(Piece { gap_before: 0, cut: false, ..piece.clone() });
        self.store_pieces(id, block)
    }

    /// The pause before part `index` and its join; the first part has neither.
    fn set_join(&mut self, id: i64, index: usize, gap_before: u64, cut: bool) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let Some(piece) = block.pieces.get_mut(index).filter(|_| index > 0) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        (piece.gap_before, piece.cut) = (gap_before, cut);
        self.store_pieces(id, block)
    }

    /// Part `index` with its new range (an edge trimmed on the SubSegment timeline): the same
    /// clip, a range of at least one frame; its pause and join stay.
    fn set_piece(&mut self, id: i64, index: usize, piece: &Piece) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let Some(stored) = block.pieces.get_mut(index) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        if piece.clip_id != stored.clip_id || piece.to_frame <= piece.from_frame {
            return Err(qnc_ui_text::fill("db.script_clip_range", &[("kind", &block.kind)]));
        }
        *stored = Piece { gap_before: stored.gap_before, cut: stored.cut, ..piece.clone() };
        self.store_pieces(id, block)
    }

    /// Part `index` changes places with its neighbour (user 2026-10-29); the pause and join
    /// before each place stay with the place, the first place has none.
    fn move_piece(&mut self, id: i64, index: usize, earlier: bool) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let other = if earlier { index.checked_sub(1) } else { index.checked_add(1) };
        let Some(other) = other.filter(|other| index < block.pieces.len() && *other < block.pieces.len()) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        let joins: Vec<(u64, bool)> = block.pieces.iter().map(|piece| (piece.gap_before, piece.cut)).collect();
        block.pieces.swap(index, other);
        for (piece, (gap, cut)) in block.pieces.iter_mut().zip(joins) {
            (piece.gap_before, piece.cut) = (gap, cut);
        }
        self.store_pieces(id, block)
    }

    /// Part `index` leaves its block (user 2026-10-29); the part that becomes first has no
    /// pause or join before it. A block's last part does not leave: the block is removed.
    fn remove_piece(&mut self, id: i64, index: usize) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        if index >= block.pieces.len() {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        }
        if block.pieces.len() < 2 {
            return Err(qnc_ui_text::text("db.script_last_piece").into());
        }
        block.pieces.remove(index);
        if let Some(first) = block.pieces.first_mut() {
            (first.gap_before, first.cut) = (0, false);
        }
        self.store_pieces(id, block)
    }

    /// The block made from its parts again, written.
    fn store_pieces(&mut self, id: i64, mut block: Block) -> Result<Data> {
        block.from_pieces();
        let pieces = serde_json::to_string(&block.pieces).map_err(err)?;
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "UPDATE script_blocks SET text = ?2, clip_id = ?3, clip_name = ?4, from_frame = ?5, to_frame = ?6, start_tc = ?7, end_tc = ?8, length = ?9, seconds = ?10, pieces_json = ?11,
                 updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1",
            params![id, block.text, block.clip_id, block.clip_name, block.from_frame as i64, block.to_frame as i64, block.start_tc, block.end_tc, block.length, block.seconds, pieces],
        )
        .map_err(err)?;
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

/// The profile the project keeps (none in a project without the table or the row).
fn profile(conn: &Connection) -> Result<Option<ScriptProfile>> {
    if !view_exists(conn, "public_script_profile")? {
        return Ok(None);
    }
    let json: Option<String> = conn.query_row("SELECT profile_json FROM public_script_profile", [], |r| r.get(0)).optional().map_err(err)?;
    json.map(|json| serde_json::from_str(&json).map_err(err)).transpose()
}

fn view_exists(conn: &Connection, name: &str) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='view' AND name=?1)", params![name], |r| r.get(0)).map_err(err)
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

/// The columns of a script block as the table keeps them (in this order). The kind is a code
/// of the project's profile, so the table names no kinds.
const COLUMNS: &str = "block_id INTEGER PRIMARY KEY AUTOINCREMENT,
            position INTEGER NOT NULL,
            kind TEXT NOT NULL CHECK (length(kind) > 0),
            text TEXT NOT NULL,
            clip_id TEXT NOT NULL,
            clip_name TEXT NOT NULL,
            from_frame INTEGER NOT NULL CHECK (from_frame >= 0),
            to_frame INTEGER NOT NULL CHECK (to_frame >= 0),
            start_tc TEXT NOT NULL,
            end_tc TEXT NOT NULL,
            length TEXT NOT NULL,
            seconds REAL NOT NULL CHECK (seconds >= 0),
            voice_uri TEXT NOT NULL DEFAULT '',
            voice_seconds REAL NOT NULL DEFAULT 0 CHECK (voice_seconds >= 0),
            voice_tc TEXT NOT NULL DEFAULT '',
            fields_json TEXT NOT NULL DEFAULT '{}',
            pieces_json TEXT NOT NULL DEFAULT '[]',
            updated_at_utc TEXT NOT NULL";

fn table_sql(conn: &Connection) -> Result<Option<String>> {
    conn.query_row("SELECT sql FROM sqlite_master WHERE type='table' AND name='script_blocks'", [], |r| r.get(0)).optional().map_err(err)
}

/// Whether the table has column `name` (a table made before block fields has no `fields_json`).
fn has_column(conn: &Connection, name: &str) -> Result<bool> {
    Ok(table_sql(conn)?.is_none_or(|sql| sql.contains(name)))
}

/// Whether the table keeps recordings (one made on the first day of the script did not).
fn has_voice(conn: &Connection) -> Result<bool> {
    Ok(table_sql(conn)?.is_none_or(|sql| sql.contains("voice_uri")))
}

/// Whether the table is as this module makes it (the ones of the days before profiles named
/// their kinds).
fn current(conn: &Connection) -> Result<bool> {
    Ok(table_sql(conn)?.is_none_or(|sql| sql.contains("voice_uri") && sql.contains("fields_json") && sql.contains("pieces_json") && !sql.contains("kind IN")))
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    if !current(conn)? {
        // The blocks written before stay, with their recordings when the table had them.
        let voice = if has_voice(conn)? { ", voice_uri, voice_seconds, voice_tc" } else { "" };
        let voice = if has_column(conn, "fields_json")? { format!("{voice}, fields_json") } else { voice.to_string() };
        let voice = if has_column(conn, "pieces_json")? { format!("{voice}, pieces_json") } else { voice };
        conn.execute_batch(&format!(
            "DROP VIEW IF EXISTS public_script_blocks;
             CREATE TABLE script_blocks_next ({COLUMNS});
             INSERT INTO script_blocks_next (block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, updated_at_utc{voice})
                 SELECT block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, updated_at_utc{voice} FROM script_blocks;
             DROP TABLE script_blocks;
             ALTER TABLE script_blocks_next RENAME TO script_blocks;"
        ))
        .map_err(err)?;
    }
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS script_blocks ({COLUMNS});
        CREATE VIEW IF NOT EXISTS public_script_blocks AS
            SELECT block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, voice_uri, voice_seconds, voice_tc, fields_json, pieces_json, updated_at_utc
            FROM script_blocks;
        CREATE TABLE IF NOT EXISTS script_profile (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            profile_id TEXT NOT NULL,
            profile_json TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL);
        CREATE VIEW IF NOT EXISTS public_script_profile AS
            SELECT profile_id, profile_json, updated_at_utc FROM script_profile;"
    ))
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

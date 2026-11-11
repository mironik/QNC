//! The script tables of a project database and their requests.

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 6] = ["script_blocks", "script_profile", "script_voice", "script_voice_takes", "script_undo", "script_sessions"];

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
    /// A table made before M markers in the text: its blocks are read without them.
    no_marks: bool,
    /// A table made before the B-roll of a text block: its blocks are read without it.
    no_broll: bool,
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
        let no_marks = ready && !has_column(&conn, "marks_json")?;
        let no_broll = ready && !has_column(&conn, "covers_json")?;
        Ok(Self { conn, ready, legacy, no_fields, no_pieces, no_marks, no_broll })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::Undo | Operation::Redo if !self.ready => Ok(Data::Blocks(Vec::new())),
            Operation::Undo | Operation::Redo => self.in_one(|store| {
                let done = crate::undo::step(&store.conn, matches!(operation, Operation::Undo))?;
                Ok(if done { Data::Changed } else { Data::Blocks(Vec::new()) })
            }),
            // A change of the blocks is one step of undo, kept in the same transaction.
            operation if self.ready && undoable(operation) => self.in_one(|store| {
                let before = crate::undo::snapshot(&store.conn)?;
                let done = store.change(operation)?;
                if crate::undo::snapshot(&store.conn)? != before {
                    crate::undo::changed(&store.conn, &before)?;
                }
                Ok(done)
            }),
            operation => self.change(operation),
        }
    }

    /// `work` in one transaction: its own, or a savepoint of the one the broker opened.
    fn in_one(&mut self, work: impl FnOnce(&mut Self) -> Result<Data>) -> Result<Data> {
        let own = self.conn.is_autocommit();
        self.conn.execute_batch(if own { "BEGIN IMMEDIATE" } else { "SAVEPOINT script_undo_step" }).map_err(err)?;
        match work(self) {
            Ok(data) => {
                self.conn.execute_batch(if own { "COMMIT" } else { "RELEASE script_undo_step" }).map_err(err)?;
                Ok(data)
            }
            Err(error) => {
                let _ = self.conn.execute_batch(if own { "ROLLBACK" } else { "ROLLBACK TO script_undo_step; RELEASE script_undo_step" });
                Err(error)
            }
        }
    }

    fn change(&mut self, operation: &Operation) -> Result<Data> {
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
            Operation::SetSmooth { id, index, frames } => self.set_smooth(*id, *index, *frames),
            Operation::SetSmoothClip { id, index, clip_id } => self.set_smooth_clip(*id, *index, clip_id),
            Operation::SetPiece { id, index, piece } => self.set_piece(*id, *index, piece),
            Operation::SetJoinSound { id, index, lead, tail } => self.set_join_sound(*id, *index, *lead, *tail),
            Operation::SetJoinPicture { id, index, lead, tail } => self.set_join_picture(*id, *index, *lead, *tail),
            Operation::MovePiece { id, index, earlier } => self.move_piece(*id, *index, *earlier),
            Operation::RemovePiece { id, index } => self.remove_piece(*id, *index),
            Operation::CutOut { id, index, before, after } => self.cut_out(*id, *index, before.as_ref(), after.as_ref()),
            Operation::KeepOf { id, index, kept } => self.keep_of(*id, *index, kept),
            // Inside the one transaction `execute` opened: a refused change rolls all back.
            Operation::Together { changes } => {
                if !self.ready || changes.iter().any(|change| !part_change(change)) {
                    return Err(qnc_ui_text::text("db.script_reply").into());
                }
                for change in changes {
                    self.change(change)?;
                }
                Ok(Data::Changed)
            }
            Operation::Undo | Operation::Redo => Ok(Data::Blocks(Vec::new())),
            Operation::ReadVoices => self.voices().map(Data::Voices),
            Operation::AddVoiceTake { id, name, clip_uri, take } => self.add_voice_take(*id, name, clip_uri, take),
            Operation::AddSession { wav_uri, seconds, tc, marks } => self.add_session(wav_uri, *seconds, tc, marks),
            Operation::SetVoiceChosen { id, take } => self.set_voice_chosen(*id, *take),
            Operation::SetTakeHeard { id, take, heard } => self.set_take_heard(*id, *take, heard),
            Operation::SetMarks { id, marks } => self.set_marks(*id, marks),
            Operation::SetBrollNote { id, note } => self.set_broll_note(*id, note),
            Operation::SetCovers { covers } => self.set_covers(covers),
        }
    }

    fn read(&self) -> Result<Vec<Block>> {
        let voice = if self.legacy { "'', 0.0, ''" } else { "voice_uri, voice_seconds, voice_tc" };
        let fields = if self.no_fields { "'{}'" } else { "fields_json" };
        let pieces = if self.no_pieces { "'[]'" } else { "pieces_json" };
        let marks = if self.no_marks { "'[]'" } else { "marks_json" };
        let broll = if self.no_broll { "'', '[]'" } else { "broll_note, covers_json" };
        let mut statement = self
            .conn
            .prepare(&format!(
                "SELECT block_id, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, {voice}, {fields}, {pieces}, {marks}, {broll}
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
                    marks: serde_json::from_str(&r.get::<_, String>(16)?).unwrap_or_default(),
                    broll_note: r.get(17)?,
                    covers: serde_json::from_str(&r.get::<_, String>(18)?).unwrap_or_default(),
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
    pub(crate) fn kind_spec(&self, code: &str) -> Result<KindSpec> {
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
    pub(crate) fn kind_of_block(&self, id: i64) -> Result<String> {
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
        // The markers stay with their words; a word deleted takes its marker with it.
        let (old, marks): (String, String) = tx.query_row("SELECT text, marks_json FROM script_blocks WHERE block_id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(err)?;
        let marks = serde_json::to_string(&marks_after_edit(&old, text, &serde_json::from_str::<Vec<u32>>(&marks).unwrap_or_default())).map_err(err)?;
        tx.execute("UPDATE script_blocks SET text = ?2, marks_json = ?3, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, text, marks]).map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    /// The M markers in the text of block `id`, each before a word of it but the first.
    fn set_marks(&mut self, id: i64, marks: &[u32]) -> Result<Data> {
        let kind = self.kind_spec(&self.kind_of_block(id)?)?;
        let text: String = self.conn.query_row("SELECT text FROM script_blocks WHERE block_id = ?1", params![id], |r| r.get(0)).map_err(err)?;
        let words = text.split_whitespace().count() as u32;
        if kind.role == Role::Clip || marks.iter().any(|mark| *mark == 0 || *mark >= words) {
            return Err(qnc_ui_text::text("db.script_mark_invalid").into());
        }
        let mut marks = marks.to_vec();
        marks.sort_unstable();
        marks.dedup();
        let json = serde_json::to_string(&marks).map_err(err)?;
        self.conn.execute("UPDATE script_blocks SET marks_json = ?2, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, json]).map_err(err)?;
        Ok(Data::Changed)
    }

    /// The B-roll note of a text block a reporter reads (an OFF), as typed.
    fn set_broll_note(&mut self, id: i64, note: &str) -> Result<Data> {
        let kind = self.kind_spec(&self.kind_of_block(id)?)?;
        if kind.role != Role::Spoken || note.len() > crate::NOTE_MOST || note.chars().any(|c| c.is_control() && c != '\n') {
            return Err(qnc_ui_text::text("db.script_broll_invalid").into());
        }
        self.conn.execute("UPDATE script_blocks SET broll_note = ?2, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, note]).map_err(err)?;
        Ok(Data::Changed)
    }

    /// The covers of the text blocks named (inside the one transaction `execute` opened: one
    /// refused rolls them all back).
    fn set_covers(&mut self, covers: &[(i64, Vec<crate::Cover>)]) -> Result<Data> {
        for (id, block_covers) in covers {
            let kind = self.kind_spec(&self.kind_of_block(*id)?)?;
            let valid = block_covers.iter().all(|cover| !cover.clip_id.is_empty() && cover.to_frame > cover.from_frame && cover.seconds > 0.0 && cover.at_seconds >= 0.0 && cover.seconds.is_finite() && cover.at_seconds.is_finite());
            if kind.role != Role::Spoken || !valid || block_covers.len() > 200 {
                return Err(qnc_ui_text::text("db.script_broll_invalid").into());
            }
            let json = serde_json::to_string(block_covers).map_err(err)?;
            self.conn.execute("UPDATE script_blocks SET covers_json = ?2, updated_at_utc = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE block_id = ?1", params![id, json]).map_err(err)?;
        }
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
        block.pieces.push(Piece { gap_before: 0, cut: false, a1_lead_frames: 0, a1_tail_frames: 0, v_lead_frames: 0, v_tail_frames: 0, ..piece.clone() });
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

    /// The pictures the smooth cut of the join before part `index` replaces; the first part
    /// has no join.
    fn set_smooth(&mut self, id: i64, index: usize, frames: u64) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let Some(piece) = block.pieces.get_mut(index).filter(|_| index > 0) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        piece.smooth_frames = frames;
        self.store_pieces(id, block)
    }

    fn set_smooth_clip(&mut self, id: i64, index: usize, clip_id: &str) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let Some(piece) = block.pieces.get_mut(index).filter(|_| index > 0) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        piece.smooth_clip = clip_id.to_string();
        self.store_pieces(id, block)
    }

    /// Part `index` with its new range (an edge trimmed on the SubSegment timeline): the same
    /// clip, a range of at least one frame; its pause and join stay.
    fn set_piece(&mut self, id: i64, index: usize, piece: &Piece) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        // A statement of one range is its own first part (cut by its text, user 2026-11-04).
        if block.pieces.is_empty() {
            block.pieces.push(Piece::of(&block));
        }
        let Some(stored) = block.pieces.get_mut(index) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        if piece.clip_id != stored.clip_id || piece.to_frame <= piece.from_frame {
            return Err(qnc_ui_text::fill("db.script_clip_range", &[("kind", &block.kind)]));
        }
        *stored = Piece { gap_before: stored.gap_before, cut: stored.cut, a1_lead_frames: stored.a1_lead_frames, a1_tail_frames: stored.a1_tail_frames, v_lead_frames: stored.v_lead_frames, v_tail_frames: stored.v_tail_frames, smooth_frames: stored.smooth_frames, smooth_clip: stored.smooth_clip.clone(), ..piece.clone() };
        self.store_pieces(id, block)
    }

    /// The J and L of the join before part `index` (user 2026-11-03); the first part has none.
    fn set_join_sound(&mut self, id: i64, index: usize, lead: i64, tail: i64) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let Some(piece) = block.pieces.get_mut(index).filter(|_| index > 0) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        (piece.a1_lead_frames, piece.a1_tail_frames) = (lead, tail);
        self.store_pieces(id, block)
    }

    /// The J and L of the picture at the join before part `index`; the first part has none.
    fn set_join_picture(&mut self, id: i64, index: usize, lead: u64, tail: u64) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        let Some(piece) = block.pieces.get_mut(index).filter(|_| index > 0) else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        (piece.v_lead_frames, piece.v_tail_frames) = (lead, tail);
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
        let joins: Vec<(u64, bool, i64, i64, u64, u64, u64)> = block.pieces.iter().map(|piece| (piece.gap_before, piece.cut, piece.a1_lead_frames, piece.a1_tail_frames, piece.v_lead_frames, piece.v_tail_frames, piece.smooth_frames)).collect();
        block.pieces.swap(index, other);
        for (piece, (gap, cut, lead, tail, v_lead, v_tail, smooth)) in block.pieces.iter_mut().zip(joins) {
            (piece.gap_before, piece.cut, piece.a1_lead_frames, piece.a1_tail_frames, piece.v_lead_frames, piece.v_tail_frames, piece.smooth_frames) = (gap, cut, lead, tail, v_lead, v_tail, smooth);
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
            (first.gap_before, first.cut, first.a1_lead_frames, first.a1_tail_frames, first.v_lead_frames, first.v_tail_frames, first.smooth_frames) = (0, false, 0, 0, 0, 0, 0);
        }
        self.store_pieces(id, block)
    }

    /// Words cut out of part `index` (user 2026-11-05): the part becomes the pieces before and
    /// after them, each the same clip within the part's range; the first keeps the part's join,
    /// the second joins it with a cut and no pause (user 2026-11-05: no crossfade). Neither: the part leaves.
    fn cut_out(&mut self, id: i64, index: usize, before: Option<&Piece>, after: Option<&Piece>) -> Result<Data> {
        let kept: Vec<Piece> = before.into_iter().chain(after).cloned().collect();
        self.keep_of(id, index, &kept)
    }

    /// Part `index` of block `id` becomes the pieces `kept` of it (in order, none overlapping):
    /// what the text edited still says (user 2026-11-06: "what is deleted in the text is deleted
    /// in the sound"); the first keeps the part's join, the others join it with a cut.
    fn keep_of(&mut self, id: i64, index: usize, kept: &[Piece]) -> Result<Data> {
        let mut block = self.read()?.into_iter().find(|block| block.id == id).ok_or_else(|| qnc_ui_text::text("db.script_no_block").to_string())?;
        if block.pieces.is_empty() {
            block.pieces.push(Piece::of(&block));
        }
        let Some(stored) = block.pieces.get(index).cloned() else {
            return Err(qnc_ui_text::text("db.script_no_join").into());
        };
        if kept.is_empty() {
            return self.remove_piece(id, index);
        }
        let fits = |piece: &Piece| piece.clip_id == stored.clip_id && piece.to_frame > piece.from_frame && piece.from_frame >= stored.from_frame && piece.to_frame <= stored.to_frame;
        if !kept.iter().all(fits) || kept.windows(2).any(|pair| pair[0].to_frame > pair[1].from_frame) {
            return Err(qnc_ui_text::fill("db.script_clip_range", &[("kind", &block.kind)]));
        }
        let joined = |piece: &Piece| Piece { gap_before: stored.gap_before, cut: stored.cut, a1_lead_frames: stored.a1_lead_frames, a1_tail_frames: stored.a1_tail_frames, v_lead_frames: stored.v_lead_frames, v_tail_frames: stored.v_tail_frames, smooth_frames: stored.smooth_frames, smooth_clip: stored.smooth_clip.clone(), ..piece.clone() };
        let fresh = |piece: &Piece| Piece { gap_before: 0, cut: true, a1_lead_frames: 0, a1_tail_frames: 0, v_lead_frames: 0, v_tail_frames: 0, ..piece.clone() };
        let new: Vec<Piece> = kept.iter().enumerate().map(|(at, piece)| if at == 0 { joined(piece) } else { fresh(piece) }).collect();
        block.pieces.splice(index..=index, new);
        if let Some(first) = block.pieces.first_mut() {
            (first.gap_before, first.cut, first.a1_lead_frames, first.a1_tail_frames, first.v_lead_frames, first.v_tail_frames) = (0, false, 0, 0, 0, 0);
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
        crate::voice::forget(&tx, id)?;
        renumber(&tx)?;
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }

    pub(crate) fn connection_mut(&mut self) -> &mut Connection {
        &mut self.conn
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

pub(crate) fn view_exists(conn: &Connection, name: &str) -> Result<bool> {
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
            marks_json TEXT NOT NULL DEFAULT '[]',
            broll_note TEXT NOT NULL DEFAULT '',
            covers_json TEXT NOT NULL DEFAULT '[]',
            updated_at_utc TEXT NOT NULL";

/// The M markers of `old` in the text as edited to `new`: each with its word, the words the two
/// have in common found in order; a marker whose word is gone goes with it.
fn marks_after_edit(old: &str, new: &str, marks: &[u32]) -> Vec<u32> {
    let (old, new): (Vec<&str>, Vec<&str>) = (old.split_whitespace().collect(), new.split_whitespace().collect());
    let mut common = vec![vec![0u32; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            common[i][j] = if old[i] == new[j] { common[i + 1][j + 1] + 1 } else { common[i + 1][j].max(common[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut kept) = (0, 0, Vec::new());
    while i < old.len() && j < new.len() {
        if old[i] == new[j] {
            if marks.contains(&(i as u32)) && j > 0 {
                kept.push(j as u32);
            }
            (i, j) = (i + 1, j + 1);
        } else if common[i + 1][j] >= common[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    kept
}

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
    // Blocks written before M markers in the text keep everything and get none.
    if !has_column(conn, "marks_json")? {
        conn.execute_batch("DROP VIEW IF EXISTS public_script_blocks; ALTER TABLE script_blocks ADD COLUMN marks_json TEXT NOT NULL DEFAULT '[]';").map_err(err)?;
    }
    // Blocks written before the B-roll of a text block keep everything and get none.
    if !has_column(conn, "covers_json")? {
        conn.execute_batch(
            "DROP VIEW IF EXISTS public_script_blocks;
             ALTER TABLE script_blocks ADD COLUMN broll_note TEXT NOT NULL DEFAULT '';
             ALTER TABLE script_blocks ADD COLUMN covers_json TEXT NOT NULL DEFAULT '[]';",
        )
        .map_err(err)?;
    }
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS script_blocks ({COLUMNS});
        CREATE VIEW IF NOT EXISTS public_script_blocks AS
            SELECT block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, voice_uri, voice_seconds, voice_tc, fields_json, pieces_json, marks_json, broll_note, covers_json, updated_at_utc
            FROM script_blocks;
        CREATE TABLE IF NOT EXISTS script_profile (
            singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
            profile_id TEXT NOT NULL,
            profile_json TEXT NOT NULL,
            updated_at_utc TEXT NOT NULL);
        CREATE VIEW IF NOT EXISTS public_script_profile AS
            SELECT profile_id, profile_json, updated_at_utc FROM script_profile;"
    ))
    .map_err(err)?;
    conn.execute_batch(crate::undo::SCHEMA).map_err(err)?;
    crate::voice::ensure_schema(conn)
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

/// A change of the script's blocks one undo takes back (a recording is not: it stays).
fn undoable(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Add { .. }
            | Operation::SetText { .. }
            | Operation::Move { .. }
            | Operation::Remove { .. }
            | Operation::SetFields { .. }
            | Operation::SetMarks { .. }
            | Operation::SetBrollNote { .. }
            | Operation::SetCovers { .. }
            | Operation::AddPiece { .. }
            | Operation::SetJoin { .. }
            | Operation::SetSmooth { .. }
            | Operation::SetPiece { .. }
            | Operation::SetJoinSound { .. }
            | Operation::SetJoinPicture { .. }
            | Operation::MovePiece { .. }
            | Operation::RemovePiece { .. }
            | Operation::CutOut { .. }
            | Operation::KeepOf { .. }
            | Operation::Together { .. }
    )
}

/// A change `Together` takes: a change of a block's parts.
fn part_change(operation: &Operation) -> bool {
    matches!(operation, Operation::SetPiece { .. } | Operation::SetJoin { .. } | Operation::SetJoinSound { .. } | Operation::SetJoinPicture { .. })
}

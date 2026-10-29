//! The Story tables of a project database and their requests (moved unchanged from
//! the former project content store; v5 `story/*.rs`).

use crate::*;
use rusqlite::{params, Connection, OptionalExtension};
use std::time::{SystemTime, UNIX_EPOCH};

/// The tables this module owns; it writes no other.
mod checks;
mod cover_split;
mod covers;
mod leads;
mod markers;
mod order;
mod reads;
mod rules;
mod schema;
mod segments;
mod slots;
mod sound;
mod spans;

pub(crate) use checks::*;
pub(crate) use cover_split::*;
pub(crate) use leads::*;
pub(crate) use order::*;
pub(crate) use rules::*;
pub use rules::story_per_source;
pub(crate) use schema::*;
pub(crate) use slots::*;
pub(crate) use spans::*;

const OWNED: [&str; 8] = [
    "story_revision",
    "story_parts",
    "story_markers",
    "story_marker_slots",
    "story_covers",
    "story_state",
    "story_object_history",
    "story_undo",
];

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the Story tables if missing (read-write) and serves them; a read-only
    /// connection of a project without a story reads an empty one.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            ensure_story_schema(&tx)?;
            crate::undo::ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='story_state')
                    AND EXISTS(SELECT 1 FROM sqlite_master WHERE name='story_undo')",
                [],
                |r| r.get(0),
            )
            .map_err(err)?;
        // A story written by an older schema is not read (external audit 2026-10-17: the
        // reader failed on columns the writer adds); the owner brings it up first.
        if schema_ready && schema_outdated(&conn)? {
            return Err(qnc_ui_text::text("story.schema_outdated").into());
        }
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        if !self.schema_ready {
            return match operation {
                Operation::ListSegments => Ok(Data::Segments(Vec::new())),
                Operation::ListMarkers => Ok(Data::Markers(Vec::new())),
                Operation::ListSlots => Ok(Data::Slots(Vec::new())),
                Operation::ListCovers => Ok(Data::Covers(Vec::new())),
                Operation::ReadProgramSnapshot => Ok(Data::Program(ProgramSnapshot::default())),
                Operation::ReadStorySelection => {
                    Ok(Data::StorySelection(StorySelection::default()))
                }
                _ => Err(qnc_ui_text::text("story.project_story_initialised").into()),
            };
        }
        if !operation.is_write() {
            return self.dispatch(operation);
        }
        // Every write is one transaction (external audit 2026-10-17): an edit, its check and
        // its undo step are written together or not at all, so no other process ever reads
        // a half done or refused edit, and a crash between them leaves nothing behind.
        self.conn.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
        match self.write_in_transaction(operation) {
            Ok(data) => match self.conn.execute_batch("COMMIT") {
                Ok(()) => Ok(data),
                Err(error) => {
                    let _ = self.conn.execute_batch("ROLLBACK");
                    Err(err(error))
                }
            },
            Err(error) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(error)
            }
        }
    }

    fn write_in_transaction(&mut self, operation: &Operation) -> Result<Data> {
        if !operation.edits_story() {
            let data = self.dispatch(operation)?;
            if matches!(operation, Operation::UndoStory | Operation::RedoStory) {
                bump_revision(&self.conn)?;
            }
            return Ok(data);
        }
        let before = crate::undo::capture(&self.conn)?;
        let short = cover_shortfalls(&self.conn)?;
        let data = self.dispatch(operation)?;
        // An M marker is the OUT of one cover and the IN of the next (user 2026-10-17): no
        // edit may leave a cover shorter than its slot, so no hole shows the segment under
        // it. Such an edit is refused; the transaction takes it back whole.
        if let Some(refusal) = cover_hole(&short, &cover_shortfalls(&self.conn)?) {
            return Err(refusal);
        }
        crate::undo::record(&self.conn, &before)?;
        bump_revision(&self.conn)?;
        Ok(data)
    }

    /// Segments, covers and rate in one read transaction, with the revision.
    fn read_program_snapshot(&mut self) -> Result<Data> {
        self.conn.execute_batch("BEGIN").map_err(err)?;
        let read = (|| -> Result<ProgramSnapshot> {
            let Data::Segments(segments) = self.list_segments()? else {
                return Err(qnc_ui_text::text("story.invalid_reply_story").into());
            };
            let Data::Covers(covers) = self.list_covers()? else {
                return Err(qnc_ui_text::text("story.invalid_reply_story").into());
            };
            let Data::Markers(markers) = self.list_markers()? else {
                return Err(qnc_ui_text::text("story.invalid_reply_story").into());
            };
            let Data::Slots(slots) = self.list_slots()? else {
                return Err(qnc_ui_text::text("story.invalid_reply_story").into());
            };
            let Data::StorySelection(selection) = self.read_story_selection()? else {
                return Err(qnc_ui_text::text("story.invalid_reply_story").into());
            };
            let story_fps = selection.story_fps;
            Ok(ProgramSnapshot { revision: read_revision(&self.conn)?, segments, covers, story_fps, markers, slots, selection })
        })();
        let _ = self.conn.execute_batch(if read.is_ok() { "COMMIT" } else { "ROLLBACK" });
        read.map(Data::Program)
    }

    fn dispatch(&mut self, operation: &Operation) -> Result<Data> {
        // The lead-in and lead-out are kept by the owner (user 2026-10-24), never edited.
        match operation {
            Operation::DeleteSegment { segment_id }
            | Operation::IncludeSegment { segment_id }
            | Operation::ReplaceSegment { segment_id, .. }
            | Operation::TrimSegment { segment_id, .. }
            | Operation::PurgeSegment { segment_id }
            | Operation::MoveSegment { segment_id, .. }
            | Operation::SetSegmentSound { segment_id, .. }
            | Operation::SetSegmentPictureLead { segment_id, .. } => refuse_lead(&self.conn, segment_id)?,
            _ => {}
        }
        match operation {
            Operation::UndoStory => {
                crate::undo::step(&self.conn, true)?;
                Ok(Data::Changed)
            }
            Operation::RedoStory => {
                crate::undo::step(&self.conn, false)?;
                Ok(Data::Changed)
            }
            Operation::CreateSegment {
                project_id,
                kind,
                clip_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
                a1_source_channel,
                a1_level_db,
                a1_off,
            } => self.create_segment(
                project_id,
                kind,
                clip_id,
                (*in_frame, *out_frame),
                (*fps_num, *fps_den),
                (*a1_source_channel, lane_level(*a1_level_db)?, *a1_off),
            ),
            Operation::DeleteSegment { segment_id } => self.delete_segment(segment_id),
            Operation::IncludeSegment { segment_id } => self.include_segment(segment_id),
            Operation::ReplaceSegment {
                segment_id,
                clip_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
            } => self.replace_segment(
                segment_id,
                clip_id,
                (*in_frame, *out_frame),
                (*fps_num, *fps_den),
            ),
            Operation::TrimSegment {
                segment_id,
                in_frame,
                out_frame,
            } => self.trim_segment(segment_id, (*in_frame, *out_frame)),
            Operation::PurgeSegment { segment_id } => self.purge_segment(segment_id),
            Operation::MoveSegment { segment_id, up } => self.move_segment(segment_id, *up),
            Operation::ListSegments => self.list_segments(),
            Operation::CreateMarker {
                part_id,
                local_frame,
            } => self.create_marker(part_id, *local_frame),
            Operation::MoveMarker {
                marker_id,
                program_frame,
            } => self.move_marker(marker_id, *program_frame),
            Operation::MoveMarkerPicture { marker_id, program_frame } => self.move_marker_picture(marker_id, *program_frame),
            Operation::SetCutSound { marker_id, offset } => self.set_cut_sound(marker_id, *offset),
            Operation::SetCutSoundEdges { marker_id, tail_frames, lead_frames } => {
                self.set_cut_sound_edges(marker_id, *tail_frames, *lead_frames)
            }
            Operation::MoveSlot { slot_id, start_frame } => self.move_slot(slot_id, *start_frame),
            Operation::TrimSlot { slot_id, start, frames } => self.trim_slot(slot_id, *start, *frames),
            Operation::DeleteMarker { marker_id } => self.delete_marker(marker_id),
            Operation::ListMarkers => self.list_markers(),
            Operation::ListSlots => self.list_slots(),
            Operation::ListCovers => self.list_covers(),
            Operation::ReadStorySelection => self.read_story_selection(),
            Operation::ReadProgramSnapshot => self.read_program_snapshot(),
            Operation::SelectPart { part_id } => self.select_part(part_id),
            Operation::SelectSlot { slot_id } => self.select_slot(slot_id),
            Operation::CreateCover {
                project_id,
                slot_id,
                clip_id,
                virtual_shot_id,
                in_frame,
                out_frame,
                fps_num,
                fps_den,
                a2_source_channel,
                a2_level_db,
                a2_off,
            } => self.create_cover(
                project_id,
                slot_id,
                (clip_id, virtual_shot_id),
                (*in_frame, *out_frame),
                (*fps_num, *fps_den),
                (*a2_source_channel, lane_level(*a2_level_db)?, *a2_off),
            ),
            Operation::SetSegmentSound { segment_id, channel, level_db, off } => {
                self.set_sound("story_parts", "part_id", "a1", segment_id, (*channel, lane_level(*level_db)?, *off))
            }
            Operation::SetCoverSound { cover_id, channel, level_db, off } => {
                self.set_sound("story_covers", "cover_id", "a2", cover_id, (*channel, lane_level(*level_db)?, *off))
            }
            Operation::SetSegmentPictureLead { segment_id, frames } => self.set_picture_lead(segment_id, *frames),
            Operation::SetCoverOverlap { cover_id, lead_frames, tail_frames } => {
                self.set_cover_overlap(cover_id, *lead_frames, *tail_frames)
            }
            Operation::DeleteCover { cover_id } => self.delete_cover(cover_id),
            Operation::SelectCover { cover_id } => self.select_cover(cover_id),
        }
    }

}

impl Store {
    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
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

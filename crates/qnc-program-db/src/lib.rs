//! The Story of a project (QNC v5 `story_*` tables in the project database):
//! program segments, M markers, M-M slots, covers, the selection and undo. A table
//! module of the project database intermediary (`qnc-db-broker`): it owns only its
//! tables, reads only public views of others (`public_clips`,
//! `public_project_settings`) and is written only through that intermediary. It
//! knows no form, no application and no player.

mod store;
mod undo;

use std::sync::Arc;

use qnc_db_broker::{
    Access, Pending, ProjectDbClient, ProjectDbTarget, ProjectDbWriter, TableModule,
    TableModuleFactory,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MODULE_ID: &str = "qnc.module.program-db";

pub type Result<T> = std::result::Result<T, String>;

/// One segment of the edited program. Stored kind is `tonovi` or `offovi`.
/// Frames are source frames of its clip in the timebase `fps_num/fps_den`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramSegment {
    pub segment_id: String,
    pub kind: String,
    pub sort_index: u32,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub fps_num: u32,
    pub fps_den: u32,
    /// False once deleted (v5 keeps the row, `active = 0`): shown greyed in the
    /// Segment tab, not part of the program.
    pub active: bool,
    /// Source channel heard on A1 (zero based), chosen on the Wrap segment;
    /// channel 1 (0) unless the user picks another one.
    #[serde(default)]
    pub a1_source_channel: u16,
}

/// An M marker on the program axis (v5 `story_markers`). `system_role` is
/// `program_start`, `program_end` (locked) or empty for a user marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramMarker {
    pub marker_id: String,
    pub program_frame: u64,
    pub system_role: String,
}

/// An M-M slot (v5 `story_marker_slots`): `slot_id` is the marker pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramSlot {
    pub slot_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub start_marker_id: String,
    pub end_marker_id: String,
    pub has_cover: bool,
}

/// A cover (v5 `story_covers`) bound to its M-M slot: program frames, the
/// source frames of its clip and the source channel heard on A2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramCover {
    pub cover_id: String,
    pub slot_id: String,
    pub clip_id: String,
    pub virtual_shot_id: String,
    pub program_start_frame: u64,
    pub program_end_frame: u64,
    pub source_in_frame: u64,
    pub source_out_frame: u64,
    pub fps_num: u32,
    pub fps_den: u32,
    pub a2_source_channel: u16,
}

/// The stored Story selection (v5 `story_state`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorySelection {
    pub selected_part_id: String,
    pub selected_slot_id: String,
    pub selected_cover_id: String,
    /// Story edits that can be undone and redone (UNDO / REDO).
    #[serde(default)]
    pub undo_depth: u64,
    #[serde(default)]
    pub redo_depth: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    /// Appends a segment at the end of the program.
    CreateSegment {
        project_id: String,
        kind: String,
        clip_id: String,
        in_frame: u64,
        out_frame: u64,
        fps_num: u32,
        fps_den: u32,
        /// Source channel (zero based) heard on A1, chosen on the source timeline;
        /// channel 1 when not given (v5).
        #[serde(default)]
        a1_source_channel: u16,
    },
    /// Takes a segment out of the program (v5 `delete_part`: `active = 0`); it stays
    /// listed where it was.
    DeleteSegment { segment_id: String },
    /// Puts an excluded segment back where it was (user rule 2026-09-25).
    IncludeSegment { segment_id: String },
    /// Replace (user rule 2026-09-25): the base layer of a segment becomes the
    /// marked source; its kind stays. Longer: the markers after it move right and
    /// the added part is uncovered; shorter: the markers in the cut part go with
    /// their slots and covers, the ones after it move left.
    ReplaceSegment {
        segment_id: String,
        clip_id: String,
        in_frame: u64,
        out_frame: u64,
        fps_num: u32,
        fps_den: u32,
    },
    /// The story as it was before the last edit comes back (UNDO).
    UndoStory,
    /// The last undone story edit comes back (REDO).
    RedoStory,
    /// Removes an excluded segment for good; an active one cannot be purged.
    PurgeSegment { segment_id: String },
    /// Swaps the segment with its neighbour before (`up`) or after it.
    MoveSegment { segment_id: String, up: bool },
    ListSegments,
    /// M placed on a Wrap segment (v5 `create_marker_from_part_frame`): the frame
    /// inside the segment `part_id`; the program frame follows from it. A marker
    /// already on that program frame is refreshed, not duplicated.
    CreateMarker { part_id: String, local_frame: u64 },
    MoveMarker { marker_id: String, program_frame: u64 },
    DeleteMarker { marker_id: String },
    ListMarkers,
    ListSlots,
    ListCovers,
    ReadStorySelection,
    /// v5 `select_part`.
    SelectPart { part_id: String },
    /// v5 `select_marker_slot`.
    SelectSlot { slot_id: String },
    /// v5 `create_cover` from source frames: the B-roll virtual shot of the source
    /// IN/OUT (made first by the virtual shot owner, as v5
    /// `add_virtual_shot_from_frames`) becomes the cover of the slot, replacing a
    /// cover already there, and is selected. A2 starts on source channel 1.
    CreateCover {
        project_id: String,
        slot_id: String,
        clip_id: String,
        virtual_shot_id: String,
        in_frame: u64,
        out_frame: u64,
        fps_num: u32,
        fps_den: u32,
        /// Source channel (zero based) heard on A2, chosen on the source timeline;
        /// channel 1 when not given (v5).
        #[serde(default)]
        a2_source_channel: u16,
    },
    /// v5 `delete_cover`; its B-roll virtual shot stays.
    DeleteCover { cover_id: String },
    /// v5 `select_cover`.
    SelectCover { cover_id: String },
}

impl Operation {
    /// Whether this write edits the story (an UNDO step), not only its selection.
    pub fn edits_story(&self) -> bool {
        matches!(
            self,
            Self::CreateSegment { .. }
                | Self::DeleteSegment { .. }
                | Self::IncludeSegment { .. }
                | Self::ReplaceSegment { .. }
                | Self::PurgeSegment { .. }
                | Self::MoveSegment { .. }
                | Self::CreateMarker { .. }
                | Self::MoveMarker { .. }
                | Self::DeleteMarker { .. }
                | Self::CreateCover { .. }
                | Self::DeleteCover { .. }
        )
    }

    pub fn is_write(&self) -> bool {
        !matches!(
            self,
            Self::ListSegments
                | Self::ListMarkers
                | Self::ListSlots
                | Self::ListCovers
                | Self::ReadStorySelection
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Created(String),
    Segments(Vec<ProgramSegment>),
    Markers(Vec<ProgramMarker>),
    Slots(Vec<ProgramSlot>),
    Covers(Vec<ProgramCover>),
    StorySelection(StorySelection),
    Changed,
}

/// Joins the Story tables to a project database.
pub struct StoryModule;

impl StoryModule {
    pub fn factory() -> Arc<dyn TableModuleFactory> {
        Arc::new(Self)
    }
}

impl TableModuleFactory for StoryModule {
    fn id(&self) -> &'static str {
        MODULE_ID
    }
    fn is_write(&self, payload: &Value) -> bool {
        serde_json::from_value::<Operation>(payload.clone()).is_ok_and(|o| o.is_write())
    }
    fn attach(
        &self,
        connection: rusqlite::Connection,
        access: Access,
    ) -> Result<Box<dyn TableModule>> {
        Ok(Box::new(Attached(store::Store::attach(connection, access)?)))
    }
}

struct Attached(store::Store);

impl TableModule for Attached {
    fn execute(&mut self, payload: Value) -> Result<Value> {
        let operation: Operation = serde_json::from_value(payload).map_err(|e| e.to_string())?;
        let data = self.0.execute(&operation)?;
        serde_json::to_value(data).map_err(|e| e.to_string())
    }
}

fn decode(value: Value) -> Result<Data> {
    serde_json::from_value(value).map_err(|e| e.to_string())
}

fn encode(operation: &Operation) -> Result<Value> {
    serde_json::to_value(operation).map_err(|e| e.to_string())
}

fn wrong() -> String {
    "Neispravan odgovor price.".into()
}

/// Reads of the Story of the active project: a read-only client of its database,
/// so a form reading never waits for a writer.
pub struct StoryReader(ProjectDbClient);

impl StoryReader {
    pub fn open(target: &ProjectDbTarget) -> Result<Self> {
        Ok(Self(target.open(Access::ReadOnly, vec![StoryModule::factory()])?))
    }

    fn call(&mut self, operation: &Operation) -> Result<Data> {
        decode(self.0.execute(MODULE_ID, encode(operation)?)?)
    }

    pub fn list_segments(&mut self) -> Result<Vec<ProgramSegment>> {
        match self.call(&Operation::ListSegments)? {
            Data::Segments(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn list_markers(&mut self) -> Result<Vec<ProgramMarker>> {
        match self.call(&Operation::ListMarkers)? {
            Data::Markers(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn list_slots(&mut self) -> Result<Vec<ProgramSlot>> {
        match self.call(&Operation::ListSlots)? {
            Data::Slots(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn list_covers(&mut self) -> Result<Vec<ProgramCover>> {
        match self.call(&Operation::ListCovers)? {
            Data::Covers(rows) => Ok(rows),
            _ => Err(wrong()),
        }
    }

    pub fn read_story_selection(&mut self) -> Result<StorySelection> {
        match self.call(&Operation::ReadStorySelection)? {
            Data::StorySelection(selection) => Ok(selection),
            _ => Err(wrong()),
        }
    }
}

/// Writes of the Story of the active project, through the one serial writer of its
/// database in this process.
#[derive(Debug, Clone)]
pub struct StoryWriter(ProjectDbWriter);

impl StoryWriter {
    pub fn start(target: ProjectDbTarget) -> Result<Self> {
        Ok(Self(ProjectDbWriter::start(target, vec![StoryModule::factory()])?))
    }

    /// Runs a request and waits for its reply.
    pub fn call(&self, operation: &Operation) -> Result<Data> {
        decode(self.0.call(MODULE_ID, encode(operation)?)?)
    }

    /// Sends a request without waiting; the reply is taken from the returned handle.
    pub fn submit(&self, operation: &Operation) -> Result<StoryPending> {
        Ok(StoryPending(self.0.submit(MODULE_ID, encode(operation)?)?))
    }
}

/// The reply of a submitted Story request, once it is there.
#[derive(Debug)]
pub struct StoryPending(Pending);

impl StoryPending {
    pub fn try_take(&self) -> Option<Result<Data>> {
        self.0.try_take().map(|result| result.and_then(decode))
    }
}

#[cfg(test)]
mod tests;

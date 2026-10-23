use super::*;

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
    /// Level of A1 in dB (0 as recorded) and whether A1 is off, set on the source
    /// timeline and kept for the program, the prerender and the export (user 2026-10-16).
    #[serde(default)]
    pub a1_level_db: i8,
    #[serde(default)]
    pub a1_off: bool,
    /// Length in story frames: the source range times story frames per source frame
    /// (2 for a source of half the story rate, 1/2 for twice; user 2026-10-09).
    #[serde(default)]
    pub duration_frames: u64,
    /// The picture J of a Ton segment (user 2026-10-21): its picture starts that many story
    /// frames before its sound, over the end of the segment before; 0 for none.
    #[serde(default)]
    pub v_lead_frames: u32,
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
    /// Level of A2 in dB (0 as recorded) and whether A2 is off (user 2026-10-16).
    #[serde(default)]
    pub a2_level_db: i8,
    #[serde(default)]
    pub a2_off: bool,
    /// The cover's A2 overlap (user 2026-10-21): frames its sound starts before its picture
    /// (J) and goes on after it (L), story frames; 0 for the project crossfade only.
    #[serde(default)]
    pub a2_lead_frames: u32,
    #[serde(default)]
    pub a2_tail_frames: u32,
}

/// The stored Story selection (v5 `story_state`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramSnapshot {
    /// Grows with every change of the story (an edit, Undo, Redo); 0 for a story that has
    /// not changed since it got its revision.
    pub revision: u64,
    pub segments: Vec<ProgramSegment>,
    pub covers: Vec<ProgramCover>,
    /// The rate of the story; `None` without segments.
    pub story_fps: Option<(u32, u32)>,
    pub markers: Vec<ProgramMarker>,
    pub slots: Vec<ProgramSlot>,
    pub selection: StorySelection,
}

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
    /// The rate of the story (set by its first segment); `None` without segments.
    /// Sources of the same, twice or half that rate make it (user 2026-10-09).
    #[serde(default)]
    pub story_fps: Option<(u32, u32)>,
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
        /// Level of A1 in dB and A1 off, as set on the source timeline.
        #[serde(default)]
        a1_level_db: i8,
        #[serde(default)]
        a1_off: bool,
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
    /// Edit (user rule 2026-10-01): the segment keeps its clip, kind and A1 channel
    /// and takes a new source IN/OUT, shorter or longer at either end. Its M markers
    /// follow the picture; those in a cut part go with their slots and covers; the
    /// markers after it move by the change of length.
    TrimSegment {
        segment_id: String,
        in_frame: u64,
        out_frame: u64,
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
    /// Only the picture of an M marker's cut moves (user 2026-10-21, Ctrl+I/O on a taken
    /// marker): the marker goes to `program_frame` and its A2 sound stays where it was (the
    /// covers' J and L on both sides take the difference). One UNDO step.
    MoveMarkerPicture { marker_id: String, program_frame: u64 },
    /// Only the A2 sound of an M marker's cut moves (Ctrl+2 on a taken marker): `offset` story
    /// frames after the picture cut (an L of the cover before), negative before it (a J of the
    /// cover after). One UNDO step.
    SetCutSound { marker_id: String, offset: i64 },
    /// Both markers of the slot move so it starts at `start_frame` (user 2026-10-10).
    MoveSlot { slot_id: String, start_frame: u64 },
    /// A slot longer or shorter by `frames` story frames at its start (`start`) or its
    /// end (user 2026-10-13, not in v5): the other slots keep their lengths, so the
    /// markers after the slot move by as much and the last slot of the story takes the
    /// difference. At the start the slot keeps its place and its cover starts that much
    /// earlier or later in its source.
    TrimSlot { slot_id: String, start: bool, frames: i64 },
    DeleteMarker { marker_id: String },
    ListMarkers,
    ListSlots,
    ListCovers,
    ReadStorySelection,
    /// The program as one consistent read: its segments, covers and rate, read in one
    /// read transaction, with the story's revision (external audit 2026-10-17: the player
    /// read them apart and could mix an old segment with a new cover).
    ReadProgramSnapshot,
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
        /// Level of A2 in dB and A2 off, as set on the source timeline.
        #[serde(default)]
        a2_level_db: i8,
        #[serde(default)]
        a2_off: bool,
    },
    /// The sound of a segment on A1 changed later (user 2026-10-16): its source
    /// channel, level in dB and whether it is off. An UNDO step.
    SetSegmentSound { segment_id: String, channel: u16, level_db: i8, off: bool },
    /// The sound of a cover on A2 changed later. An UNDO step.
    SetCoverSound { cover_id: String, channel: u16, level_db: i8, off: bool },
    /// The picture J of a Ton segment (user 2026-10-21), story frames. An UNDO step.
    SetSegmentPictureLead { segment_id: String, frames: u32 },
    /// The J and L of a cover's A2 (user 2026-10-21), story frames. An UNDO step.
    SetCoverOverlap { cover_id: String, lead_frames: u32, tail_frames: u32 },
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
                | Self::TrimSegment { .. }
                | Self::PurgeSegment { .. }
                | Self::MoveSegment { .. }
                | Self::CreateMarker { .. }
                | Self::MoveMarker { .. }
                | Self::MoveMarkerPicture { .. }
                | Self::SetCutSound { .. }
                | Self::MoveSlot { .. }
                | Self::TrimSlot { .. }
                | Self::DeleteMarker { .. }
                | Self::CreateCover { .. }
                | Self::DeleteCover { .. }
                | Self::SetSegmentSound { .. }
                | Self::SetCoverSound { .. }
                | Self::SetCoverOverlap { .. }
                | Self::SetSegmentPictureLead { .. }
        )
    }

    pub fn is_write(&self) -> bool {
        !matches!(
            self,
            Self::ListSegments
                | Self::ReadProgramSnapshot
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
    Program(ProgramSnapshot),
    Changed,
}

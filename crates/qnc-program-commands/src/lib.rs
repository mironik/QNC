//! What can be asked of a story program, and the kinds of its segments (QNC public
//! piece): the commands a form, the navigation bar or the keyboard sends, and the one
//! map from a keyboard catalog `action_id` to a command. Pure: no state, no database,
//! no form (split out of `qnc-program-segments`, audit 2026-10-10).

/// Ton carries its picture and sound; Off only its sound (the picture comes later
/// from covers, black until then). Both are heard on A1; covers on A2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentKind {
    Ton,
    Off,
}

impl SegmentKind {
    pub fn db(self) -> &'static str {
        match self {
            Self::Ton => "tonovi",
            Self::Off => "offovi",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "tonovi" => Some(Self::Ton),
            "offovi" => Some(Self::Off),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ton => "Ton",
            Self::Off => "Off",
        }
    }

    pub fn has_base_video(self) -> bool {
        self == Self::Ton
    }
}

/// What a form, the navigation bar or the keyboard asks of the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentCommand {
    /// The mouse wheel over a Wrap row: away from the user (`up`) or toward, Ctrl held or
    /// not; the panel's arrow or Ctrl+arrow (user 2026-10-16).
    Wheel { up: bool, ctrl: bool },
    Select(String),
    /// Previous (`up`) or next segment: select it and cue its start (arrows, ⏮ ⏭).
    Step {
        up: bool,
    },
    /// Isključi: the segment leaves the program and stays listed where it was.
    Exclude(String),
    /// Uključi: an excluded segment comes back where it was.
    Include(String),
    /// Izbriši: an excluded segment is removed for good.
    Purge(String),
    /// Replace: the segment takes the marked source (clip, IN/OUT) as its base
    /// layer and its length; its kind stays (user rule 2026-09-25).
    Replace(String),
    /// Swap the selected segment with its neighbour (Up / Down).
    Move {
        up: bool,
    },
    /// The selected marker first, otherwise the selected segment (docs/94 section 2).
    DeleteSelected,
    SelectMarker(String),
    /// A right click on an M marker: it is taken as with Ctrl+M (user 2026-10-17).
    TakeMarker(String),
    /// A right click in a slot: it is taken as with Ctrl+S.
    TakeSlot(String),
    /// A right click on A1 (0) or A2 of a Wrap row: the sound of what lies at the program
    /// frame is taken as with Ctrl+1/Ctrl+2 there.
    TakeSound { lane: u8, frame: u64 },
    /// A click on a slot: select it, playhead at the clicked program frame (v5
    /// `SelectMarkerSlot { frame }`).
    SelectSlot {
        slot_id: String,
        frame: u64,
    },
    /// Previous or next M marker from the playhead.
    StepMarker {
        up: bool,
    },
    /// Previous or next M-M slot.
    StepSlot {
        up: bool,
    },
    /// Cue the start of the program (🏠).
    ProgramStart,
    /// A program frame the user pointed at.
    Cue(u64),
    /// M: a new marker at the playhead, or the selected marker moved there (docs/94 7a).
    Marker,
    /// Talking Head (Ton) or Voice over (Off) from the marked source.
    AddSegment(SegmentKind),
    /// Cover slot / B (empty selected slot) or Overwrite / Shift+B from the marked source.
    Cover {
        overwrite: bool,
    },
    /// Ctrl+M: the selected (or nearest) marker into editing.
    EditMarker,
    /// Shift+M: always a new marker at the playhead, whatever is selected.
    AddMarker,
    /// Arrows while a marker is edited: its draft by frames.
    NudgeMarker(i64),
    /// A mouse drag of a marker: its draft on this program frame.
    DragMarker {
        marker_id: String,
        frame: u64,
    },
    /// Escape while a marker is edited.
    CancelMarkerEdit,
    /// Edit (user rule 2026-10-01): the segment opens in the source timeline with its
    /// IN/OUT; I/O move them, Enter writes them, Escape drops the edit.
    Edit(String),
    /// Escape while a segment is edited.
    CancelEdit,
    /// Ctrl+S (v5 `select_current_marker_slot`): the M-M slot under the playhead.
    SelectSlotAtPlayhead,
    /// Shift+S (v5 `focus_empty_marker_slot`): the first slot without a cover.
    FocusEmptySlot,
    /// Alt+arrows (v5 `navigate_adjacent_program_object`): the next or previous marker
    /// when one is selected, else slot when one is selected, else segment.
    NavigateObject { up: bool },
    /// The Delete key (user rule 2026-09-25): only what was taken with Ctrl+
    /// before (Ctrl+M: the marker) is deleted; a click never arms Delete.
    DeleteFocused,
    /// Sync/B-roll on or off.
    ToggleSync,
    /// How covers are made, one of two (user 2026-10-08): `sync: false` Pokrivalice
    /// (Cover slot, B, Overwrite), `sync: true` Sync/B-roll.
    CoverMode { sync: bool },
    /// Enter: the cover of a closed Sync slot (v5 `sync_cover_enter_or_activate_focused_item`).
    CommitSync,
    /// A click on a cover: select it, playhead at the clicked program frame.
    SelectCover {
        cover_id: String,
        frame: u64,
    },
    /// Ctrl+click on a cover (user rule 2026-09-25: Ctrl+ selects): the cover is
    /// taken, Delete then removes it and Escape lets it go.
    TakeCover {
        cover_id: String,
        frame: u64,
    },
    /// UNDO: the story as it was before the last edit (segments, markers, covers).
    Undo,
    /// REDO: the last undone story edit again.
    Redo,
    /// Ctrl+click on V of a Wrap row: filmstrip or narrow video row (user 2026-10-15).
    ToggleWrapVideo,
}

impl SegmentCommand {
    /// The command behind a keyboard catalog `action_id`.
    pub fn from_action(action_id: &str) -> Option<Self> {
        Some(match action_id {
            "add_marker" => Self::Marker,
            // User rule (2026-09-25): Ctrl+key selects, Shift+key adds.
            "add_marker_continue" => Self::AddMarker,
            // One Delete press sends delete_part, delete_marker and delete_segment (both
            // scopes); delete_marker holds every delete key in every preset, so only it
            // deletes: one press, one delete (the marker, else the cover, else the segment).
            "delete_marker" => Self::DeleteFocused,
            "playlist_input_start" => Self::ProgramStart,
            "step_prev_part" => Self::Step { up: true },
            "step_next_part" => Self::Step { up: false },
            "add_ton_segment" => Self::AddSegment(SegmentKind::Ton),
            "add_off_segment" => Self::AddSegment(SegmentKind::Off),
            "quick_overwrite_cover" => Self::Cover { overwrite: false },
            "overwrite_cover" => Self::Cover { overwrite: true },
            "activate_focused_item" => Self::CommitSync,
            "select_marker" => Self::EditMarker,
            "select_current_marker_slot" => Self::SelectSlotAtPlayhead,
            "focus_empty_slot" => Self::FocusEmptySlot,
            "navigate_prev_object" => Self::NavigateObject { up: true },
            "navigate_next_object" => Self::NavigateObject { up: false },
            "undo_object" => Self::Undo,
            "redo_object" => Self::Redo,
            "toggle_wrap_video" => Self::ToggleWrapVideo,
            "cover_mode_covers" => Self::CoverMode { sync: false },
            "cover_mode_sync" => Self::CoverMode { sync: true },
            _ => return None,
        })
    }
}

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
    /// Ctrl+Delete: the chosen segment, excluded, removed for good.
    PurgeSelected,
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
    /// A right click over the picture (`picture`) or A2 (`lane` 1) of a Wrap row: the nearest
    /// M marker taken with only that part of its cut moving (user 2026-10-21).
    CutPart { marker_id: String, picture: bool, lane: u8 },
    /// A click on a channel in the column of the taken A1 (0) or A2 of a Wrap row: that
    /// channel, as up/down picks it (user 2026-10-18: by mouse too).
    PickChannel { lane: u8, channel: u16 },
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
    /// Shift + left click on a Wrap row: the playhead there and a new marker on it (as
    /// Shift+M; user rule 2026-10-20).
    AddMarkerAt(u64),
    /// Arrows while a marker is edited: its draft by frames.
    NudgeMarker(i64),
    /// A mouse drag of a marker: its draft on this program frame.
    DragMarker {
        marker_id: String,
        frame: u64,
    },
    /// A click on a side of the taken cut's A2 sound (user 2026-10-29): that side (the J of the
    /// cover after or the L of the cover before, the nearer to `frame`) moves alone.
    TakeCutEdge { marker_id: String, frame: u64 },
    /// A drag of a side of the taken cut's A2 sound to `frame`.
    DragCutEdge { marker_id: String, frame: u64 },
    /// Escape while a marker is edited.
    CancelMarkerEdit,
    /// Edit (user rule 2026-10-01): the segment opens in the source timeline with its
    /// IN/OUT; I/O move them, Enter writes them, Escape drops the edit.
    Edit(String),
    /// What a double click (or the key, at the Wrap playhead) enlarges in the place of the
    /// source timeline (user 2026-10-24): a slot with a cover with part of its neighbours, else
    /// the segment there; the same again closes it.
    Zoom { slot: Option<String>, frame: Option<u64> },
    /// The whole program enlarged (a double click on the program row).
    ProgramZoom,
    /// The enlarged slot closes (Escape with nothing taken).
    CloseZoom,
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
    /// The preview normalises A1 and A2 of the program to the project's loudness targets, or
    /// plays them as edited (user 2026-10-24: a key turns it on and off).
    ToggleNormalize,
    /// The export box (user 2026-10-24): a line chosen, export with it, or closed.
    ExportChoose(usize),
    /// The PAL (false) or NTSC (true) tab of the export box (user 2026-10-25).
    ExportTab(bool),
    ExportConfirm,
    ExportCancel,
}

impl SegmentCommand {
    /// The command behind a system action that names what it acts on (a pointer on a Wrap
    /// row: the marker, slot or cover, the frame, the channel); `None` for a key, which acts
    /// on what the playhead or the selection gives (`from_action`). The same action ids as
    /// the keys where one exists: a right click is Ctrl+ (user 2026-10-18).
    pub fn from_event(event: &qnc_action_event::ActionEvent) -> Option<Self> {
        let (target, value) = (event.target.clone(), event.unsigned());
        Some(match (event.action_id.as_str(), target, value) {
            ("timeline_cue_frame", _, Some(frame)) => Self::Cue(frame),
            ("select_marker", Some(id), _) => Self::TakeMarker(id),
            ("select_current_marker_slot", Some(id), _) => Self::TakeSlot(id),
            ("add_marker_continue", _, Some(frame)) => Self::AddMarkerAt(frame),
            ("timeline_take_cover", Some(cover_id), Some(frame)) => Self::TakeCover { cover_id, frame },
            ("timeline_select_cover", Some(cover_id), Some(frame)) => Self::SelectCover { cover_id, frame },
            ("timeline_select_slot", Some(slot_id), Some(frame)) => Self::SelectSlot { slot_id, frame },
            ("timeline_drag_marker", Some(marker_id), Some(frame)) => Self::DragMarker { marker_id, frame },
            ("timeline_take_cut_edge", Some(marker_id), Some(frame)) => Self::TakeCutEdge { marker_id, frame },
            ("timeline_drag_cut_edge", Some(marker_id), Some(frame)) => Self::DragCutEdge { marker_id, frame },
            // The buttons of the segment panel (user 2026-10-18: buttons are system actions too).
            ("segment_select", Some(id), _) => Self::Select(id),
            ("segment_exclude", Some(id), _) => Self::Exclude(id),
            ("segment_include", Some(id), _) => Self::Include(id),
            ("segment_purge", Some(id), _) => Self::Purge(id),
            ("segment_replace", Some(id), _) => Self::Replace(id),
            ("segment_edit", Some(id), _) => Self::Edit(id),
            ("wrap_zoom", slot, frame) => Self::Zoom { slot, frame },
            ("program_zoom", _, _) => Self::ProgramZoom,
            ("export_choose", _, Some(index)) => Self::ExportChoose(usize::try_from(index).ok()?),
            ("export_tab", _, Some(ntsc)) => Self::ExportTab(ntsc == 1),
            ("export_confirm", _, _) => Self::ExportConfirm,
            ("export_cancel", _, _) => Self::ExportCancel,
            (side @ ("segment_move_up" | "segment_move_down"), _, _) => Self::Move { up: side == "segment_move_up" },
            (side @ ("step_prev_segment" | "step_next_segment"), _, _) => Self::Step { up: side == "step_prev_segment" },
            (side @ ("step_prev_marker_slot" | "step_next_marker_slot"), _, _) => Self::StepSlot { up: side == "step_prev_marker_slot" },
            (side @ ("program_prev_marker" | "program_next_marker"), _, _) => Self::StepMarker { up: side == "program_prev_marker" },
            (lane @ ("select_audio_a1" | "select_audio_a2"), None, Some(frame)) => Self::TakeSound { lane: u8::from(lane == "select_audio_a2"), frame },
            (lane @ ("select_audio_a1" | "select_audio_a2"), Some(marker_id), _) => {
                Self::CutPart { marker_id, picture: false, lane: u8::from(lane == "select_audio_a2") }
            }
            ("select_mark_in" | "select_mark_out", Some(marker_id), _) => Self::CutPart { marker_id, picture: true, lane: 0 },
            (lane @ ("source_choose_a1_channel" | "source_choose_a2_channel"), _, Some(channel)) => {
                Self::PickChannel { lane: u8::from(lane == "source_choose_a2_channel"), channel: u16::try_from(channel).ok()? }
            }
            _ => return None,
        })
    }

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
            "segment_purge" => Self::PurgeSelected,
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
            "toggle_loudness_normalize" => Self::ToggleNormalize,
            "wrap_zoom" => Self::Zoom { slot: None, frame: None },
            "cover_mode_covers" => Self::CoverMode { sync: false },
            "cover_mode_sync" => Self::CoverMode { sync: true },
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_action_event::ActionEvent;

    #[test]
    fn a_pointer_names_what_its_key_takes_from_the_playhead() {
        let at = |id: &str| ActionEvent::key(id).at("segments");
        assert_eq!(SegmentCommand::from_event(&at("select_marker").with_target("m")), Some(SegmentCommand::TakeMarker("m".into())), "a right click on a marker takes that marker");
        assert_eq!(SegmentCommand::from_event(&at("select_marker")), None, "the key itself acts on the selection");
        assert_eq!(SegmentCommand::from_event(&at("add_marker_continue").with_value(12u64)), Some(SegmentCommand::AddMarkerAt(12)), "a click that adds is the adding key, there");
        // User 2026-10-21: a right click over the picture names the nearest marker: only its picture moves.
        assert_eq!(SegmentCommand::from_event(&at("select_mark_in").with_target("m")), Some(SegmentCommand::CutPart { marker_id: "m".into(), picture: true, lane: 0 }));
        assert_eq!(SegmentCommand::from_event(&at("select_audio_a2").with_target("m")), Some(SegmentCommand::CutPart { marker_id: "m".into(), picture: false, lane: 1 }));
        assert_eq!(SegmentCommand::from_event(&at("select_mark_out")), None, "a slot edge is taken by its keys only");
        assert_eq!(SegmentCommand::from_event(&at("timeline_cue_frame").with_value(40u64)), Some(SegmentCommand::Cue(40)));
        assert_eq!(SegmentCommand::from_event(&at("select_audio_a2").with_value(7u64)), Some(SegmentCommand::TakeSound { lane: 1, frame: 7 }));
        assert_eq!(SegmentCommand::from_event(&at("source_choose_a1_channel").with_value(3u64)), Some(SegmentCommand::PickChannel { lane: 0, channel: 3 }));
        assert_eq!(SegmentCommand::from_event(&at("play_pause")), None);
        // A button of the segment panel is a system action on its segment (user 2026-10-18).
        assert_eq!(SegmentCommand::from_event(&at("segment_exclude").with_target("p1")), Some(SegmentCommand::Exclude("p1".into())));
        assert_eq!(SegmentCommand::from_event(&at("segment_move_up").with_target("p1")), Some(SegmentCommand::Move { up: true }));
        assert_eq!(SegmentCommand::from_event(&at("program_next_marker")), Some(SegmentCommand::StepMarker { up: false }));
        assert_eq!(SegmentCommand::from_event(&at("add_marker")), None, "a button with a key goes as that key");
    }
}

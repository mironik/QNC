//! What a form shows of a story program (QNC public piece): the stored segments, markers,
//! slots and covers turned into one program on one frame axis, and the questions a form
//! or a piece asks of it. Pure: it reads nothing and writes nothing; the program model
//! (`qnc-program-segments`) gives it the stored rows (split out of it, audit 2026-10-10).

mod edit_stills;
mod markers;
mod program;
pub use program::*;

pub use edit_stills::{edit_stills, mark_clip_edges, point_stills, program_picture, trim_stills, ClipEdge, EditStill, StillAction, StillPicture, StillStrip, story_zoom_clicks};

pub use markers::{
    check_move, first_empty_slot, neighbour_marker, neighbour_segment, neighbour_slot,
    join_at, join_pins, join_segment, program_frame, resolve, slot_at, slots, source_at, MarkerPin, Slot, TakenPart, JOIN_PREFIX,
};
pub use qnc_program_commands::{SegmentCommand, SegmentKind};

use qnc_program_db::{ProgramCover, ProgramSegment};

/// A segment placed on the program axis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentRow {
    pub segment_id: String,
    pub kind: SegmentKind,
    pub clip_id: String,
    pub source_in_frame: u64,
    pub source_out_frame: u64,
    /// Source channel heard on A1 (zero based), its level in dB and whether it is off.
    pub a1_source_channel: u16,
    pub a1_level_db: i8,
    pub a1_off: bool,
    /// Program frames `[start, end)`: segments follow each other without gaps.
    pub start_frame: u64,
    pub end_frame: u64,
    /// Seconds and frames, e.g. `12:07`.
    pub duration_label: String,
    /// `under_3`, `under_5`, `under_7` or `over_7` seconds (v5 duration marks).
    pub duration_color_key: &'static str,
    pub selected: bool,
    /// The automatic lead-in or lead-out (user 2026-10-24): shown glued to the first or last
    /// segment of the user, never edited.
    pub lead: bool,
    /// The level curve of its A1 (docs/98 2.1), points on frames of its clip.
    pub level_keys: Vec<qnc_program_db::LevelKey>,
}

impl SegmentRow {
    pub fn duration_frames(&self) -> u64 {
        self.end_frame - self.start_frame
    }
}

/// One row of the Segment tab (v5 `segment_parts`): every segment in stored
/// order, deleted ones greyed (`active` false) and not clickable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentPart {
    pub segment_id: String,
    pub kind: SegmentKind,
    pub duration_label: String,
    pub active: bool,
    pub selected: bool,
}

/// A cover on the program axis: drawn over its slot (v5 `segment_timeline_covers`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverSpan {
    pub cover_id: String,
    pub slot_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub selected: bool,
    /// The clip the cover plays, its source IN/OUT and the channel heard on A2.
    pub clip_id: String,
    pub source_in_frame: u64,
    pub source_out_frame: u64,
    pub a2_source_channel: u16,
    pub a2_level_db: i8,
    pub a2_off: bool,
    /// The level curve of its A2 (docs/98 2.1), points on frames of its clip.
    pub level_keys: Vec<qnc_program_db::LevelKey>,
}

/// The level curve of one segment (lane 0, its A1) or cover (lane 1, its A2) on the program.
#[derive(Debug, Clone, PartialEq)]
pub struct LevelCurve {
    pub lane: u8,
    pub id: String,
    pub span: qnc_level_curve::Span,
    pub placed: qnc_level_curve::Placed,
    /// It is the taken one (drawn as drafted).
    pub taken: bool,
}

/// A segment's A1 or a cover's A2 taken on the Wrap row: its id, the draft channel, the
/// clip's channel count, off and level in dB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartSoundView {
    pub lane: u8,
    pub id: String,
    pub channel: u16,
    pub channels: u16,
    pub off: bool,
    pub level_db: i8,
}

/// The sound around the cut of the trim zoom (user 2026-10-25: "zoom the audio so the
/// boundary of a word is seen"): the window `[from, to)` of its axis (the program's frames,
/// or a clip's own for a source IN/OUT), the cut, the pieces of sound A1 and A2 play there,
/// and their peaks per column, per mille (empty until read).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ZoomSound {
    pub from: u64,
    pub to: u64,
    pub cut: u64,
    pub lanes: [Vec<qnc_program_waveform::Placement>; 2],
    pub peaks: [Vec<u16>; 2],
    /// The words said on A1 in the window (user 2026-10-26: the start or end of a statement
    /// found in the zoom), on its axis; empty until transcribed.
    pub words: Vec<ZoomWord>,
    /// Voice on A1 no word was heard for (user 2026-10-26), on the zoom's axis.
    pub unheard: Vec<(u64, u64)>,
    /// The taken cut's A2 sound (user 2026-10-29: its J and L in the zoom too).
    pub cut_sound: Option<ZoomCutSound>,
    /// The taken cut (an M marker or a cut between two clips): a click on the zoom's A1 or A2
    /// takes its sound on that lane (user 2026-10-29: the lanes chosen in the zoom too).
    pub taken_cut: Option<String>,
    /// Only the picture of the taken cut is taken (its V button framed red).
    pub picture_taken: bool,
    /// The side of the cut in focus (user 2026-11-04: one zoom for every edit): `false` before
    /// it, `true` after it. The zoom frames the pictures of that side red; a click on a picture
    /// asks for its side (`zoom_pick_side`); the sound a right click then takes is that side's.
    /// `None`: the owner's own red frame (the picture at the cut, `EditStill.cut`).
    pub focus_side: Option<bool>,
    /// The frame the player plays, on the zoom's axis, while it plays (user 2026-10-30: the
    /// playhead moves in the zoom and the wave goes with it).
    pub playhead: Option<u64>,
    /// A1 and A2 as the timeline's lanes show them (AGENTS 0.12, docs/97 step 3): the channel,
    /// how many there are, off, the level, and whether the lane is taken (its channel column
    /// open). `None`: the lane shows only its label, as on a timeline without a choice.
    pub lane_choices: [Option<ZoomLane>; 2],
    /// The level curves on the zoom's axis, given by its owner as its timeline draws them (user
    /// 2026-11-07: "the zoom is the same timeline, it uses the same code"): the story's, the
    /// clip's of the source timeline, the parts' of a SubSegment.
    pub level_curves: Vec<LevelCurve>,
}

/// A lane of the zoom as the timeline's lane is: what plays on it now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ZoomLane {
    pub channel: u16,
    pub count: u16,
    pub open: bool,
    pub off: bool,
    pub level_db: i8,
}

/// Where the trim zoom's playhead is (user 2026-10-30: "when the play pauses the zoom's
/// playhead stops there, it does not go back to the IN"): the player's frame while it plays,
/// and after a play the frame it stopped on until the cut moves; `None` otherwise, the window
/// then lies around the cut.
#[derive(Debug, Default)]
pub struct ZoomFollow {
    cut: std::cell::Cell<Option<u64>>,
}

impl ZoomFollow {
    /// `position`: the player's confirmed frame on the zoom's axis and whether it plays.
    pub fn playhead(&self, cut: u64, position: Option<(u64, bool)>) -> Option<u64> {
        match position {
            Some((frame, true)) => {
                self.cut.set(Some(cut));
                Some(frame)
            }
            Some((frame, false)) if self.cut.get() == Some(cut) => Some(frame),
            _ => {
                self.cut.set(None);
                None
            }
        }
    }
}

/// How far the sound of a zoom window `[from, to)` is read: ten windows ahead while it follows
/// a playhead, so the wave is there before the play reaches it; the window itself otherwise.
pub fn zoom_reach(from: u64, to: u64, playhead: Option<u64>) -> u64 {
    if playhead.is_some() { to + to.saturating_sub(from) * 5 } else { to }
}

/// The window of the trim zoom on its axis: a second (`half` frames) on each side of the
/// cut, or of the playhead while the player plays, so the wave moves with the play
/// (user 2026-10-30); never past `total`.
pub fn zoom_window(earliest: u64, latest: u64, half: u64, total: u64, playhead: Option<u64>) -> (u64, u64) {
    let (earliest, latest) = playhead.map_or((earliest, latest), |frame| (frame, frame));
    (earliest.saturating_sub(half), (latest + half).min(total.max(latest + 1)))
}

/// The sides of the taken cut's A2 sound in the zoom: where the sound after starts (its J) and
/// where the sound before ends (its L), on the zoom's axis (`None` for a side with no cover),
/// and the sides that move.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoomCutSound {
    pub marker_id: String,
    /// The lane of the sound: 0 A1 (the cut between two clips), 1 A2 (the covers).
    pub lane: u8,
    pub lead: Option<u64>,
    pub tail: Option<u64>,
    pub moving: Vec<u64>,
}

/// A word of the zoom's window: its frames on the zoom's axis and what was said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZoomWord {
    pub from: u64,
    pub to: u64,
    pub text: String,
    /// Its letters with their frames on the zoom's axis (user 2026-11-05: letters in the trim
    /// zoom too), once the letter aligner laid them; empty before, `None` for a sign without.
    pub letters: Vec<(char, Option<(u64, u64)>)>,
}

/// What a form shows: the program in order, its markers and slots, timebase,
/// length and the program playhead confirmed by the player.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SegmentsView {
    pub rows: Vec<SegmentRow>,
    /// The Segment tab list, deleted segments included.
    pub parts: Vec<SegmentPart>,
    pub markers: Vec<MarkerPin>,
    /// The cuts between two clips on A1 where no M marker stands (user 2026-10-29: every cut
    /// is a marker); they have no slot and only their A1 sound is edited.
    pub joins: Vec<MarkerPin>,
    pub slots: Vec<Slot>,
    /// Covers by program frame.
    pub covers: Vec<CoverSpan>,
    /// The pictures on both sides of an edit being moved (a slot, a marker), from the
    /// draft; their images are filled in by whoever decodes them (user 2026-10-10).
    pub edit_stills: Vec<EditStill>,
    /// The same for a segment edge taken with Ctrl+I/O, on the source timeline: the
    /// marker frame is the source frame of the moved IN or OUT.
    pub source_edit_stills: Vec<EditStill>,
    /// The sound around the cut the trim zoom shows (user 2026-10-25), its fine peaks filled
    /// in by whoever reads them.
    pub zoom_sound: Option<ZoomSound>,
    /// Program frames of the edits being moved (a taken slot's two markers, a taken
    /// marker): the trim playheads, drawn beside the player's own.
    pub trim_frames: Vec<u64>,
    /// The A1 cuts whose picture is away from their sound (the picture J of a Ton segment):
    /// (picture frame, sound frame) on the program, drawn red always (user 2026-10-22).
    pub a1_splits: Vec<(u64, u64)>,
    /// The S-Story (user 2026-10-31): every clip is a slot, and its clip is to its slot what a
    /// cover is to its own (the edit pictures follow it so).
    pub clips_are_slots: bool,
    /// Sync/B-roll is on.
    pub sync_enabled: bool,
    /// The cover a Sync is writing, `[start, end)` on the program, until Enter or Delete
    /// (user 2026-10-18: the user sees it grow, coloured by its length).
    pub sync_draft: Option<(u64, u64)>,
    /// Pokrivalice was chosen (user 2026-10-10: at first neither way is chosen).
    pub covers_chosen: bool,
    /// Story timebase (`num`, `den`); `None` while the program is empty.
    pub timebase: Option<(u32, u32)>,
    pub total_frames: u64,
    /// Program frame of the confirmed player picture in the Wrap view.
    pub playhead: Option<u64>,
    /// Story edits that UNDO and REDO can take back and replay (from the database).
    pub undo_depth: u64,
    /// Something is selected that undo selected can take back (user 2026-10-29).
    pub has_selected_object: bool,
    pub redo_depth: u64,
    /// Last controlled error of a read or write; empty otherwise.
    pub message: String,
    /// The file an export just wrote, while `message` says so (user 2026-10-24: a link).
    pub exported_file: Option<String>,
    /// `message` says an export is running or failed (user 2026-10-24: shown red).
    pub export_alert: bool,
    /// The segment being edited (Edit in the Segment tab), until Enter or Escape.
    pub editing: Option<String>,
    /// The program wave (A1 segments, A2 covers) from the stored clip waves.
    pub peaks: qnc_program_waveform::ProgramPeaks,
    /// The loudness of A1 and A2 (user 2026-10-24: EBU R 128), and whether the preview
    /// normalises them to the project's targets.
    pub loudness: Option<qnc_program_loudness::ProgramLoudness>,
    pub normalize: bool,
    /// The sound assistant is on (user 2026-11-06): its curves are drawn and heard.
    pub sound_assist: bool,
    /// The open export box (user 2026-10-24).
    pub export: Option<qnc_export_choice::ExportChoice>,
    /// The Wrap rows show a narrow video row instead of the filmstrip (Ctrl+click on V,
    /// user 2026-10-15).
    pub video_compact: bool,
    /// A refusal the user must notice (user 2026-10-21: something new taken while something
    /// else waits for Enter), drawn by `qnc-alert`.
    pub alert: Option<qnc_notice::Alert>,
    /// The program window `[start, end)` of an enlarged slot: the slot is 70 % of it, the
    /// slots before and after 15 % each (user 2026-10-21); drawn in the place of the source
    /// timeline as the same Wrap row.
    pub zoom: Option<(u64, u64)>,
    /// The filmstrip pictures of the program (segments with a picture, covers over
    /// them); empty with a narrow video row.
    pub pictures: Vec<qnc_program_filmstrip::ProgramPicture>,
    /// The sound of a stored segment (A1) or cover (A2) taken on the Wrap row and its
    /// draft (user 2026-10-16).
    pub part_sound: Option<PartSoundView>,
    /// The level curve taken on the Wrap row (docs/98 2.1): its lane (0 A1, 1 A2), its item and
    /// its draft; the line of the playhead is its point.
    pub level_line: Option<(u8, String, Vec<qnc_program_db::LevelKey>)>,
    /// The last Cover slot (`false`) or Overwrite (`true`) and when it was asked for,
    /// by click or key: its button flashes for a moment (user 2026-10-13).
    pub cover_pressed: Option<(bool, std::time::Instant)>,
}

impl SegmentsView {
    /// The story's line of the status: its message, a finished export's file as the link
    /// and green, an export running or failed red (user 2026-10-24).
    pub fn status(&self) -> qnc_status_line::Status<'_> {
        let tone = match (&self.exported_file, self.export_alert) {
            (Some(_), _) => qnc_status_line::Tone::Done,
            (None, true) => qnc_status_line::Tone::Alert,
            _ => qnc_status_line::Tone::Plain,
        };
        qnc_status_line::Status { text: &self.message, link: self.exported_file.as_deref(), tone }
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }


    /// Where each clip sounds in the program: segments on A1, covers on A2.
    /// Every level curve on the program (docs/98 2.1): segments on A1, covers on A2, the
    /// taken one as drafted; an item without points has its flat line (0 dB), always drawn so a
    /// right click can take it (user 2026-11-07: "ne vidim liniju razine tona").
    pub fn level_curves(&self) -> Vec<LevelCurve> {
        let taken = |lane: u8, id: &str| self.level_line.as_ref().filter(|(at, taken, _)| *at == lane && taken == id).map(|(_, _, keys)| keys);
        let curve = |lane: u8, id: &str, span: qnc_level_curve::Span, keys: &[qnc_program_db::LevelKey]| {
            let drafted = taken(lane, id);
            let keys = drafted.map_or(keys, Vec::as_slice);
            Some(LevelCurve {
                lane,
                id: id.to_string(),
                span,
                placed: qnc_level_curve::place(span, keys),
                taken: drafted.is_some(),
            })
        };
        let segments = self.rows.iter().filter(|row| !row.lead).filter_map(|row| {
            let span = qnc_level_curve::Span { start: row.start_frame, end: row.end_frame, source_in: row.source_in_frame, source_out: row.source_out_frame };
            curve(0, &row.segment_id, span, &row.level_keys)
        });
        let covers = self.covers.iter().filter_map(|cover| {
            let span = qnc_level_curve::Span { start: cover.start_frame, end: cover.end_frame, source_in: cover.source_in_frame, source_out: cover.source_out_frame };
            curve(1, &cover.cover_id, span, &cover.level_keys)
        });
        segments.chain(covers).collect()
    }

    /// The segment (lane 0) or cover (lane 1) whose level line is at program frame `frame`:
    /// its id, its span and its stored points.
    pub fn level_item_at(&self, lane: u8, frame: u64) -> Option<(String, qnc_level_curve::Span, Vec<qnc_program_db::LevelKey>)> {
        if lane == 0 {
            let row = self.user_segment_at(frame)?;
            let span = qnc_level_curve::Span { start: row.start_frame, end: row.end_frame, source_in: row.source_in_frame, source_out: row.source_out_frame };
            return Some((row.segment_id.clone(), span, row.level_keys.clone()));
        }
        let cover = self.covers.iter().find(|cover| cover.start_frame <= frame && frame < cover.end_frame)?;
        let span = qnc_level_curve::Span { start: cover.start_frame, end: cover.end_frame, source_in: cover.source_in_frame, source_out: cover.source_out_frame };
        Some((cover.cover_id.clone(), span, cover.level_keys.clone()))
    }

    pub fn wave_placements(&self) -> (Vec<qnc_program_waveform::Placement>, Vec<qnc_program_waveform::Placement>) {
        let segments = self.rows.iter().map(|row| qnc_program_waveform::Placement {
            clip_id: row.clip_id.clone(),
            channel: row.a1_source_channel,
            program_start: row.start_frame,
            program_end: row.end_frame,
            source_in: row.source_in_frame,
            source_out: row.source_out_frame,
            level_db: row.a1_level_db,
            off: row.a1_off,
            curve: qnc_program_db::LevelKey::pairs(&row.level_keys),
        });
        let covers = self.covers.iter().map(|cover| qnc_program_waveform::Placement {
            clip_id: cover.clip_id.clone(),
            channel: cover.a2_source_channel,
            program_start: cover.start_frame,
            program_end: cover.end_frame,
            source_in: cover.source_in_frame,
            source_out: cover.source_out_frame,
            level_db: cover.a2_level_db,
            off: cover.a2_off,
            curve: qnc_program_db::LevelKey::pairs(&cover.level_keys),
        });
        (segments.collect(), covers.collect())
    }

    /// Where each clip's picture is in the program: segments with a picture, then the
    /// covers over them.
    pub fn picture_placements(&self) -> Vec<qnc_program_filmstrip::PicturePlacement> {
        let segments = self.rows.iter().filter(|row| row.kind.has_base_video()).map(|row| qnc_program_filmstrip::PicturePlacement {
            clip_id: row.clip_id.clone(),
            program_start: row.start_frame,
            program_end: row.end_frame,
            source_in: row.source_in_frame,
            source_out: row.source_out_frame,
        });
        let covers = self.covers.iter().map(|cover| qnc_program_filmstrip::PicturePlacement {
            clip_id: cover.clip_id.clone(),
            program_start: cover.start_frame,
            program_end: cover.end_frame,
            source_in: cover.source_in_frame,
            source_out: cover.source_out_frame,
        });
        segments.chain(covers).collect()
    }

    pub fn selected(&self) -> Option<&SegmentRow> {
        self.rows.iter().find(|row| row.selected)
    }

    pub fn selected_marker(&self) -> Option<&MarkerPin> {
        self.markers.iter().find(|pin| pin.selected)
    }

    pub fn selected_slot(&self) -> Option<&Slot> {
        self.slots.iter().find(|slot| slot.selected)
    }

    pub fn selected_cover(&self) -> Option<&CoverSpan> {
        self.covers.iter().find(|cover| cover.selected)
    }

    /// Whether a program action can run now: B needs an empty selected slot,
    /// Overwrite a selected slot or cover; the others are always offered.
    pub fn action_enabled(&self, action_id: &str) -> bool {
        match SegmentCommand::from_action(action_id) {
            Some(SegmentCommand::Cover { overwrite: false }) => self.quick_cover_slot().is_ok(),
            Some(SegmentCommand::Cover { overwrite: true }) => self.overwrite_cover_slot().is_ok(),
            Some(SegmentCommand::Undo) => self.undo_depth > 0,
            Some(SegmentCommand::Redo) => self.redo_depth > 0,
            Some(SegmentCommand::UndoSelected { .. }) => self.has_selected_object,
            // Covers need segments (user 2026-10-10).
            Some(SegmentCommand::CoverMode { .. } | SegmentCommand::ToggleSync | SegmentCommand::SendToStory) => {
                !self.rows.is_empty()
            }
            _ => true,
        }
    }

    /// Which of the two ways to make covers is on: Pokrivalice, Sync/B-roll or, at
    /// first, neither (user 2026-10-10).
    pub fn action_selected(&self, action_id: &str) -> bool {
        match SegmentCommand::from_action(action_id) {
            Some(SegmentCommand::CoverMode { sync: true }) => self.sync_enabled,
            Some(SegmentCommand::CoverMode { sync: false }) => self.covers_chosen && !self.sync_enabled,
            _ => false,
        }
    }

    /// Whether an action belongs to the program.
    pub fn handles(&self, action_id: &str) -> bool {
        SegmentCommand::from_action(action_id).is_some()
    }

    /// v5 `quick_cover_target`: the selected slot, while it has no cover.
    pub fn quick_cover_slot(&self) -> Result<&Slot, String> {
        // A chosen slot that already has a B-roll gives way to the empty slot under the
        // Wrap playhead (user 2026-10-23: after Sync the playhead is in the next slot).
        let empty_at_playhead = self.slot_at_playhead().filter(|slot| !slot.has_cover);
        let slot = match self.selected_slot() {
            Some(slot) if slot.has_cover => empty_at_playhead.unwrap_or(slot),
            Some(slot) => slot,
            None => self.slot_at_playhead().ok_or(qnc_ui_text::text("story.msg.pick_slot_for_broll"))?,
        };
        if slot.has_cover {
            return Err(qnc_ui_text::text("story.msg.slot_has_broll").into());
        }
        Ok(slot)
    }

    /// The slot under the Wrap playhead (user 2026-10-13: the playhead says which slot is
    /// meant). An empty slot has no colour strip to click (user 2026-10-17), so B and
    /// Overwrite take the slot under the playhead when none is chosen.
    pub fn slot_at_playhead(&self) -> Option<&Slot> {
        slot_at(&self.slots, self.playhead?)
    }

    /// v5 `overwrite_cover_target`: the selected slot, else the selected cover's slot.
    pub fn overwrite_cover_slot(&self) -> Result<&str, String> {
        self.selected_slot()
            .map(|slot| slot.slot_id.as_str())
            .or_else(|| self.selected_cover().map(|cover| cover.slot_id.as_str()))
            .or_else(|| self.slot_at_playhead().map(|slot| slot.slot_id.as_str()))
            .ok_or_else(|| qnc_ui_text::text("story.msg.pick_slot_for_broll").into())
    }

    /// The Wrap rows (user 2026-10-24: the lead-in and lead-out are glued to the first and last
    /// segment of the user): each user segment with the program window it is drawn in, which
    /// for the first one starts with the lead-in and for the last one ends with the lead-out.
    pub fn wrap_rows(&self) -> Vec<((u64, u64), &SegmentRow)> {
        let mut shown: Vec<((u64, u64), &SegmentRow)> = Vec::new();
        let mut lead_from = None;
        for row in &self.rows {
            if !row.lead {
                shown.push(((lead_from.take().unwrap_or(row.start_frame), row.end_frame), row));
            } else if let Some(last) = shown.last_mut() {
                last.0 .1 = row.end_frame;
            } else {
                lead_from.get_or_insert(row.start_frame);
            }
        }
        shown
    }

    /// The segment of the user a program frame belongs to: a frame of the lead-in belongs to
    /// the first segment, of the lead-out to the last one (they are glued to them).
    pub fn user_segment_at(&self, program_frame: u64) -> Option<&SegmentRow> {
        self.wrap_rows()
            .into_iter()
            .find(|((start, end), _)| (*start..*end).contains(&program_frame))
            .map(|(_, row)| row)
            .or_else(|| self.rows.iter().rev().find(|row| !row.lead).filter(|_| program_frame >= self.total_frames))
    }

    /// The segment that holds a program frame; at or after the end, the last one.
    pub fn segment_at(&self, program_frame: u64) -> Option<&SegmentRow> {
        self.rows
            .iter()
            .find(|row| (row.start_frame..row.end_frame).contains(&program_frame))
            .or_else(|| {
                self.rows
                    .last()
                    .filter(|_| program_frame >= self.total_frames)
            })
    }

    /// `seconds:frames` of a program length in the story timebase.
    pub fn label(&self, frames: u64) -> String {
        self.timebase
            .map(|(num, den)| duration_label(frames, num, den))
            .unwrap_or_else(|| "--".into())
    }

    /// Program time as HH:MM:SS:FF from 00:00:00:00 (v5 segment panel header): the
    /// internal timecode of the program; exports use the original source timecodes.
    pub fn timecode(&self, frames: u64) -> String {
        self.timebase
            .map(|(num, den)| whole_fps(num, den))
            .filter(|fps| *fps > 0)
            .map(|fps| qnc_source_timecode::format_frames(frames, fps as u32))
            .unwrap_or_else(|| qnc_source_timecode::UNKNOWN.into())
    }
}


#[cfg(test)]
mod quick_cover_tests;

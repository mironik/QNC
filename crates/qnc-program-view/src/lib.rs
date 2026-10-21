//! What a form shows of a story program (QNC public piece): the stored segments, markers,
//! slots and covers turned into one program on one frame axis, and the questions a form
//! or a piece asks of it. Pure: it reads nothing and writes nothing; the program model
//! (`qnc-program-segments`) gives it the stored rows (split out of it, audit 2026-10-10).

mod edit_stills;
mod markers;

pub use edit_stills::{edit_stills, mark_clip_edges, program_picture, trim_stills, ClipEdge, EditStill, StillPicture};

pub use markers::{
    check_move, first_empty_slot, neighbour_marker, neighbour_segment, neighbour_slot,
    program_frame, resolve, slot_at, slots, source_at, MarkerPin, Slot,
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

/// What a form shows: the program in order, its markers and slots, timebase,
/// length and the program playhead confirmed by the player.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SegmentsView {
    pub rows: Vec<SegmentRow>,
    /// The Segment tab list, deleted segments included.
    pub parts: Vec<SegmentPart>,
    pub markers: Vec<MarkerPin>,
    pub slots: Vec<Slot>,
    /// Covers by program frame.
    pub covers: Vec<CoverSpan>,
    /// The pictures on both sides of an edit being moved (a slot, a marker), from the
    /// draft; their images are filled in by whoever decodes them (user 2026-10-10).
    pub edit_stills: Vec<EditStill>,
    /// The same for a segment edge taken with Ctrl+I/O, on the source timeline: the
    /// marker frame is the source frame of the moved IN or OUT.
    pub source_edit_stills: Vec<EditStill>,
    /// Program frames of the edits being moved (a taken slot's two markers, a taken
    /// marker): the trim playheads, drawn beside the player's own.
    pub trim_frames: Vec<u64>,
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
    pub redo_depth: u64,
    /// Last controlled error of a read or write; empty otherwise.
    pub message: String,
    /// The segment being edited (Edit in the Segment tab), until Enter or Escape.
    pub editing: Option<String>,
    /// The program wave (A1 segments, A2 covers) from the stored clip waves.
    pub peaks: qnc_program_waveform::ProgramPeaks,
    /// The Wrap rows show a narrow video row instead of the filmstrip (Ctrl+click on V,
    /// user 2026-10-15).
    pub video_compact: bool,
    /// A refusal the user must notice (user 2026-10-21: something new taken while something
    /// else waits for Enter), drawn by `qnc-alert`.
    pub alert: Option<qnc_alert::Alert>,
    /// The filmstrip pictures of the program (segments with a picture, covers over
    /// them); empty with a narrow video row.
    pub pictures: Vec<qnc_program_filmstrip::ProgramPicture>,
    /// The sound of a stored segment (A1) or cover (A2) taken on the Wrap row and its
    /// draft (user 2026-10-16).
    pub part_sound: Option<PartSoundView>,
    /// The last Cover slot (`false`) or Overwrite (`true`) and when it was asked for,
    /// by click or key: its button flashes for a moment (user 2026-10-13).
    pub cover_pressed: Option<(bool, std::time::Instant)>,
}

impl SegmentsView {
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }


    /// Where each clip sounds in the program: segments on A1, covers on A2.
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
            // Covers need segments (user 2026-10-10).
            Some(SegmentCommand::CoverMode { .. } | SegmentCommand::ToggleSync) => !self.rows.is_empty(),
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
        let slot = self
            .selected_slot()
            .or_else(|| self.slot_at_playhead())
            .ok_or(qnc_ui_text::text("story.msg.pick_slot_for_broll"))?;
        if slot.has_cover {
            return Err(qnc_ui_text::text("story.msg.slot_has_broll").into());
        }
        Ok(slot)
    }

    /// The slot under the Wrap playhead (user 2026-10-13: the playhead says which slot is
    /// meant). An empty slot has no colour strip to click (user 2026-10-17), so B and
    /// Overwrite take the slot under the playhead when none is chosen.
    pub fn slot_at_playhead(&self) -> Option<&Slot> {
        let frame = self.playhead?;
        self.slots.iter().find(|slot| (slot.start_frame..slot.end_frame).contains(&frame))
    }

    /// v5 `overwrite_cover_target`: the selected slot, else the selected cover's slot.
    pub fn overwrite_cover_slot(&self) -> Result<&str, String> {
        self.selected_slot()
            .map(|slot| slot.slot_id.as_str())
            .or_else(|| self.selected_cover().map(|cover| cover.slot_id.as_str()))
            .or_else(|| self.slot_at_playhead().map(|slot| slot.slot_id.as_str()))
            .ok_or_else(|| qnc_ui_text::text("story.msg.pick_slot_for_broll").into())
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

/// Builds the program: the stored order, one window after another, in story frames at
/// the story rate (a source of twice or half that rate is counted in story frames).
pub fn program(
    segments: &[ProgramSegment],
    selected: Option<&str>,
    story_rate: Option<(u32, u32)>,
) -> SegmentsView {
    let timebase = story_rate.or_else(|| {
        segments
            .iter()
            .find(|segment| segment.active && SegmentKind::from_db(&segment.kind).is_some())
            .map(|first| (first.fps_num, first.fps_den))
    });
    let (rate_num, rate_den) = timebase.unwrap_or((0, 1));
    let mut start = 0u64;
    let mut rows = Vec::with_capacity(segments.len());
    let mut parts = Vec::with_capacity(segments.len());
    for segment in segments {
        let Some(kind) = SegmentKind::from_db(&segment.kind) else {
            continue;
        };
        let frames = segment.duration_frames;
        parts.push(SegmentPart {
            segment_id: segment.segment_id.clone(),
            kind,
            duration_label: duration_label(frames, rate_num, rate_den),
            active: segment.active,
            selected: segment.active && selected == Some(segment.segment_id.as_str()),
        });
        if !segment.active {
            continue;
        }
        rows.push(SegmentRow {
            segment_id: segment.segment_id.clone(),
            kind,
            clip_id: segment.clip_id.clone(),
            source_in_frame: segment.in_frame,
            source_out_frame: segment.out_frame,
            a1_source_channel: segment.a1_source_channel,
            a1_level_db: segment.a1_level_db,
            a1_off: segment.a1_off,
            start_frame: start,
            end_frame: start + frames,
            duration_label: duration_label(frames, rate_num, rate_den),
            duration_color_key: duration_color_key(frames, rate_num, rate_den),
            selected: selected == Some(segment.segment_id.as_str()),
        });
        start += frames;
    }
    SegmentsView {
        parts,
        timebase,
        total_frames: start,
        rows,
        ..SegmentsView::default()
    }
}

/// The source frame after the last one a cover plays: its slot length counted in its
/// source rate (the story rate, twice it or half it); the stored OUT when the rates
/// do not fit.
fn played_source_out(cover: &ProgramCover, story_rate: Option<(u32, u32)>) -> u64 {
    let slot = cover.program_end_frame.saturating_sub(cover.program_start_frame);
    story_rate
        .and_then(|rate| qnc_program_db::story_per_source(rate, (cover.fps_num, cover.fps_den)).ok())
        .map_or(cover.source_out_frame, |(num, den)| {
            cover.source_in_frame + (slot * den as u64).div_ceil(num as u64)
        })
}

/// `seconds:frames` with the frame rate rounded to whole frames per second (v5).
pub fn duration_label(frames: u64, fps_num: u32, fps_den: u32) -> String {
    let fps = whole_fps(fps_num, fps_den);
    if fps == 0 {
        return "0:00".into();
    }
    format!("{}:{:02}", frames / fps, frames % fps)
}

/// v5 duration marks: 3, 5 and 7 seconds.
pub fn duration_color_key(frames: u64, fps_num: u32, fps_den: u32) -> &'static str {
    if fps_num == 0 || fps_den == 0 {
        return "over_7";
    }
    let seconds = frames as f64 * f64::from(fps_den) / f64::from(fps_num);
    match seconds {
        s if s < 3.0 => "under_3",
        s if s < 5.0 => "under_5",
        s if s < 7.0 => "under_7",
        _ => "over_7",
    }
}

fn whole_fps(fps_num: u32, fps_den: u32) -> u64 {
    if fps_den == 0 {
        return 0;
    }
    (f64::from(fps_num) / f64::from(fps_den)).round().max(0.0) as u64
}


/// The covers on the program axis, each drawn over its slot (v5 `segment_timeline_covers`),
/// with the source frames it plays.
pub fn cover_spans(covers: &[ProgramCover], selected: Option<&str>, story_rate: Option<(u32, u32)>) -> Vec<CoverSpan> {
    covers
        .iter()
        .map(|cover| CoverSpan {
            cover_id: cover.cover_id.clone(),
            slot_id: cover.slot_id.clone(),
            start_frame: cover.program_start_frame,
            end_frame: cover.program_end_frame,
            selected: selected == Some(cover.cover_id.as_str()),
            clip_id: cover.clip_id.clone(),
            source_in_frame: cover.source_in_frame,
            source_out_frame: played_source_out(cover, story_rate),
            a2_source_channel: cover.a2_source_channel,
            a2_level_db: cover.a2_level_db,
            a2_off: cover.a2_off,
        })
        .collect()
}

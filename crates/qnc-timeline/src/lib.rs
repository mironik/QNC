//! Passive public QNC timeline UI component.
//!
//! The timeline-engine paints a supplied frame-space UI projection and emits
//! user intents. It owns no playback clock, no database, no scanner, no probe,
//! no media processing path, and no Broadcast Player event interpretation.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use qnc_filmstrip::{FILMSTRIP_FRAME_COUNT, FilmstripBackground};
pub use qnc_channel_picker::ChannelChoice;

mod events;
mod marker_reach;
use marker_reach::{marker_for_right_click, marker_to_drag, source_mark_in_reach, taken_marker_for_right_click};
pub use events::take_marker_drag_event;
mod lanes;
pub use events::event;
pub use lanes::{drafted_choice, lane_choice, lane_key, lane_message, Lane, LaneChoice, LaneKey};

pub const MODULE_ID: &str = "qnc.module.timeline";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioLane {
    #[default]
    None,
    A1,
    A2,
    A3,
    A4,
}

impl AudioLane {
    pub fn label(self) -> &'static str {
        match self {
            Self::None => "",
            Self::A1 => "A1",
            Self::A2 => "A2",
            Self::A3 => "A3",
            Self::A4 => "A4",
        }
    }

    pub fn toggle(self, lane: Self) -> Self {
        if lane == Self::None {
            Self::None
        } else if self == lane {
            Self::None
        } else {
            lane
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineLayerFlags {
    pub carrier: bool,
    pub audio_a1: bool,
    pub base_video: bool,
    pub audio_a2: bool,
    pub audio_a3: bool,
    pub audio_a4: bool,
    pub virtual_spans: bool,
    pub covers: bool,
    pub markers: bool,
    pub marker_slots: bool,
    pub shot_range: bool,
    pub in_out: bool,
    pub playhead: bool,
    /// A1 and A2 in the dark bands of the video row (user 2026-10-15, the segment design):
    /// the row has a dark band above and one as high below its light middle; A1 sits in
    /// the band above, A2 in the band below. The stack keeps its height.
    pub audio_bands: bool,
    /// With audio bands: the light middle as low as an audio band, without pictures
    /// (user 2026-10-15: Ctrl+click on V switches a Wrap row between filmstrip and a
    /// narrow video row).
    pub compact_video: bool,
}

/// The dark band above the light middle of the video row (without audio bands).
pub const VIDEO_BAND: f32 = 17.0;
/// The dark strip under the light middle without audio bands.
const VIDEO_FOOT: f32 = 10.0;
/// A cover over the filmstrip of a Wrap row: a strip as high as its label.
/// The cover strip over the filmstrip of a Wrap row: at the bottom of the picture, half as
/// high again as the first one (user 2026-10-17: it was 13 px at the top).
const COVER_STRIP: f32 = 19.5;

impl TimelineLayerFlags {
    pub fn a1_v_a2() -> Self {
        Self {
            carrier: true,
            audio_a1: true,
            base_video: true,
            audio_a2: true,
            audio_a3: false,
            audio_a4: false,
            virtual_spans: false,
            covers: false,
            markers: false,
            marker_slots: false,
            shot_range: false,
            in_out: false,
            playhead: true,
            audio_bands: false,
            compact_video: false,
        }
    }

    /// A1 above and A2 below the light middle of the video row, in its dark bands.
    pub fn with_audio_bands(mut self) -> Self {
        self.audio_bands = true;
        self
    }

    /// With audio bands: a narrow light middle (no pictures).
    pub fn with_compact_video(mut self, compact: bool) -> Self {
        self.compact_video = compact;
        self
    }

    pub fn source() -> Self {
        let mut layers = Self::a1_v_a2().with_source_marks();
        layers.base_video = false;
        // The source range is the whole clip: its outline drew blue lines round every lane
        // and said nothing (user 2026-10-14: no dark blue lines on the source timeline).
        layers.shot_range = false;
        layers
    }

    pub fn with_source_marks(mut self) -> Self {
        self.shot_range = true;
        self.in_out = true;
        self
    }

    pub fn with_overlays(mut self) -> Self {
        self.virtual_spans = true;
        self.covers = true;
        self.markers = true;
        self.marker_slots = true;
        self
    }

    pub fn passive_overlay_projection() -> Self {
        Self::a1_v_a2().with_overlays()
    }

    pub fn video_stack_on(self) -> bool {
        self.carrier
            || self.base_video
            || self.virtual_spans
            || self.covers
            || self.markers
            || self.marker_slots
            || self.shot_range
            || self.in_out
            || self.playhead
    }
}

impl Default for TimelineLayerFlags {
    fn default() -> Self {
        Self::a1_v_a2()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineMetrics {
    pub label_width: f32,
    pub audio_height: f32,
    pub audio_expanded_height: f32,
    pub video_height: f32,
    pub row_gap: f32,
    pub border_width: f32,
}

impl Default for TimelineMetrics {
    fn default() -> Self {
        Self {
            label_width: 28.0,
            audio_height: 18.0, // the 3 px gaps went to the lanes: the stack keeps its v4 height
            audio_expanded_height: 52.0,
            video_height: 64.0,
            // The lanes sit against each other, without lines between them (user 2026-10-14).
            row_gap: 0.0,
            border_width: 1.0,
        }
    }
}

impl TimelineMetrics {
    pub fn lane_height(self, lane: AudioLane, expanded_audio: AudioLane) -> f32 {
        if lane != AudioLane::None && lane == expanded_audio {
            self.audio_expanded_height
        } else {
            self.audio_height
        }
    }

    pub fn content_height(self, layers: TimelineLayerFlags, expanded_audio: AudioLane) -> f32 {
        if layers.audio_bands {
            return self.banded_video_height(layers);
        }
        let mut rows = Vec::new();
        if layers.audio_a1 {
            rows.push(self.lane_height(AudioLane::A1, expanded_audio));
        }
        if layers.video_stack_on() {
            rows.push(self.video_height);
        }
        if layers.audio_a2 {
            rows.push(self.lane_height(AudioLane::A2, expanded_audio));
        }
        if layers.audio_a3 {
            rows.push(self.lane_height(AudioLane::A3, expanded_audio));
        }
        if layers.audio_a4 {
            rows.push(self.lane_height(AudioLane::A4, expanded_audio));
        }
        rows.iter().sum::<f32>() + self.row_gap * rows.len().saturating_sub(1) as f32
    }

    /// The video row with A1 and A2 in its bands: as high as the rows it replaces.
    pub fn banded_video_height(self, layers: TimelineLayerFlags) -> f32 {
        let lanes = [layers.audio_a1, layers.audio_a2].into_iter().filter(|on| *on).count() as f32;
        // Narrow: twice an audio band (user 2026-10-15).
        let middle = if layers.compact_video { 2.0 * self.audio_height } else { self.video_height };
        middle + lanes * self.audio_height
    }

    pub fn outer_height(self, layers: TimelineLayerFlags, expanded_audio: AudioLane) -> f32 {
        self.content_height(layers, expanded_audio) + self.border_width * 2.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineTheme {
    pub background: Color32,
    pub label_background: Color32,
    pub video_background: Color32,
    pub audio_primary_background: Color32,
    pub audio_secondary_background: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub playhead: Color32,
    pub focus: Color32,
    pub shot_range: Color32,
    pub in_out_dim: Color32,
    pub wave_a1: Color32,
    pub wave_a2: Color32,
    pub wave_a3: Color32,
    pub wave_a4: Color32,
    /// The wave on an audio lane (the segment colours of a story use `wave_a1`/`wave_a2`).
    pub audio_wave: Color32,
    /// The rim of the A1 or A2 button whose channel is being changed (user 2026-10-14).
    pub lane_taken: Color32,
    /// The A1 or A2 button of a lane that is off (level 0), nothing of it heard (user
    /// 2026-10-15).
    pub lane_silent: Color32,
    /// An M marker the mouse is close enough to take or drag (user 2026-10-18: red in its
    /// reach, so the user sees it is caught).
    pub marker_hover: Color32,
    /// The thin level lines of an audio lane: alignment (-18 dBFS), permitted maximum
    /// (-9 dBFS), limit (-1 dBFS).
    pub level_marks: [Color32; 3],
}

impl TimelineTheme {
    pub fn from_qnc_theme(
        background: Color32,
        surface: Color32,
        surface_alt: Color32,
        border: Color32,
        text: Color32,
        muted: Color32,
        accent: Color32,
    ) -> Self {
        Self {
            background,
            label_background: surface_alt,
            video_background: surface,
            // Audio lanes in the colours of a story's video segment: its dark green band with
            // the mint of its body as the wave (user 2026-10-14).
            audio_primary_background: Color32::from_rgb(17, 103, 88),
            audio_secondary_background: Color32::from_rgb(17, 103, 88),
            border,
            text,
            muted,
            playhead: accent,
            focus: Color32::from_rgb(255, 180, 60),
            shot_range: Color32::from_rgb(96, 165, 250),
            in_out_dim: Color32::from_black_alpha(133),
            wave_a1: Color32::from_rgb(132, 214, 158),
            wave_a2: Color32::from_rgb(132, 214, 158),
            wave_a3: Color32::from_rgb(132, 214, 158),
            wave_a4: Color32::from_rgb(132, 214, 158),
            audio_wave: Color32::from_rgb(80, 190, 148),
            lane_taken: Color32::from_rgb(240, 200, 40),
            lane_silent: Color32::from_rgb(200, 45, 45),
            marker_hover: Color32::from_rgb(230, 40, 40),
            level_marks: [
                Color32::from_rgba_unmultiplied(220, 230, 225, 70),
                Color32::from_rgba_unmultiplied(240, 200, 40, 150),
                Color32::from_rgba_unmultiplied(220, 60, 50, 170),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimelineSourceMarkFocus {
    #[default]
    Playhead,
    In,
    Out,
}

impl TimelineSourceMarkFocus {
    fn paint_focus(self) -> TimelineFocusPaint {
        match self {
            Self::Playhead => TimelineFocusPaint::Playhead,
            Self::In => TimelineFocusPaint::In,
            Self::Out => TimelineFocusPaint::Out,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineProjection {
    pub range_start_frame: u64,
    pub duration_frames: u64,
    pub playhead_frame: Option<u64>,
    pub cue_enabled: bool,
    pub source_in_frame: Option<u64>,
    pub source_out_frame: Option<u64>,
    pub source_mark_focus: TimelineSourceMarkFocus,
}

impl Default for TimelineProjection {
    fn default() -> Self {
        Self {
            range_start_frame: 0,
            duration_frames: 1,
            playhead_frame: None,
            cue_enabled: false,
            source_in_frame: None,
            source_out_frame: None,
            source_mark_focus: TimelineSourceMarkFocus::Playhead,
        }
    }
}

impl TimelineProjection {
    pub fn new(range_start_frame: u64, duration_frames: u64) -> Self {
        Self {
            range_start_frame,
            duration_frames: duration_frames.max(1),
            playhead_frame: None,
            cue_enabled: false,
            source_in_frame: None,
            source_out_frame: None,
            source_mark_focus: TimelineSourceMarkFocus::Playhead,
        }
    }

    pub fn with_playhead(mut self, playhead_frame: u64) -> Self {
        self.playhead_frame = Some(playhead_frame);
        self
    }

    pub fn with_cue_enabled(mut self, cue_enabled: bool) -> Self {
        self.cue_enabled = cue_enabled;
        self
    }

    pub fn range_start(&self) -> u64 {
        self.range_start_frame
    }

    pub fn confirmed_frame(&self) -> Option<u64> {
        self.playhead_frame
    }

    pub fn duration_frames(&self) -> u64 {
        self.duration_frames.max(1)
    }

    pub fn can_cue(&self) -> bool {
        self.cue_enabled
    }

    pub fn preserving_source_marks_from(mut self, previous: &Self) -> Self {
        self.source_in_frame = previous.source_in_frame;
        self.source_out_frame = previous.source_out_frame;
        self.source_mark_focus = previous.source_mark_focus;
        // While the player prepares the clip again (another A1/A2 channel, live 2026-10-16)
        // it has confirmed no position and its length is not known yet: the marks stay as
        // they were, not clamped to that unknown length.
        if self.playhead_frame.is_none() {
            return self;
        }
        self.normalize_source_marks()
    }

    /// IN at the confirmed frame. Marking does not take the mark (user 2026-10-20: a mark
    /// left taken made the next click on the timeline only let it go, so the playhead moved
    /// on the second click); Ctrl+I/O, a right click or Ctrl+wheel take it.
    pub fn with_source_in_at_confirmed(mut self) -> Self {
        if let Some(frame) = self.confirmed_frame() {
            self.source_in_frame = Some(self.local_frame(frame));
            if let Some(out) = self.source_out_frame {
                if out <= self.source_in_frame.unwrap_or(0) {
                    self.source_out_frame = Some(self.duration_frames());
                }
            }
        }
        self.normalize_source_marks()
    }

    pub fn with_source_out_at_confirmed(mut self) -> Self {
        if let Some(frame) = self.confirmed_frame() {
            self.source_out_frame = Some(
                self.local_frame(frame).max(
                    self.source_in_frame
                        .unwrap_or(0)
                        .saturating_add(1)
                        .min(self.duration_frames()),
                ),
            );
        }
        self.normalize_source_marks()
    }

    /// The first cut of a pair at the playhead: IN there, no OUT yet (user 2026-10-18).
    pub fn with_first_cut_at_confirmed(self) -> Self {
        match self.confirmed_frame() {
            Some(frame) => self.with_first_cut_at(frame),
            None => self.normalize_source_marks(),
        }
    }

    /// The first cut at `frame` (an absolute frame): IN there, no OUT.
    pub fn with_first_cut_at(mut self, frame: u64) -> Self {
        self.source_in_frame = Some(self.local_frame(frame));
        self.source_out_frame = None;
        self.normalize_source_marks()
    }

    /// The second cut at `frame`, the first at `first` (absolute frames): the earlier is IN,
    /// the later OUT.
    pub fn with_second_cut(mut self, first: u64, frame: u64) -> Self {
        let (earlier, later) = (self.local_frame(first.min(frame)), self.local_frame(first.max(frame)));
        self.source_in_frame = Some(earlier);
        self.source_out_frame = Some(later.max(earlier.saturating_add(1)).min(self.duration_frames()));
        self.normalize_source_marks()
    }

    /// The second cut at the playhead, the first at `first` (an absolute frame): the distance
    /// between them is marked, the earlier is IN, the later OUT, whatever their order.
    pub fn with_second_cut_at_confirmed(self, first: u64) -> Self {
        match self.confirmed_frame() {
            Some(frame) => self.with_second_cut(first, frame),
            None => self.normalize_source_marks(),
        }
    }

    pub fn focus_source_in(mut self) -> Self {
        self.source_mark_focus = TimelineSourceMarkFocus::In;
        self
    }

    pub fn focus_source_out(mut self) -> Self {
        self.source_mark_focus = TimelineSourceMarkFocus::Out;
        self
    }

    fn local_frame(&self, absolute_frame: u64) -> u64 {
        absolute_frame
            .saturating_sub(self.range_start_frame)
            .min(self.duration_frames())
    }

    /// The source mark taken (Ctrl+I/O, a right click): `Some(true)` IN, `Some(false)` OUT.
    pub fn taken_source_mark(&self) -> Option<bool> {
        match self.source_mark_focus {
            TimelineSourceMarkFocus::In => Some(true),
            TimelineSourceMarkFocus::Out => Some(false),
            TimelineSourceMarkFocus::Playhead => None,
        }
    }

    /// IN/OUT the source timeline paints. A confirmed player source shows the
    /// whole clip at once, the same way v5 does, until a mark replaces that end.
    pub fn visible_source_marks(&self) -> Option<(u64, u64)> {
        let duration = self.duration_frames;
        if self.playhead_frame.is_some() && duration > 0 {
            let start = self.source_in_frame.unwrap_or(0).min(duration);
            let mut end = self.source_out_frame.unwrap_or(duration).min(duration);
            if end <= start {
                end = start.saturating_add(1).min(duration);
            }
            return (end > start).then_some((start, end));
        }
        match (self.source_in_frame, self.source_out_frame) {
            (Some(start), Some(end)) if end > start => Some((start, end)),
            _ => None,
        }
    }

    /// The length the header shows for the marks `(start, end)`: while OUT is not set, from
    /// IN to the playhead, following it (user 2026-10-16: how far the playhead is from IN);
    /// with OUT set, IN to OUT.
    pub fn shown_source_length(&self, (start, end): (u64, u64)) -> u64 {
        match (self.source_out_frame, self.playhead_frame) {
            (None, Some(playhead)) => self.local_frame(playhead).saturating_sub(start),
            _ => end.saturating_sub(start),
        }
    }

    fn normalize_source_marks(mut self) -> Self {
        let duration = self.duration_frames();
        self.source_in_frame = self.source_in_frame.map(|frame| frame.min(duration));
        self.source_out_frame = self.source_out_frame.map(|frame| frame.min(duration));
        if let (Some(in_frame), Some(out_frame)) = (self.source_in_frame, self.source_out_frame) {
            if out_frame <= in_frame {
                self.source_out_frame = Some(in_frame.saturating_add(1).min(duration));
            }
        }
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineVirtualSpan<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub start_frame: u64,
    pub end_frame: u64,
    pub has_base_video: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineCoverSpan<'a> {
    pub id: &'a str,
    pub start_frame: u64,
    pub end_frame: u64,
    pub selected: bool,
    pub pending: bool,
    /// The colour the caller gives a saved cover (e.g. by its length); the theme's
    /// shot range colour otherwise.
    pub fill: Option<Color32>,
    /// A short text drawn inside a saved cover when it fits (e.g. its length).
    pub label: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineSlotSpan<'a> {
    pub id: &'a str,
    pub start_frame: u64,
    pub end_frame: u64,
    pub has_cover: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineMarkerPin<'a> {
    pub id: &'a str,
    pub frame: u64,
    /// The taken marker (Ctrl+M, a right click): only it has a zone for the mouse (red,
    /// dragged).
    pub taken: bool,
    /// The locked start or end marker of the program: a right click never takes it.
    pub locked: bool,
    /// Its part that moves while taken: red is the whole line, or only its picture's or its
    /// sound's (A2) part (user 2026-10-22).
    pub part: TimelineMarkerPart,
    /// While taken: where it is stored, drawn white (user 2026-10-22: the original stays, the
    /// red is what moves), and where its taken A2 sound's cut is going.
    pub origin: Option<u64>,
    pub sound_at: Option<u64>,
}

/// The part of a taken M marker that moves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TimelineMarkerPart {
    #[default]
    Whole,
    Picture,
    Sound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineProgramRow {
    pub index: usize,
    pub count: usize,
    pub start_frame: u64,
    pub end_frame: u64,
}

impl TimelineProgramRow {
    pub fn new(index: usize, count: usize, start_frame: u64, end_frame: u64) -> Self {
        Self {
            index,
            count,
            start_frame,
            end_frame: end_frame.max(start_frame.saturating_add(1)),
        }
    }

    pub fn overview(duration_frames: u64) -> Self {
        Self::new(0, 1, 0, duration_frames.max(1))
    }

    pub fn start_frame(self) -> u64 {
        self.start_frame
    }

    pub fn end_frame(self) -> u64 {
        self.end_frame.max(self.start_frame.saturating_add(1))
    }

    pub fn duration_frames(self) -> u64 {
        self.end_frame().saturating_sub(self.start_frame()).max(1)
    }

    pub fn is_last(self) -> bool {
        self.count > 0 && self.index.saturating_add(1) == self.count
    }

    pub fn program_frame_from_local(self, local_frame: u64) -> u64 {
        self.start_frame()
            .saturating_add(local_frame.min(self.duration_frames()))
    }

    pub fn local_playhead_frame(self, program_frame: u64) -> Option<u64> {
        let start = self.start_frame();
        let end = self.end_frame();
        let in_row = program_frame >= start
            && (program_frame < end || self.is_last() && program_frame == end);
        in_row.then(|| {
            program_frame
                .saturating_sub(start)
                .min(self.duration_frames())
        })
    }

    pub fn local_range(
        self,
        program_start_frame: u64,
        program_end_frame: u64,
    ) -> Option<(u64, u64)> {
        if program_end_frame <= program_start_frame {
            return None;
        }
        let start = program_start_frame.max(self.start_frame());
        let end = program_end_frame.min(self.end_frame());
        (end > start).then(|| {
            (
                start.saturating_sub(self.start_frame()),
                end.saturating_sub(self.start_frame()),
            )
        })
    }

    pub fn local_marker_frame(self, program_frame: u64) -> Option<u64> {
        let start = self.start_frame();
        let end = self.end_frame();
        if program_frame < start || program_frame > end {
            return None;
        }
        if program_frame == start && self.index != 0 {
            return None;
        }
        if program_frame == end && !self.is_last() {
            return Some(self.duration_frames());
        }
        Some(program_frame.saturating_sub(start))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TimelineFocusPaint {
    #[default]
    Playhead,
    In,
    Out,
}

/// A filmstrip picture on the timeline's frames `[start_frame, end_frame)`, drawn in the
/// light middle of a banded video row (a Wrap row's filmstrip, user 2026-10-15).
#[derive(Debug, Clone, Copy)]
pub struct TimelinePicture<'a> {
    pub start_frame: u64,
    pub end_frame: u64,
    pub frame: &'a qnc_filmstrip::FilmstripFrameAsset,
    /// The filmstrip the picture is from: touching pictures of one strip are one segment
    /// or cover, tiled together.
    pub strip: &'a qnc_filmstrip::FilmstripBackground,
}

#[derive(Clone, Copy)]
pub struct TimelineInput<'a> {
    pub state: &'a TimelineProjection,
    pub layers: TimelineLayerFlags,
    pub metrics: TimelineMetrics,
    pub theme: TimelineTheme,
    pub expanded_audio: AudioLane,
    pub focus: TimelineFocusPaint,
    pub show_lane_labels: bool,
    pub shot_in_frame: u64,
    pub shot_out_frame: u64,
    pub draft_in_frame: u64,
    pub draft_out_frame: u64,
    pub draft_in_active: bool,
    pub draft_out_active: bool,
    pub a1_peaks: &'a [f32],
    pub a2_peaks: &'a [f32],
    pub a3_peaks: &'a [f32],
    pub a4_peaks: &'a [f32],
    pub virtual_spans: &'a [TimelineVirtualSpan<'a>],
    pub covers: &'a [TimelineCoverSpan<'a>],
    pub marker_slots: &'a [TimelineSlotSpan<'a>],
    pub markers: &'a [TimelineMarkerPin<'a>],
    pub base_video_blank: bool,
    pub filmstrip_background: Option<&'a FilmstripBackground>,
    pub video_background: Option<&'a dyn Fn(&mut egui::Ui, Rect)>,
    /// The source channel heard on A1 and how many there are; when given, a click on
    /// the A1 label opens the channel picker right of it (user rule 2026-09-30).
    pub a1_channel: Option<ChannelChoice>,
    /// The same for A2 (the channel heard on A2 in covers), its own choice.
    pub a2_channel: Option<ChannelChoice>,
    /// An audio lane drawn over the video row, at a display gain of 2^(level-1)
    /// (user rule 2026-10-01: a closer look at the wave without a taller dock).
    pub wave_over_video: Option<(AudioLane, u8)>,
    /// Trim playheads: the local frames of the edits being moved and their labels; the
    /// player's playhead stays where it is.
    pub trim_playheads: &'a [(u64, &'a str)],
    /// A1 cuts whose picture is away from their sound, local (picture, sound) frames: red
    /// always (user 2026-10-22).
    pub a1_splits: &'a [(u64, u64)],
    /// Filmstrip pictures of a banded video row (empty: none).
    pub pictures: &'a [TimelinePicture<'a>],
    /// On a row with markers: the host holds something taken there (a slot, a slot edge, a
    /// marker, a sound), so a left click keeps it first (a source timeline knows its own).
    pub holding: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineIntent {
    None,
    CueFrame(u64),
    ToggleAudioExpand(AudioLane),
    SelectVirtual { id: String, frame: u64 },
    /// A double click on a segment of a row: it opens on the source timeline (Edit; user
    /// 2026-10-21).
    OpenVirtual { id: String, frame: u64 },
    /// A double click on the picture or A2 of a row: its slot is shown enlarged in the place of
    /// the source timeline (user 2026-10-21: "a zoom of a part of the Wrap row").
    OpenSlot { id: String, frame: u64 },
    SelectCover { id: String, frame: u64 },
    SelectMarkerSlot { id: String, frame: u64 },
    /// The source channel (zero based) chosen for A1 in the channel picker.
    ChooseA1Channel(u16),
    /// The source channel (zero based) chosen for A2 in its channel picker.
    ChooseA2Channel(u16),
    /// A right click on A1 or A2 of a source timeline: the lane is taken (as Ctrl+1, Ctrl+2).
    TakeAudioLane(AudioLane),
    /// A click on the A1 or A2 label (a button): the lane is taken (as Ctrl+1, Ctrl+2); on a
    /// row with markers the sound under the playhead in the row, else at its start (`frame`).
    LaneLabel { lane: AudioLane, frame: Option<u64> },
    /// A click on the V label: Home of the panel (start of the clip or of the program).
    GoHome,
    /// Ctrl+click on the V label of a banded row: filmstrip or narrow video (user
    /// 2026-10-15).
    ToggleVideoMode,
    /// The mouse wheel over the timeline (user 2026-10-16: the timeline behaves the same
    /// in every form): one step a frame, Ctrl held or not.
    Wheel(qnc_wheel_jog::Wheel),
    /// A left click while something is taken (IN/OUT, A1/A2, a marker, a slot, a sound): it is
    /// kept, as Enter, and then the playhead goes to `Some` frame (user rule 2026-10-20); `None`
    /// keeps only (the end of a drag, a click on a label).
    Keep(Option<u64>),
    /// Shift + left click on a row with markers: an M marker added there (as Shift+M).
    AddMarkerAt(u64),
    /// A right click is Ctrl+ (user 2026-10-17: the right mouse button takes what it points
    /// at): IN (`true`) or OUT, whichever is nearer.
    TakeMark(bool),
    /// IN (`in_mark`) or OUT dragged with the left button to `frame` of the clip (user
    /// 2026-10-17: as in editing programs); letting the button go keeps it (`KeepMark`).
    DragMark { in_mark: bool, frame: u64 },
    /// A right click on an M marker of a row (as Ctrl+M).
    TakeMarker { id: String, frame: u64 },
    /// A right click on a cover's colour strip: its slot (as Ctrl+S).
    TakeSlot { id: String, frame: u64 },
    /// A right click on a cover outside any slot: the cover taken (for Delete).
    TakeCover { id: String, frame: u64 },
    /// Ctrl + left click on a cover's colour strip: the cover taken (for Delete).
    TakeCoverOnStrip { id: String, frame: u64 },
    /// A right click on A1 or A2 of a row with markers (a Wrap row): the sound of what lies
    /// there, at that frame (as Ctrl+1/Ctrl+2 there).
    TakeSound { lane: AudioLane, frame: u64 },
    /// A second right click over A2 near the taken M marker: only its sound moves (user
    /// 2026-10-22: a right click takes the whole marker, a second one its part). The row's
    /// owner decides whether it has parts (only the enlarged slot has).
    TakeMarkerSound { id: String },
    /// A second right click over the picture near the taken M marker: only its picture moves.
    TakeMarkerPicture { id: String },
    /// A cut (scissors) at the playhead of a source timeline: the middle mouse button, a
    /// touchpad's three-finger tap set to it, a two-finger tap on a touch screen (user
    /// 2026-10-18: two cuts mark IN and OUT, the earlier IN, the later OUT).
    Cut,
    /// A double click on a source timeline: a cut at the frame clicked (user 2026-10-26:
    /// "double click for IN/OUT; which is IN and which OUT the timecode decides, IN is the
    /// smaller"), as two cuts do.
    CutAt(u64),
}

impl Default for TimelineIntent {
    fn default() -> Self {
        Self::None
    }
}

pub fn height_for_layers(layers: TimelineLayerFlags, expanded_audio: AudioLane) -> f32 {
    TimelineMetrics::default().outer_height(layers, expanded_audio)
}

pub fn source_player_timeline_height() -> f32 {
    height_for_layers(TimelineLayerFlags::source(), AudioLane::None)
}

pub fn show_source_player_timeline(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &TimelineProjection,
    theme: TimelineTheme,
) -> TimelineIntent {
    show_source_player_timeline_with_filmstrip(ui, rect, state, theme, None)
}

pub fn show_source_player_timeline_with_filmstrip(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &TimelineProjection,
    theme: TimelineTheme,
    filmstrip_background: Option<&FilmstripBackground>,
) -> TimelineIntent {
    show_source_player_timeline_with_artifacts(
        ui,
        rect,
        state,
        theme,
        filmstrip_background,
        &[],
        &[],
        &[],
        &[],
    )
}

pub fn show_source_player_timeline_with_artifacts(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &TimelineProjection,
    theme: TimelineTheme,
    filmstrip_background: Option<&FilmstripBackground>,
    a1_peaks: &[f32],
    a2_peaks: &[f32],
    a3_peaks: &[f32],
    a4_peaks: &[f32],
) -> TimelineIntent {
    let peaks = [a1_peaks, a2_peaks, a3_peaks, a4_peaks];
    show_source_player_timeline_with_channel(ui, rect, state, theme, filmstrip_background, peaks, None, None, None)
}

/// The source timeline with the A1 and A2 channel pickers, when the caller has a
/// choice to make; the same timeline otherwise.
pub fn show_source_player_timeline_with_channel(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &TimelineProjection,
    theme: TimelineTheme,
    filmstrip_background: Option<&FilmstripBackground>,
    [a1_peaks, a2_peaks, a3_peaks, a4_peaks]: [&[f32]; 4],
    a1_channel: Option<ChannelChoice>,
    a2_channel: Option<ChannelChoice>,
    wave_over_video: Option<(AudioLane, u8)>,
) -> TimelineIntent {
    let duration = state.duration_frames();
    let (draft_in, draft_out, in_active, out_active) = match state.visible_source_marks() {
        Some((start, end)) => (start, end, true, true),
        None => (0, duration, false, false),
    };
    let mut out = TimelineIntent::None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        out = show(
            ui,
            TimelineInput {
                state,
                layers: TimelineLayerFlags::source(),
                metrics: TimelineMetrics::default(),
                theme,
                expanded_audio: AudioLane::None,
                focus: state.source_mark_focus.paint_focus(),
                show_lane_labels: true,
                shot_in_frame: 0,
                shot_out_frame: duration,
                draft_in_frame: draft_in,
                draft_out_frame: draft_out,
                draft_in_active: in_active,
                draft_out_active: out_active,
                a1_peaks,
                a2_peaks,
                a3_peaks,
                a4_peaks,
                virtual_spans: &[],
                covers: &[],
                marker_slots: &[],
                markers: &[],
                base_video_blank: true,
                filmstrip_background,
                video_background: None,
                a1_channel,
                a2_channel,
                wave_over_video,
                trim_playheads: &[],
                a1_splits: &[],
                pictures: &[],
                holding: false,
            },
        );
    });
    out
}

pub fn show(ui: &mut egui::Ui, input: TimelineInput<'_>) -> TimelineIntent {
    let width = ui.available_width().max(1.0);
    let height = input
        .metrics
        .outer_height(input.layers, input.expanded_audio)
        .max(1.0);
    let (outer, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    // The mouse wheel and the touchpad, each its own block with its own map (user 2026-10-18).
    let notch = qnc_wheel_jog::take(ui, outer, ui.id().with("qnc-timeline-wheel"));
    let swipe = qnc_touchpad::take(ui, outer, ui.id().with("qnc-timeline-touchpad"));
    let touch = qnc_touch_gestures::take(ui, outer, ui.id().with("qnc-timeline-touch"));
    let wheel = notch.or(swipe).or(touch);
    let over = ui.ctx().pointer_hover_pos().is_some_and(|at| outer.contains(at));
    // A cut (scissors) on every timeline: the middle button over it or a two-finger tap
    // (user 2026-10-18: on the source timeline and on the Wrap rows with the covers).
    let cut = (over && ui.input(|input| input.pointer.button_clicked(egui::PointerButton::Middle)))
        || qnc_touch_gestures::tapped(ui, outer, ui.id().with("qnc-timeline-touch-tap"));
    ui.painter().rect_filled(outer, 0.0, input.theme.background);
    ui.painter().rect_stroke(
        outer,
        0.0,
        Stroke::new(input.metrics.border_width, input.theme.border),
        StrokeKind::Inside,
    );

    let mut next_top = outer.top() + input.metrics.border_width;
    let left = outer.left() + input.metrics.border_width;
    let right = outer.right() - input.metrics.border_width;
    let mut intent = TimelineIntent::None;
    let mut video_row = None;
    let (mut a1_row, mut a2_row) = (None, None);

    if input.layers.audio_a1 && !input.layers.audio_bands {
        let row = next_row(
            &mut next_top,
            left,
            right,
            input
                .metrics
                .lane_height(AudioLane::A1, input.expanded_audio),
            input.metrics.row_gap,
        );
        a1_row = Some(row);
        keep_first_intent(
            &mut intent,
            paint_audio_row(
                ui,
                row,
                AudioLane::A1,
                lane_peaks(&input, input.a1_channel, input.a1_peaks),
                input.theme.audio_primary_background,
                input.theme.audio_wave,
                &input,
            ),
        );
    }
    if input.layers.video_stack_on() {
        let height = if input.layers.audio_bands {
            input.metrics.banded_video_height(input.layers)
        } else {
            input.metrics.video_height
        };
        let row = next_row(&mut next_top, left, right, height, input.metrics.row_gap);
        video_row = Some(row);
        keep_first_intent(&mut intent, paint_video_row(ui, row, &input));
        if input.layers.audio_bands {
            // A1 in the band above the light middle, A2 in the band below (user 2026-10-15).
            // Each band is as high as an audio lane of the source timeline, the light middle as
            // its video row (user 2026-10-15: the Wrap rows in the source timeline's measures).
            let height = input.metrics.audio_height;
            let band = |top: f32| Rect::from_min_max(egui::pos2(row.left(), top), egui::pos2(row.right(), top + height));
            if input.layers.audio_a1 {
                let rect = band(row.top());
                a1_row = Some(rect);
                let peaks = lane_peaks(&input, input.a1_channel, input.a1_peaks);
                keep_first_intent(&mut intent, paint_audio_band(ui, rect, AudioLane::A1, peaks, &input));
            }
            if input.layers.audio_a2 {
                let rect = band(row.bottom() - height);
                a2_row = Some(rect);
                let peaks = lane_peaks(&input, input.a2_channel, input.a2_peaks);
                keep_first_intent(&mut intent, paint_audio_band(ui, rect, AudioLane::A2, peaks, &input));
            }
            // The playhead runs through the whole height of the row, over the bands.
            if input.layers.playhead {
                if let Some(frame) = input.state.confirmed_frame() {
                    let (_, track) = split_label_track(row, input.metrics.label_width);
                    paint_playhead(ui, track, input.state.range_start(), input.state.duration_frames(), frame, &input);
                }
            }
        }
    }
    if input.layers.audio_a2 && !input.layers.audio_bands {
        let row = next_row(
            &mut next_top,
            left,
            right,
            input
                .metrics
                .lane_height(AudioLane::A2, input.expanded_audio),
            input.metrics.row_gap,
        );
        a2_row = Some(row);
        keep_first_intent(
            &mut intent,
            paint_audio_row(
                ui,
                row,
                AudioLane::A2,
                lane_peaks(&input, input.a2_channel, input.a2_peaks),
                input.theme.audio_secondary_background,
                input.theme.audio_wave,
                &input,
            ),
        );
    }
    if input.layers.audio_a3 {
        let row = next_row(
            &mut next_top,
            left,
            right,
            input
                .metrics
                .lane_height(AudioLane::A3, input.expanded_audio),
            input.metrics.row_gap,
        );
        keep_first_intent(
            &mut intent,
            paint_audio_row(
                ui,
                row,
                AudioLane::A3,
                input.a3_peaks,
                input.theme.audio_secondary_background,
                input.theme.audio_wave,
                &input,
            ),
        );
    }
    if input.layers.audio_a4 {
        let row = next_row(
            &mut next_top,
            left,
            right,
            input
                .metrics
                .lane_height(AudioLane::A4, input.expanded_audio),
            input.metrics.row_gap,
        );
        keep_first_intent(
            &mut intent,
            paint_audio_row(
                ui,
                row,
                AudioLane::A4,
                input.a4_peaks,
                input.theme.audio_secondary_background,
                input.theme.audio_wave,
                &input,
            ),
        );
    }
    let taken = [(AudioLane::A1, a1_row, input.a1_channel), (AudioLane::A2, a2_row, input.a2_channel)]
        .into_iter()
        .find_map(|(lane, row, choice)| Some((lane, row?, choice.filter(|c| c.open)?)));
    // The zoom is the lane itself grown over the video row up to the other audio lane
    // (user 2026-10-14): a taken lane shows its draft channel, a zoomed one its channel.
    let grown = |lane: AudioLane, video: Rect| match lane {
        AudioLane::A1 => a1_row.map(|row| Rect::from_min_max(row.min, video.right_bottom())),
        AudioLane::A2 => a2_row.map(|row| Rect::from_min_max(video.min, row.right_bottom())),
        _ => Some(video),
    };
    if let Some(video) = video_row {
        let zoom = match taken {
            Some((lane, _, choice)) => {
                let level = input.wave_over_video.filter(|(over, _)| *over == lane).map_or(1, |(_, level)| level.max(1));
                let own = if lane == AudioLane::A1 { input.a1_peaks } else { input.a2_peaks };
                Some((lane, lane_peaks(&input, Some(choice), own), level))
            }
            None => input.wave_over_video.map(|(lane, level)| {
                let peaks = match lane {
                    AudioLane::A1 => lane_peaks(&input, input.a1_channel, input.a1_peaks),
                    AudioLane::A2 => lane_peaks(&input, input.a2_channel, input.a2_peaks),
                    AudioLane::A3 => input.a3_peaks,
                    AudioLane::A4 => input.a4_peaks,
                    AudioLane::None => &[],
                };
                (lane, peaks, level)
            }),
        };
        if let Some((lane, peaks, level)) = zoom {
            if let Some(rect) = grown(lane, video) {
                paint_wave_over_video(ui, rect, peaks, level, lane_gain(&input, lane), &input);
                paint_zoom_strips(ui, split_label_track(video, input.metrics.label_width).1, &input);
                // The M markers are the layer over the zoom (user 2026-10-22).
                if input.layers.markers {
                    let track = split_label_track(video, input.metrics.label_width).1;
                    let (start, duration) = (input.state.range_start(), input.state.duration_frames());
                    paint_markers(ui, track, start, duration, input.markers, &input);
                    paint_a1_splits(ui, track, start, duration, &input);
                }
            }
        }
    }
    if let Some((lane, row, choice)) = taken {
        // The column of channels: from the lane over the video row, never into the other
        // audio lane.
        // In the label column (over the V label), right under A1 or above A2: the lane and
        // the video row stay visible (user 2026-10-14).
        let (label, _) = split_label_track(row, input.metrics.label_width);
        let down = lane == AudioLane::A1;
        let room = if down {
            (row.bottom(), a2_row.map_or(outer.bottom(), |other| other.top()))
        } else {
            (a1_row.map_or(outer.top(), |other| other.bottom()), row.top())
        };
        let rect = qnc_channel_picker::column_rect(label.left(), label.width(), room, row.height(), down, choice);
        let style = qnc_channel_picker::PickerStyle {
            fill: input.theme.label_background, // the background of the labels, no box of its own
            border: Color32::TRANSPARENT,
            text: input.theme.text,
            accent: input.theme.playhead,
        };
        let id = ui.make_persistent_id(("qnc_timeline_channel_picker", lane.label()));
        if let Some(channel) = qnc_channel_picker::show(ui, id, rect, style, choice) {
            intent = match lane {
                AudioLane::A2 => TimelineIntent::ChooseA2Channel(channel),
                _ => TimelineIntent::ChooseA1Channel(channel),
            };
        } else if ui.rect_contains_pointer(rect) {
            intent = TimelineIntent::None; // the click belongs to the column, not the track under it
        }
    }
    paint_trim_playheads(ui, outer, &input);

    // The middle button also presses on the track; the cut is what it means (live
    // 2026-10-18: the cue of that press won and no cut came).
    if cut {
        return TimelineIntent::Cut;
    }
    // The second click of a double click on a source timeline cues where the first did; it is
    // a cut there (a Wrap row keeps its own double clicks).
    if !input.layers.markers && over && ui.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
        if let TimelineIntent::CueFrame(frame) = intent {
            return TimelineIntent::CutAt(frame);
        }
    }
    match (intent, wheel) {
        // Ctrl+wheel (or the wheel with the right button held) on a source timeline with
        // nothing taken takes IN or OUT, whichever is nearer the mouse, the clip's start or
        // end when the user set none (user 2026-10-17); the next steps move it.
        (TimelineIntent::None, Some(wheel))
            if wheel.ctrl
                && !input.layers.markers
                && input.focus == TimelineFocusPaint::Playhead
                && !lane_taken(&input)
                && (input.draft_in_active || input.draft_out_active) =>
        {
            let track = split_label_track(outer, input.metrics.label_width).1;
            let (range_start, duration) = (input.state.range_start(), input.state.duration_frames());
            let at = ui.ctx().pointer_hover_pos().map_or(track.center().x, |pos| pos.x);
            let local = frame_for_x(track, range_start, duration, at).saturating_sub(range_start);
            TimelineIntent::TakeMark(local.abs_diff(input.draft_in_frame) <= local.abs_diff(input.draft_out_frame))
        }
        (TimelineIntent::None, Some(wheel)) => TimelineIntent::Wheel(wheel),
        (intent, wheel) => {
            if wheel.is_some() && qnc_dev_diagnostics::player_diagnostics_enabled() {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("timeline-wheel lost to {intent:?}"));
            }
            intent
        }
    }
}

/// The trim playheads (user 2026-10-10: a second playhead on the edit being moved, the
/// player's own stays where it is): a line over every lane with its label at the top.
fn paint_trim_playheads(ui: &mut egui::Ui, outer: Rect, input: &TimelineInput<'_>) {
    let border = input.metrics.border_width;
    let row = Rect::from_min_max(
        egui::pos2(outer.left() + border, outer.top() + border),
        egui::pos2(outer.right() - border, outer.bottom() - border),
    );
    let (_, track) = split_label_track(row, input.metrics.label_width);
    let duration = input.state.duration_frames();
    // Where a taken marker's red is, the trim playhead gives only its label (the red line is
    // the moving cut, user 2026-10-22).
    let red = |frame: u64| {
        input.layers.markers
            && input.markers.iter().any(|marker| {
                marker.taken && (match marker.part { TimelineMarkerPart::Sound => marker.sound_at.unwrap_or(marker.frame), _ => marker.frame }) == frame
            })
    };
    for (frame, label) in input.trim_playheads {
        let x = x_for_local_frame(track, duration, *frame);
        let color = input.theme.text;
        if !red(*frame) {
            ui.painter().line_segment([egui::pos2(x, track.top()), egui::pos2(x, track.bottom())], Stroke::new(2.0, color));
        }
        let text = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(11.0), color);
        let left = (x + 3.0).min(track.right() - text.size().x - 2.0).max(track.left());
        let rect = Rect::from_min_size(egui::pos2(left, track.top() + 1.0), text.size() + Vec2::new(4.0, 0.0));
        ui.painter().rect_filled(rect, 2.0, input.theme.background);
        ui.painter().galley(egui::pos2(left + 2.0, track.top() + 1.0), text, color);
    }
    // A taken marker's red stays over the trim playhead at its frame (the log 2026-10-22: the
    // white line hid it).
    if input.layers.markers {
        let (range_start, duration) = (input.state.range_start(), input.state.duration_frames());
        for marker in input.markers.iter().filter(|marker| marker.taken) {
            paint_taken_marker(ui, track, taken_x(track, range_start, duration, marker), marker.part, input);
        }
    }
}

fn next_row(next_top: &mut f32, left: f32, right: f32, height: f32, gap: f32) -> Rect {
    let row = Rect::from_min_max(
        egui::pos2(left, *next_top),
        egui::pos2(right, *next_top + height),
    );
    *next_top += height + gap;
    row
}

fn keep_first_intent(current: &mut TimelineIntent, update: TimelineIntent) {
    if matches!(current, TimelineIntent::None) {
        *current = update;
    }
}

fn paint_audio_row(
    ui: &mut egui::Ui,
    row: Rect,
    lane: AudioLane,
    peaks: &[f32],
    fill: Color32,
    wave: Color32,
    input: &TimelineInput<'_>,
) -> TimelineIntent {
    let (label_rect, track_rect) = split_label_track(row, input.metrics.label_width);
    let label = if input.show_lane_labels {
        lane.label()
    } else {
        ""
    };
    paint_lane_label(ui, label_rect, label, input);
    paint_lane_button(ui, label_rect, lane, label, input);
    let response = ui.interact(
        track_rect,
        ui.make_persistent_id(("qnc_timeline_audio", lane.label())),
        Sense::click_and_drag(),
    );
    let label_response = ui.interact(
        label_rect,
        ui.make_persistent_id(("qnc_timeline_audio_label", lane.label())),
        Sense::click(),
    );
    let picker = match lane {
        AudioLane::A1 => input.a1_channel,
        AudioLane::A2 => input.a2_channel,
        _ => None,
    }
    .filter(|_| input.show_lane_labels);
    if input.show_lane_labels && label_response.clicked() {
        return label_intent(lane, picker.is_some(), input);
    }
    if let Some(take) = take_lane_intent(&[&label_response, &response], track_rect, lane, input) {
        return take;
    }
    if let Some(drag) = mark_drag_intent(&response, track_rect, input) {
        return drag;
    }
    ui.painter().rect_filled(track_rect, 0.0, fill); // no frame: the lane meets the filmstrip
    let gain = lane_gain(input, lane);
    qnc_wave_view::paint_wave_peaks_gain(ui.painter(), track_rect, peaks, gain, wave);
    qnc_wave_view::paint_level_marks(ui.painter(), track_rect, 1.0, input.theme.level_marks);
    paint_lane_level(ui, track_rect, lane, input);
    paint_ranges_and_playhead(ui, track_rect, input);
    if let Some(take) = take_or_keep_intent(&response, track_rect, input) {
        return take;
    }
    cue_intent_from_response(&response, track_rect, input)
}

/// The level a lane is heard at, at the start of its track, when it is not as recorded or
/// the lane is taken (live 2026-10-16: the user could not see the level); "OFF" when off.
fn paint_lane_level(ui: &mut egui::Ui, track: Rect, lane: AudioLane, input: &TimelineInput<'_>) {
    let choice = match lane {
        AudioLane::A1 => input.a1_channel,
        AudioLane::A2 => input.a2_channel,
        _ => None,
    };
    let Some(choice) = choice.filter(|choice| choice.open || choice.off || choice.heard_db != 0) else {
        return;
    };
    let text = if choice.off { "OFF".to_string() } else { format!("{:+} dB", choice.heard_db) };
    let font = FontId::proportional((track.height() - 4.0).clamp(9.0, 12.0));
    let galley = ui.painter().layout_no_wrap(text, font, input.theme.text);
    let at = egui::pos2(track.left() + 4.0, track.center().y - galley.size().y / 2.0);
    let back = Rect::from_min_size(at, galley.size()).expand2(Vec2::new(3.0, 1.0));
    ui.painter().rect_filled(back, 2.0, input.theme.label_background);
    ui.painter().galley(at, galley, input.theme.text);
}

/// A1/A2 are buttons (they take the lane): as bright as the text, in a frame. A lane that
/// is off is a red button; the lane being changed has a yellow rim (user 2026-10-15).
fn paint_lane_button(ui: &mut egui::Ui, label_rect: Rect, lane: AudioLane, label: &str, input: &TimelineInput<'_>) {
    if label.is_empty() {
        return;
    }
    let button = label_rect.shrink2(Vec2::new(3.0, 1.5));
    let choice = match lane {
        AudioLane::A1 => input.a1_channel,
        AudioLane::A2 => input.a2_channel,
        _ => None,
    };
    let silent = choice.is_some_and(|choice| choice.silent());
    let fill = if silent { input.theme.lane_silent } else { input.theme.label_background };
    ui.painter().rect_filled(button, 3.0, fill);
    let rim = if choice.is_some_and(|choice| choice.open) {
        Stroke::new(2.0, input.theme.lane_taken)
    } else {
        Stroke::new(1.0, input.theme.text)
    };
    ui.painter().rect_stroke(button, 3.0, rim, StrokeKind::Inside);
    ui.painter().text(button.center(), Align2::CENTER_CENTER, label, FontId::proportional(12.0), input.theme.text);
}

/// A1 or A2 in a dark band of the video row: its button in the label column and its wave
/// over the band (the video row under it keeps the clicks on the track, the markers and the
/// playhead).
fn paint_audio_band(ui: &mut egui::Ui, band: Rect, lane: AudioLane, peaks: &[f32], input: &TimelineInput<'_>) -> TimelineIntent {
    let (label_rect, track_rect) = split_label_track(band, input.metrics.label_width);
    let label = if input.show_lane_labels { lane.label() } else { "" };
    paint_lane_button(ui, label_rect, lane, label, input);
    let gain = lane_gain(input, lane);
    qnc_wave_view::paint_wave_peaks_gain(ui.painter(), track_rect, peaks, gain, input.theme.audio_wave);
    qnc_wave_view::paint_level_marks(ui.painter(), track_rect, 1.0, input.theme.level_marks);
    paint_lane_level(ui, track_rect, lane, input);
    let label_response = ui.interact(
        label_rect,
        ui.make_persistent_id(("qnc_timeline_audio_band_label", lane.label())),
        Sense::click(),
    );
    if input.show_lane_labels && label_response.clicked() {
        let picker = match lane {
            AudioLane::A1 => input.a1_channel,
            AudioLane::A2 => input.a2_channel,
            _ => None,
        };
        return label_intent(lane, picker.is_some(), input);
    }
    let band_response = ui.interact(track_rect, ui.make_persistent_id(("qnc_timeline_audio_band", lane.label())), Sense::click());
    if let Some(take) = take_lane_intent(&[&label_response, &band_response], track_rect, lane, input) {
        return take;
    }
    // A double click: on A1 the segment under it opens (Edit; A1 is the base of a slot, user
    // 2026-10-15), on A2 the slot shows enlarged (user 2026-10-21).
    if let Some(pos) = band_response.interact_pointer_pos().filter(|_| band_response.double_clicked() && input.layers.markers) {
        let frame = frame_for_x(track_rect, input.state.range_start(), input.state.duration_frames(), pos.x);
        let opened = match lane {
            AudioLane::A1 => input
                .virtual_spans
                .iter()
                .find(|span| (span.start_frame..span.end_frame).contains(&frame))
                .map(|span| TimelineIntent::OpenVirtual { id: span.id.to_string(), frame }),
            _ => slot_of_frame(input.marker_slots, frame).map(|id| TimelineIntent::OpenSlot { id: id.to_string(), frame }),
        };
        if let Some(opened) = opened {
            return opened;
        }
    }
    // A left click on the band is a click on the row: the playhead goes there, or a taken edit
    // is kept (audit 2026-10-17: the band took the click and nothing happened).
    match band_response.interact_pointer_pos().filter(|_| band_response.clicked() && input.state.can_cue()) {
        Some(pos) => {
            let frame = frame_for_x(track_rect, input.state.range_start(), input.state.duration_frames(), pos.x);
            if holding(input) {
                TimelineIntent::Keep(Some(frame))
            } else if input.layers.markers && band_response.ctx.input(|i| i.modifiers.shift) {
                TimelineIntent::AddMarkerAt(frame)
            } else {
                TimelineIntent::CueFrame(frame)
            }
        }
        None => TimelineIntent::None,
    }
}

/// A click on the A1/A2 label (a button) takes the lane as Ctrl+1/Ctrl+2 does (user rule
/// 2026-10-01): on a row with markers the sound under its playhead (else at its start), and
/// while something is taken there it keeps it (a left click keeps first). Without a channel
/// column on a source timeline the label opens the larger wave.
fn label_intent(lane: AudioLane, picker: bool, input: &TimelineInput<'_>) -> TimelineIntent {
    if !input.layers.markers {
        return if picker { TimelineIntent::LaneLabel { lane, frame: None } } else { TimelineIntent::ToggleAudioExpand(lane) };
    }
    if input.holding {
        return TimelineIntent::Keep(None);
    }
    let (start, duration) = (input.state.range_start(), input.state.duration_frames());
    let frame = input.state.playhead_frame.filter(|frame| (start..start + duration).contains(frame)).unwrap_or(start);
    TimelineIntent::LaneLabel { lane, frame: Some(frame) }
}

/// A right click on A1/A2 (its label or its track) takes the lane, as Ctrl+1/Ctrl+2: on a
/// Wrap row (markers) the sound of what lies at the clicked frame.
fn take_lane_intent(responses: &[&egui::Response], track: Rect, lane: AudioLane, input: &TimelineInput<'_>) -> Option<TimelineIntent> {
    let clicked = responses.iter().find(|response| right_click(response))?;
    if !matches!(lane, AudioLane::A1 | AudioLane::A2) {
        return None;
    }
    if input.layers.markers {
        let pos = clicked.interact_pointer_pos()?;
        let (start, duration) = (input.state.range_start(), input.state.duration_frames());
        // Over A2 near the taken M marker: only its sound (user 2026-10-22); A1 has no cut at a marker.
        if matches!(lane, AudioLane::A2) {
            if let Some(id) = taken_marker_for_right_click(track, start, duration, pos, input.markers) {
                return Some(TimelineIntent::TakeMarkerSound { id: id.to_string() });
            }
        }
        let frame = frame_for_x(track, start, duration, pos.x);
        return Some(TimelineIntent::TakeSound { lane, frame });
    }
    Some(TimelineIntent::TakeAudioLane(lane))
}

/// A left press on IN or OUT of a source timeline (within a few pixels; the clip's start
/// and end when the user set none) drags that mark: the timeline reports where the pointer
/// is (`DragMark`) and, when the button is let go, keeps it (`KeepMark`). It moves nothing
/// itself. `None`: no mark drag.
fn mark_drag_intent(response: &egui::Response, track: Rect, input: &TimelineInput<'_>) -> Option<TimelineIntent> {
    if input.layers.markers || !(input.draft_in_active || input.draft_out_active) {
        return None;
    }
    let key = response.id.with("qnc_timeline_mark_drag");
    let (range_start, duration) = (input.state.range_start(), input.state.duration_frames());
    if response.drag_started_by(egui::PointerButton::Primary) {
        let at = response.ctx.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos())?;
        // The zone of the M markers (user 2026-10-18: the same on the source timeline).
        let (in_grab, out_grab) = grabbable_marks(input);
        let marks = ((input.draft_in_frame, in_grab), (input.draft_out_frame, out_grab));
        if let Some(in_mark) = source_mark_in_reach(track, range_start, duration, at, marks.0, marks.1) {
            response.ctx.data_mut(|data| data.insert_temp(key, in_mark));
        }
    }
    let in_mark = response.ctx.data(|data| data.get_temp::<bool>(key))?;
    if response.dragged() || response.is_pointer_button_down_on() {
        let x = response.interact_pointer_pos()?.x;
        let frame = frame_for_x(track, range_start, duration, x).saturating_sub(range_start);
        return Some(TimelineIntent::DragMark { in_mark, frame });
    }
    response.ctx.data_mut(|data| data.remove::<bool>(key));
    Some(TimelineIntent::Keep(None))
}

/// Which of IN and OUT the mouse may drag: only the taken one (Ctrl+I/O, a right click,
/// Ctrl+wheel; user rule 2026-10-20: a drag moves only what is taken).
fn grabbable_marks(input: &TimelineInput<'_>) -> (bool, bool) {
    (
        input.draft_in_active && input.focus == TimelineFocusPaint::In,
        input.draft_out_active && input.focus == TimelineFocusPaint::Out,
    )
}

/// A right click (Ctrl+), unless the right button was held for the wheel (then it was Ctrl
/// for the wheel, not a click).
fn right_click(response: &egui::Response) -> bool {
    response.secondary_clicked() && !qnc_wheel_jog::wheeled_with_right(&response.ctx)
}

/// A1 or A2 is taken (its column is open): a left click on the timeline keeps it, as on a
/// Wrap row (user 2026-10-17: the source timeline did not).
fn lane_taken(input: &TimelineInput<'_>) -> bool {
    input.a1_channel.is_some_and(|choice| choice.open) || input.a2_channel.is_some_and(|choice| choice.open)
}

/// Whether something is taken on this timeline: IN/OUT or A1/A2 of a source timeline, what
/// the host holds on a row with markers.
fn holding(input: &TimelineInput<'_>) -> bool {
    if input.layers.markers {
        input.holding
    } else {
        input.focus != TimelineFocusPaint::Playhead || lane_taken(input)
    }
}

/// A click on the track (user rule 2026-10-20, the mouse map `qnc-mouse-map`): a left click
/// while something is taken keeps it (as Enter) and the playhead goes there; a right click
/// takes what it points at (as Ctrl+): the nearer IN or OUT on the source timeline; on a row
/// with markers the slot on a colour strip, else the nearest M marker (never the locked start
/// or end). `None`: neither.
fn take_or_keep_intent(response: &egui::Response, track: Rect, input: &TimelineInput<'_>) -> Option<TimelineIntent> {
    let (range_start, duration) = (input.state.range_start(), input.state.duration_frames());
    // While IN/OUT or A1/A2 is taken, a press of the left button keeps it, a quick click or a
    // slow one alike (live 2026-10-17: a press held a moment was a drag and moved the playhead,
    // the mark stayed taken); until that button is let go the press moves nothing more.
    let kept_key = response.id.with("qnc_timeline_kept_press");
    let kept_press = response.ctx.data(|data| data.get_temp::<bool>(kept_key)).unwrap_or(false);
    if kept_press {
        if response.is_pointer_button_down_on() || response.dragged() {
            return Some(TimelineIntent::None);
        }
        response.ctx.data_mut(|data| data.remove::<bool>(kept_key));
        if response.clicked() || response.drag_stopped() {
            return Some(TimelineIntent::None); // the release of the press that kept
        }
    }
    // A left click on a cover's strip in the A2 zoom takes that cover's sound (user
    // 2026-10-22), the one taken before is kept first by its owner.
    if response.clicked() {
        if let (Some(rows), Some(pos)) = (zoom_strip_rows(track, input), response.interact_pointer_pos()) {
            if cover_hit(track, range_start, duration, pos, input.covers, rows).is_some() {
                return Some(TimelineIntent::TakeSound { lane: AudioLane::A2, frame: frame_for_x(track, range_start, duration, pos.x) });
            }
        }
    }
    if holding(input) && (response.clicked() || response.drag_started_by(egui::PointerButton::Primary)) {
        if !response.clicked() {
            response.ctx.data_mut(|data| data.insert_temp(kept_key, true));
        }
        let at = response.ctx.input(|i| i.pointer.press_origin()).or(response.interact_pointer_pos());
        let frame = at.filter(|_| input.state.can_cue()).map(|pos| frame_for_x(track, range_start, duration, pos.x));
        return Some(TimelineIntent::Keep(frame));
    }
    if !right_click(response) {
        return None;
    }
    let pos = response.interact_pointer_pos()?;
    let frame = frame_for_x(track, range_start, duration, pos.x);
    if input.layers.markers {
        let cover = input.layers.covers.then(|| cover_hit(track, range_start, duration, pos, input.covers, cover_rows(track, input))).flatten();
        // On a colour strip a right click takes its slot (as Ctrl+S); a cover in no slot is taken.
        if let Some(cover) = cover {
            return Some(match slot_of_frame(input.marker_slots, frame) {
                Some(slot) => TimelineIntent::TakeSlot { id: slot.to_string(), frame },
                None => TimelineIntent::TakeCover { id: cover.to_string(), frame },
            });
        }
        // Near the taken M marker only its picture (user 2026-10-22: a second right click).
        if let Some(id) = taken_marker_for_right_click(track, range_start, duration, pos, input.markers) {
            return Some(TimelineIntent::TakeMarkerPicture { id: id.to_string() });
        }
        // Elsewhere the nearest M marker, whole (as Ctrl+M; M is OUT and IN in one mark).
        return marker_for_right_click(track, range_start, duration, pos, input.markers, false).map(|id| TimelineIntent::TakeMarker { id: id.to_string(), frame });
    }
    if !(input.draft_in_active || input.draft_out_active) {
        return None;
    }
    let local = frame.saturating_sub(range_start);
    Some(TimelineIntent::TakeMark(local.abs_diff(input.draft_in_frame) <= local.abs_diff(input.draft_out_frame)))
}

/// The insets of the light middle of the video row: the dark band above it and the dark
/// part under it (a band as high with audio bands).
/// Whether a lane's zoom is drawn over the video row (a taken lane open, or a wave over video).
fn lane_zoomed(input: &TimelineInput<'_>) -> bool {
    zoomed_lane(input).is_some()
}

/// The lane whose zoom is drawn over the video row.
fn zoomed_lane(input: &TimelineInput<'_>) -> Option<AudioLane> {
    let open = |choice: Option<ChannelChoice>| choice.is_some_and(|c| c.open);
    if input.layers.audio_a1 && open(input.a1_channel) {
        return Some(AudioLane::A1);
    }
    if input.layers.audio_a2 && open(input.a2_channel) {
        return Some(AudioLane::A2);
    }
    input.wave_over_video.map(|(lane, _)| lane)
}

/// The slot field of the A2 zoom on a Wrap row (user 2026-10-22: the sound of one cover is set
/// by a click on its strip): a strip at the bottom of the zoom over the picture's place.
fn zoom_strip_rows(track: Rect, input: &TimelineInput<'_>) -> Option<(f32, f32)> {
    (input.layers.audio_bands && input.layers.covers && zoomed_lane(input) == Some(AudioLane::A2)).then(|| {
        let bottom = track.bottom() - video_middle_insets(input).1;
        (bottom - COVER_STRIP, bottom)
    })
}

/// The covers' strips over the A2 zoom, always shown; the taken one framed.
fn paint_zoom_strips(ui: &mut egui::Ui, track: Rect, input: &TimelineInput<'_>) {
    let Some((top, bottom)) = zoom_strip_rows(track, input) else {
        return;
    };
    let (range_start, duration) = (input.state.range_start(), input.state.duration_frames());
    for cover in input.covers.iter().filter(|cover| cover.end_frame > cover.start_frame && !cover.pending) {
        let x0 = x_for_frame(track, range_start, duration, cover.start_frame);
        let x1 = x_for_frame(track, range_start, duration, cover.end_frame);
        let rect = Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x1.max(x0 + 3.0), bottom));
        let fill = cover.fill.unwrap_or(input.theme.shot_range);
        ui.painter().rect_filled(rect, 1.0, fill.linear_multiply(if cover.selected { 0.9 } else { 0.55 }));
        if cover.selected {
            ui.painter().rect_stroke(rect, 1.0, Stroke::new(2.0, input.theme.marker_hover), StrokeKind::Inside);
        }
        if let Some(label) = cover.label.filter(|_| rect.width() >= 34.0) {
            ui.painter().text(rect.center(), Align2::CENTER_CENTER, label, FontId::proportional(10.0), Color32::BLACK);
        }
    }
}

fn video_middle_insets(input: &TimelineInput<'_>) -> (f32, f32) {
    if input.layers.audio_bands {
        let band = input.metrics.audio_height;
        (if input.layers.audio_a1 { band } else { 0.0 }, if input.layers.audio_a2 { band } else { 0.0 })
    } else {
        (VIDEO_BAND, VIDEO_FOOT)
    }
}

fn paint_video_row(ui: &mut egui::Ui, row: Rect, input: &TimelineInput<'_>) -> TimelineIntent {
    let (label_rect, track_rect) = split_label_track(row, input.metrics.label_width);
    let label = if input.show_lane_labels { "V" } else { "" };
    paint_lane_label(ui, label_rect, label, input);
    // A click on V is Home: the playhead goes to the start (user rule 2026-10-01).
    let label_response = ui.interact(
        label_rect,
        ui.make_persistent_id("qnc_timeline_video_label"),
        Sense::click(),
    );
    if input.show_lane_labels && label_response.clicked() {
        if input.layers.audio_bands && ui.input(|input| input.modifiers.command) {
            return TimelineIntent::ToggleVideoMode;
        }
        return TimelineIntent::GoHome;
    }
    // A right click is Ctrl+ (user 2026-10-17): on V it is Ctrl+click.
    if input.show_lane_labels && input.layers.audio_bands && right_click(&label_response) {
        return TimelineIntent::ToggleVideoMode;
    }
    let response = ui.interact(
        track_rect,
        ui.make_persistent_id("qnc_timeline_video"),
        Sense::click_and_drag(),
    );
    ui.painter()
        .rect_filled(track_rect, 0.0, input.theme.video_background);
    if input.layers.audio_bands {
        // A1 and A2 bands in the colour of an audio lane, both the same (user 2026-10-15);
        // the slot and the covers are drawn over them afterwards.
        let (top, bottom) = video_middle_insets(input);
        let fill = input.theme.audio_primary_background;
        if top > 0.0 {
            ui.painter().rect_filled(Rect::from_min_max(track_rect.min, egui::pos2(track_rect.right(), track_rect.top() + top)), 0.0, fill);
        }
        if bottom > 0.0 {
            ui.painter().rect_filled(Rect::from_min_max(egui::pos2(track_rect.left(), track_rect.bottom() - bottom), track_rect.max), 0.0, fill);
        }
    }
    if let Some(background) = input.filmstrip_background {
        paint_filmstrip_background(ui, track_rect, background);
    }
    if let Some(paint_background) = input.video_background {
        paint_background(ui, track_rect);
    }
    paint_video_layers(ui, track_rect, input);
    video_intent_from_response(&response, track_rect, input)
}

fn paint_filmstrip_background(ui: &mut egui::Ui, track: Rect, background: &FilmstripBackground) {
    if background.frames.is_empty() || track.width() <= 1.0 || track.height() <= 1.0 {
        return;
    }
    let slot_count = filmstrip_slot_count(background);
    let slot_width = track.width() / slot_count as f32;
    for frame in &background.frames {
        let slot_index = frame.index.min(slot_count.saturating_sub(1));
        let left = track.left() + slot_index as f32 * slot_width;
        let slot = Rect::from_min_max(
            egui::pos2(left, track.top()),
            egui::pos2((left + slot_width).min(track.right()), track.bottom()),
        );
        let _ = qnc_ui_kit::paint_rgba_image(
            ui,
            slot,
            &frame.uri,
            frame.image.content_key,
            frame.image.size,
            &frame.image.pixels,
        );
    }
}

fn filmstrip_slot_count(background: &FilmstripBackground) -> usize {
    filmstrip_slot_count_from_indices(background.frames.iter().map(|frame| frame.index))
}

fn filmstrip_slot_count_from_indices(indices: impl IntoIterator<Item = usize>) -> usize {
    indices
        .into_iter()
        .map(|index| index + 1)
        .max()
        .unwrap_or(0)
        .max(FILMSTRIP_FRAME_COUNT)
        .max(1)
}

fn paint_lane_label(ui: &mut egui::Ui, rect: Rect, label: &str, input: &TimelineInput<'_>) {
    ui.painter()
        .rect_filled(rect, 0.0, input.theme.label_background);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        input.theme.muted,
    );
}

fn paint_ranges_and_playhead(ui: &mut egui::Ui, track: Rect, input: &TimelineInput<'_>) {
    let duration = input.state.duration_frames();
    if input.layers.shot_range {
        paint_range_outline(
            ui,
            track,
            duration,
            input.shot_in_frame,
            input.shot_out_frame,
            input.theme.shot_range,
        );
    }
    if input.layers.in_out {
        if input.draft_in_active || input.draft_out_active {
            paint_in_out_dim(
                ui,
                track,
                duration,
                input.draft_in_frame,
                input.draft_out_frame,
                input.theme.in_out_dim,
            );
        }
        paint_in_out_handles(
            ui,
            track,
            duration,
            input.draft_in_frame,
            input.draft_out_frame,
            input,
        );
    }
    if input.layers.playhead {
        if let Some(frame) = input.state.confirmed_frame() {
            paint_playhead(ui, track, input.state.range_start(), duration, frame, input);
        }
    }
}

fn paint_video_layers(ui: &mut egui::Ui, track: Rect, input: &TimelineInput<'_>) {
    let duration = input.state.duration_frames();
    let range_start = input.state.range_start();

    if input.layers.covers && input.layers.marker_slots {
        paint_marker_slots(
            ui,
            track,
            range_start,
            duration,
            input.marker_slots,
            input,
            false,
        );
    }
    if input.layers.base_video {
        if input.virtual_spans.is_empty() {
            if !input.base_video_blank {
                let (top, bottom) = video_middle_insets(input);
                ui.painter().rect_filled(
                    Rect::from_min_max(track.min + Vec2::new(0.0, top), track.max - Vec2::new(0.0, bottom)),
                    1.0,
                    input.theme.wave_a1.linear_multiply(0.20),
                );
            }
        } else if input.layers.virtual_spans {
            paint_virtual_spans(ui, track, range_start, duration, input.virtual_spans, input);
        }
    } else if input.layers.virtual_spans {
        if input.virtual_spans.is_empty() && !input.base_video_blank {
            let (top, bottom) = video_middle_insets(input);
            ui.painter().rect_filled(
                Rect::from_min_max(track.min + Vec2::new(0.0, top), track.max - Vec2::new(0.0, bottom)),
                1.0,
                input.theme.wave_a1.linear_multiply(0.20),
            );
        } else {
            paint_virtual_spans(ui, track, range_start, duration, input.virtual_spans, input);
        }
    }
    paint_pictures(ui, track, range_start, duration, input);
    if input.layers.shot_range {
        paint_range_outline(
            ui,
            track,
            duration,
            input.shot_in_frame,
            input.shot_out_frame,
            input.theme.shot_range,
        );
    }
    if input.layers.covers {
        paint_covers(ui, track, range_start, duration, input.covers, input);
        if input.layers.marker_slots {
            paint_marker_slots(
                ui,
                track,
                range_start,
                duration,
                input.marker_slots,
                input,
                true,
            );
        }
        // The markers are the layer over the picture or a lane's zoom (user 2026-10-22: under
        // them either the filmstrip or the sound): with a zoom they are drawn over it.
        if input.layers.markers && !lane_zoomed(input) {
            paint_markers(ui, track, range_start, duration, input.markers, input);
            paint_a1_splits(ui, track, range_start, duration, input);
        }
    }
    if input.layers.in_out {
        if input.draft_in_active || input.draft_out_active {
            paint_in_out_dim(
                ui,
                track,
                duration,
                input.draft_in_frame,
                input.draft_out_frame,
                input.theme.in_out_dim,
            );
        }
        paint_in_out_handles(
            ui,
            track,
            duration,
            input.draft_in_frame,
            input.draft_out_frame,
            input,
        );
    }
    if input.layers.playhead && !input.layers.audio_bands {
        if let Some(frame) = input.state.confirmed_frame() {
            paint_playhead(ui, track, range_start, duration, frame, input);
        }
    }
}

fn paint_playhead(
    ui: &mut egui::Ui,
    track: Rect,
    range_start: u64,
    duration: u64,
    frame: u64,
    input: &TimelineInput<'_>,
) {
    let x = x_for_frame(track, range_start, duration, frame);
    let color = if input.focus == TimelineFocusPaint::Playhead {
        input.theme.focus
    } else {
        input.theme.playhead
    };
    let width = if input.focus == TimelineFocusPaint::Playhead {
        2.5
    } else {
        1.5
    };
    ui.painter().line_segment(
        [egui::pos2(x, track.top()), egui::pos2(x, track.bottom())],
        Stroke::new(width, color),
    );
}

fn paint_in_out_handles(
    ui: &mut egui::Ui,
    track: Rect,
    duration: u64,
    in_frame: u64,
    out_frame: u64,
    input: &TimelineInput<'_>,
) {
    let in_x = x_for_local_frame(track, duration, in_frame);
    let out_x = x_for_local_frame(track, duration, out_frame);
    // IN or OUT in the mouse's reach is red, as an M marker (user 2026-10-18).
    let range_start = input.state.range_start();
    let hovered = ui
        .ctx()
        .pointer_hover_pos()
        .filter(|pos| track.y_range().contains(pos.y))
        .and_then(|pos| {
            let (in_grab, out_grab) = grabbable_marks(input);
            source_mark_in_reach(track, range_start, duration, pos, (in_frame, in_grab), (out_frame, out_grab))
        });
    let stroke = |is_in: bool, focus: TimelineFocusPaint| {
        if hovered == Some(is_in) {
            Stroke::new(3.0, input.theme.marker_hover)
        } else if input.focus == focus {
            Stroke::new(3.0, input.theme.focus)
        } else {
            Stroke::new(2.0, input.theme.text)
        }
    };
    let (in_stroke, out_stroke) = (stroke(true, TimelineFocusPaint::In), stroke(false, TimelineFocusPaint::Out));
    if input.draft_in_active {
        ui.painter().line_segment(
            [
                egui::pos2(in_x, track.top()),
                egui::pos2(in_x, track.bottom()),
            ],
            in_stroke,
        );
    }
    if input.draft_out_active {
        ui.painter().line_segment(
            [
                egui::pos2(out_x, track.top()),
                egui::pos2(out_x, track.bottom()),
            ],
            out_stroke,
        );
    }
}

fn paint_virtual_spans(
    ui: &mut egui::Ui,
    track: Rect,
    range_start: u64,
    duration: u64,
    spans: &[TimelineVirtualSpan<'_>],
    input: &TimelineInput<'_>,
) {
    for span in spans {
        if span.end_frame <= span.start_frame {
            continue;
        }
        let x0 = x_for_frame(track, range_start, duration, span.start_frame);
        let x1 = x_for_frame(track, range_start, duration, span.end_frame);
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + video_middle_insets(input).0),
            egui::pos2(x1.max(x0 + 3.0), track.bottom() - video_middle_insets(input).1),
        );
        let base = if span.has_base_video {
            input.theme.wave_a1
        } else {
            input.theme.wave_a2
        };
        ui.painter()
            .rect_filled(rect, 1.0, base.linear_multiply(0.24));
        ui.painter().rect_stroke(
            rect,
            1.0,
            Stroke::new(
                if span.selected { 1.8 } else { 1.0 },
                if span.selected {
                    input.theme.focus
                } else {
                    base
                },
            ),
            StrokeKind::Inside,
        );
        if rect.width() >= 42.0 && !span.label.trim().is_empty() {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                span.label,
                FontId::proportional(10.0),
                input.theme.text,
            );
        }
    }
}

/// The pictures of one segment or cover: neighbours from the same filmstrip that touch.
fn picture_runs<'p, 'a>(pictures: &'p [TimelinePicture<'a>]) -> Vec<&'p [TimelinePicture<'a>]> {
    let mut runs = Vec::new();
    let mut first = 0;
    for index in 1..=pictures.len() {
        let breaks = index == pictures.len() || {
            let (before, here) = (&pictures[index - 1], &pictures[index]);
            here.start_frame > before.end_frame || here.start_frame < before.start_frame || !std::ptr::eq(here.strip, before.strip)
        };
        if breaks {
            runs.push(&pictures[first..index]);
            first = index;
        }
    }
    runs
}

/// The filmstrip pictures in the light middle of a banded row, with the outline of each
/// segment over them.
fn paint_pictures(ui: &mut egui::Ui, track: Rect, range_start: u64, duration: u64, input: &TimelineInput<'_>) {
    if input.pictures.is_empty() || !input.layers.audio_bands {
        return;
    }
    let (top, bottom) = video_middle_insets(input);
    let (top, bottom) = (track.top() + top, track.bottom() - bottom);
    // A segment or cover shows only as many pictures as fit at their aspect (user
    // 2026-10-15), each filling its cell: the picture of the clip at the cell's middle.
    for run in picture_runs(input.pictures) {
        let (first, last) = (&run[0], &run[run.len() - 1]);
        let x0 = x_for_frame(track, range_start, duration, first.start_frame);
        let x1 = x_for_frame(track, range_start, duration, last.end_frame);
        let image = &first.frame.image;
        if x1 <= x0 || image.size[1] == 0 {
            continue;
        }
        let rect = Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x1, bottom));
        let tile = rect.height() * image.size[0] as f32 / image.size[1] as f32;
        let cells = ((rect.width() / tile.max(1.0)).floor() as usize).max(1);
        let width = rect.width() / cells as f32;
        let span = (last.end_frame - first.start_frame) as f32;
        for cell in 0..cells {
            let cell_rect = Rect::from_min_size(egui::pos2(rect.left() + cell as f32 * width, top), Vec2::new(width, rect.height()));
            let middle = first.start_frame + (span * (cell as f32 + 0.5) / cells as f32) as u64;
            let picture = run.iter().find(|p| middle < p.end_frame).unwrap_or(last);
            let image = &picture.frame.image;
            let _ = qnc_ui_kit::paint_rgba_image_filled(ui, cell_rect, &picture.frame.uri, image.content_key, image.size, &image.pixels);
        }
    }
    for span in input.virtual_spans.iter().filter(|span| span.end_frame > span.start_frame) {
        let x0 = x_for_frame(track, range_start, duration, span.start_frame);
        let x1 = x_for_frame(track, range_start, duration, span.end_frame);
        let rect = Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x1.max(x0 + 3.0), bottom));
        let color = if span.selected { input.theme.focus } else { input.theme.border };
        ui.painter().rect_stroke(rect, 1.0, Stroke::new(if span.selected { 1.8 } else { 1.0 }, color), StrokeKind::Inside);
    }
}

fn paint_covers(
    ui: &mut egui::Ui,
    track: Rect,
    range_start: u64,
    duration: u64,
    covers: &[TimelineCoverSpan<'_>],
    input: &TimelineInput<'_>,
) {
    for cover in covers {
        if cover.end_frame <= cover.start_frame {
            continue;
        }
        let x0 = x_for_frame(track, range_start, duration, cover.start_frame);
        let x1 = x_for_frame(track, range_start, duration, cover.end_frame);
        let (top, bottom) = cover_rows(track, input);
        let rect = Rect::from_min_max(egui::pos2(x0, top), egui::pos2(x1.max(x0 + 3.0), bottom));
        // Over the filmstrip the strip shows only under the mouse or when the cover is chosen
        // (user 2026-10-17); the narrow video row keeps it, it has no picture of its own.
        let strip = cover_strip(input);
        if strip && !cover.selected && !cover.pending && !ui.rect_contains_pointer(rect) {
            continue;
        }
        let base = match (cover.pending, cover.fill) {
            (_, Some(fill)) => fill,
            (true, None) => input.theme.focus,
            (false, None) => input.theme.shot_range,
        };
        let fill = if cover.selected {
            base.linear_multiply(0.85)
        } else {
            base.linear_multiply(0.55)
        };
        ui.painter().rect_filled(rect, 1.0, fill);
        if cover.pending {
            // Not saved yet: a frame in the focus colour round it.
            ui.painter().rect_stroke(rect, 1.0, Stroke::new(2.0, input.theme.focus), StrokeKind::Inside);
        }
        if let Some(label) = cover.label.filter(|_| rect.width() >= 34.0) {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                Color32::BLACK,
            );
        }
        if cover.pending && cover.label.is_none() && rect.width() >= 56.0 {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                qnc_ui_text::text("story.ui.not_in_program"),
                FontId::proportional(10.0),
                input.theme.text,
            );
        }
    }
}

fn paint_marker_slots(
    ui: &mut egui::Ui,
    track: Rect,
    range_start: u64,
    duration: u64,
    slots: &[TimelineSlotSpan<'_>],
    input: &TimelineInput<'_>,
    selected_only: bool,
) {
    for slot in slots {
        if slot.end_frame <= slot.start_frame || selected_only && !slot.selected {
            continue;
        }
        let x0 = x_for_frame(track, range_start, duration, slot.start_frame);
        let x1 = x_for_frame(track, range_start, duration, slot.end_frame);
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + slot_top(input) + 2.0),
            egui::pos2(x1.max(x0 + 3.0), track.bottom() - 2.0),
        );
        let base = if slot.selected {
            input.theme.focus
        } else if slot.has_cover {
            input.theme.shot_range
        } else {
            input.theme.playhead
        };
        // With audio bands the slot fills only the video part and frames A2, so A1 and
        // A2 keep the same colour (user 2026-10-15).
        let fill = if input.layers.audio_bands {
            Rect::from_min_max(rect.min, egui::pos2(rect.right(), track.bottom() - video_middle_insets(input).1))
        } else {
            rect
        };
        // Over the filmstrip the selected slot is only framed, its pictures stay clear.
        if !(selected_only && !input.pictures.is_empty()) {
            ui.painter().rect_filled(
                fill,
                1.0,
                base.linear_multiply(if selected_only { 0.28 } else { 0.18 }),
            );
        }
        ui.painter().rect_stroke(
            rect,
            1.0,
            Stroke::new(if slot.selected { 1.8 } else { 1.0 }, base),
            StrokeKind::Inside,
        );
    }
}

fn paint_markers(
    ui: &mut egui::Ui,
    track: Rect,
    range_start: u64,
    duration: u64,
    markers: &[TimelineMarkerPin<'_>],
    input: &TimelineInput<'_>,
) {
    // The marker in the mouse's reach (the same reach a click or drag takes it from) is red.
    let hovered = ui
        .ctx()
        .pointer_hover_pos()
        .filter(|pos| track.y_range().contains(pos.y))
        .and_then(|pos| marker_to_drag(track, range_start, duration, pos, markers));
    for marker in markers {
        if marker.id.trim().is_empty() {
            continue;
        }
        let x = x_for_frame(track, range_start, duration, marker.frame);
        let (width, color) = if hovered == Some(marker.id) { (2.0, input.theme.marker_hover) } else { (1.0, input.theme.focus) };
        // With audio bands a marker belongs to the covers: its line runs over the video and
        // A2, never over A1, and it has no "M" over A1's wave (user 2026-10-15).
        let top = track.top() + slot_top(input);
        if marker.taken {
            // The original stays white where it is stored; the red is what moves.
            if let Some(origin) = marker.origin.map(|frame| x_for_frame(track, range_start, duration, frame)) {
                ui.painter().line_segment([egui::pos2(origin, top), egui::pos2(origin, track.bottom())], Stroke::new(1.5, input.theme.text));
            }
            paint_taken_marker(ui, track, taken_x(track, range_start, duration, marker), marker.part, input);
            continue;
        }
        // A cut whose A2 sound is away from its picture (a J or an L): the line over the
        // picture, the sound's cut red on A2 (user 2026-10-22).
        let bottom = if marker.sound_at.is_some() { track.bottom() - video_middle_insets(input).1 } else { track.bottom() };
        // A split cut is red in both parts (user 2026-10-22); a whole one keeps its colour.
        let (width, color) = if marker.sound_at.is_some() { (2.0, input.theme.marker_hover) } else { (width, color) };
        ui.painter().line_segment([egui::pos2(x, top), egui::pos2(x, bottom)], Stroke::new(width, color));
        if let Some(sound) = marker.sound_at {
            let sound = x_for_frame(track, range_start, duration, sound);
            ui.painter().line_segment([egui::pos2(sound, bottom), egui::pos2(sound, track.bottom())], Stroke::new(2.0, input.theme.marker_hover));
            ui.painter().line_segment([egui::pos2(x.min(sound), bottom), egui::pos2(x.max(sound), bottom)], Stroke::new(1.0, input.theme.marker_hover));
        }
        if !input.layers.audio_bands {
            ui.painter().text(
                egui::pos2(x + 3.0, track.top() + 2.0),
                Align2::LEFT_TOP,
                "M",
                FontId::proportional(9.0),
                color,
            );
        }
    }
}

/// The A1 cuts split in two (the picture J of a Ton segment): the picture's cut red over the
/// picture, the sound's red over A1, joined at the edge between them (user 2026-10-22).
fn paint_a1_splits(ui: &mut egui::Ui, track: Rect, range_start: u64, duration: u64, input: &TimelineInput<'_>) {
    let edge = track.top() + slot_top(input);
    let picture_bottom = track.bottom() - video_middle_insets(input).1;
    let stroke = Stroke::new(2.0, input.theme.marker_hover);
    for &(picture, sound) in input.a1_splits {
        let (picture, sound) = (x_for_frame(track, range_start, duration, picture), x_for_frame(track, range_start, duration, sound));
        ui.painter().line_segment([egui::pos2(picture, edge), egui::pos2(picture, picture_bottom)], stroke);
        ui.painter().line_segment([egui::pos2(sound, track.top()), egui::pos2(sound, edge)], stroke);
        ui.painter().line_segment([egui::pos2(picture.min(sound), edge), egui::pos2(picture.max(sound), edge)], Stroke::new(1.0, input.theme.marker_hover));
    }
}

/// Where the moving part of a taken marker is: its sound's cut when the sound is taken, else
/// the marker's draft.
fn taken_x(track: Rect, range_start: u64, duration: u64, marker: &TimelineMarkerPin<'_>) -> f32 {
    let frame = match marker.part {
        TimelineMarkerPart::Sound => marker.sound_at.unwrap_or(marker.frame),
        _ => marker.frame,
    };
    x_for_frame(track, range_start, duration, frame)
}

/// A taken marker is red (user 2026-10-22): whole, or only the part that moves (its picture
/// or its A2 sound); thicker than a trim playhead so it shows over one at the same frame.
fn paint_taken_marker(ui: &mut egui::Ui, track: Rect, x: f32, part: TimelineMarkerPart, input: &TimelineInput<'_>) {
    let top = track.top() + slot_top(input);
    let sound_top = track.bottom() - video_middle_insets(input).1;
    let (from, to) = match part {
        TimelineMarkerPart::Whole => (top, track.bottom()),
        TimelineMarkerPart::Picture => (top, sound_top),
        TimelineMarkerPart::Sound => (sound_top, track.bottom()),
    };
    ui.painter().line_segment([egui::pos2(x, from), egui::pos2(x, to)], Stroke::new(3.0, input.theme.marker_hover));
}

fn paint_range_outline(
    ui: &mut egui::Ui,
    track: Rect,
    duration: u64,
    start_frame: u64,
    end_frame: u64,
    color: Color32,
) {
    if end_frame <= start_frame {
        return;
    }
    let x0 = x_for_local_frame(track, duration, start_frame);
    let x1 = x_for_local_frame(track, duration, end_frame);
    let rect = Rect::from_min_max(
        egui::pos2(x0, track.top() + 1.0),
        egui::pos2(x1.max(x0 + 3.0), track.bottom() - 1.0),
    );
    ui.painter()
        .rect_stroke(rect, 1.0, Stroke::new(1.2, color), StrokeKind::Inside);
}

fn paint_in_out_dim(
    ui: &mut egui::Ui,
    track: Rect,
    duration: u64,
    in_frame: u64,
    out_frame: u64,
    dim: Color32,
) {
    let start = in_frame.min(duration);
    let end = out_frame.max(start).min(duration);
    let x_in = x_for_local_frame(track, duration, start);
    let x_out = x_for_local_frame(track, duration, end);
    if x_in > track.left() + 0.5 {
        ui.painter().rect_filled(
            Rect::from_min_max(track.left_top(), egui::pos2(x_in, track.bottom())),
            0.0,
            dim,
        );
    }
    if x_out < track.right() - 0.5 {
        ui.painter().rect_filled(
            Rect::from_min_max(egui::pos2(x_out, track.top()), track.right_bottom()),
            0.0,
            dim,
        );
    }
}

fn cue_intent_from_response(
    response: &egui::Response,
    track: Rect,
    input: &TimelineInput<'_>,
) -> TimelineIntent {
    if !(response.clicked() || response.dragged()) || !input.state.can_cue() {
        return TimelineIntent::None;
    }
    let Some(pos) = response.interact_pointer_pos() else {
        return TimelineIntent::None;
    };
    if !track.expand(2.0).contains(pos) {
        return TimelineIntent::None;
    }
    TimelineIntent::CueFrame(frame_for_x(
        track,
        input.state.range_start(),
        input.state.duration_frames(),
        pos.x,
    ))
}

const MARKER_DRAG: &str = "qnc_timeline_marker_drag";

/// A drag that started on an M marker of a row with markers (Story segment rows,
/// never the source timeline): the marker follows the pointer as a draft. The
/// timeline only reports it (`take_marker_drag`); it moves nothing and writes
/// nothing. Returns whether such a drag runs, so the row does not scrub.
fn track_marker_drag(response: &egui::Response, track: Rect, input: &TimelineInput<'_>) -> bool {
    if !(input.layers.covers && input.layers.markers) {
        return false;
    }
    let (ctx, start, duration) = (
        &response.ctx,
        input.state.range_start(),
        input.state.duration_frames(),
    );
    let active = egui::Id::new(MARKER_DRAG).with(response.id);
    if response.drag_started() {
        let press = ctx.input(|i| i.pointer.press_origin());
        let hit = press.and_then(|pos| marker_to_drag(track, start, duration, pos, input.markers)).map(str::to_string);
        // Every drag on a row with markers in the player log: which marker was nearest and
        // how far (live 2026-10-18: a marker drag that scrubbed instead could not be told).
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            let nearest = press.and_then(|pos| marker_reach::nearest(track, start, duration, pos, input.markers));
            qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("timeline-drag start hit={hit:?} nearest={nearest:?}"));
        }
        ctx.data_mut(|d| match hit {
            Some(id) => d.insert_temp(active, id),
            None => d.remove::<String>(active),
        });
    }
    let Some(id) = ctx.data(|d| d.get_temp::<String>(active)) else {
        return false;
    };
    if let Some(pos) = response
        .interact_pointer_pos()
        .filter(|_| response.dragged())
    {
        let frame = frame_for_x(track, start, duration, pos.x);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(MARKER_DRAG), (id, frame)));
    }
    if response.drag_stopped() {
        ctx.data_mut(|d| d.remove::<String>(active));
    }
    true
}

/// The marker a drag moved during the last painted row: its id and the frame on
/// that row (`None` when no marker is dragged). Read right after the row.
pub fn take_marker_drag(ctx: &egui::Context) -> Option<(String, u64)> {
    ctx.data_mut(|d| d.remove_temp::<(String, u64)>(egui::Id::new(MARKER_DRAG)))
}

fn video_intent_from_response(
    response: &egui::Response,
    track: Rect,
    input: &TimelineInput<'_>,
) -> TimelineIntent {
    if track_marker_drag(response, track, input) {
        return TimelineIntent::None;
    }
    if let Some(drag) = mark_drag_intent(response, track, input) {
        return drag;
    }
    if let Some(take) = take_or_keep_intent(response, track, input) {
        return take;
    }
    if !(response.clicked() || response.dragged()) || !input.state.can_cue() {
        return TimelineIntent::None;
    }
    let Some(pos) = response.interact_pointer_pos() else {
        return TimelineIntent::None;
    };
    if !track.expand(2.0).contains(pos) {
        return TimelineIntent::None;
    }
    let frame = frame_for_x(
        track,
        input.state.range_start(),
        input.state.duration_frames(),
        pos.x,
    );
    // A double click on the picture is its slot, enlarged (its first click placed the
    // playhead); A1 below the slot is the segment's (`paint_audio_band`).
    if response.double_clicked() {
        if let Some(id) = slot_of_frame(input.marker_slots, frame) {
            return TimelineIntent::OpenSlot { id: id.to_string(), frame };
        }
    }
    if response.clicked() {
        let modifiers = response.ctx.input(|i| i.modifiers);
        // Shift + left click adds an M marker there (as Shift+M; user rule 2026-10-20).
        if input.layers.markers && modifiers.shift {
            return TimelineIntent::AddMarkerAt(frame);
        }
        if input.layers.covers {
            if let Some(id) = cover_hit(
                track,
                input.state.range_start(),
                input.state.duration_frames(),
                pos,
                input.covers,
                cover_rows(track, input),
            ) {
                // A left click on a colour strip selects its slot; Ctrl takes the cover (for
                // Delete); a cover in no slot is selected.
                let id = id.to_string();
                return match (modifiers.command, slot_of_frame(input.marker_slots, frame)) {
                    (true, _) => TimelineIntent::TakeCoverOnStrip { id, frame },
                    (false, Some(slot)) => TimelineIntent::SelectMarkerSlot { id: slot.to_string(), frame },
                    (false, None) => TimelineIntent::SelectCover { id, frame },
                };
            }
        }
        // Outside the colour strip a left click only places the playhead (user 2026-10-17).
        if input.layers.virtual_spans {
            if let Some(id) = virtual_span_hit(
                track,
                input.state.range_start(),
                input.state.duration_frames(),
                pos,
                input.virtual_spans,
                video_middle_insets(input),
            ) {
                return TimelineIntent::SelectVirtual {
                    id: id.to_string(),
                    frame,
                };
            }
        }
    }
    TimelineIntent::CueFrame(frame)
}

/// The slot a program frame lies in.
fn slot_of_frame<'a>(slots: &'a [TimelineSlotSpan<'a>], frame: u64) -> Option<&'a str> {
    slots.iter().find(|slot| (slot.start_frame..slot.end_frame).contains(&frame)).map(|slot| slot.id)
}

fn cover_hit<'a>(
    track: Rect,
    range_start: u64,
    duration: u64,
    pos: egui::Pos2,
    covers: &'a [TimelineCoverSpan<'a>],
    rows: (f32, f32),
) -> Option<&'a str> {
    covers.iter().rev().find_map(|cover| {
        if cover.end_frame <= cover.start_frame || cover.id.trim().is_empty() {
            return None;
        }
        let x0 = x_for_frame(track, range_start, duration, cover.start_frame);
        let x1 = x_for_frame(track, range_start, duration, cover.end_frame);
        let rect = Rect::from_min_max(egui::pos2(x0, rows.0), egui::pos2(x1.max(x0 + 3.0), rows.1)).expand(2.0);
        rect.contains(pos).then_some(cover.id)
    })
}

/// The top and bottom of a cover in the video row: with audio bands the whole light middle
/// (user 2026-10-15: a cover paints the video part of the Wrap row), otherwise the upper part.
fn cover_rows(track: Rect, input: &TimelineInput<'_>) -> (f32, f32) {
    if cover_strip(input) {
        // Over the filmstrip a cover is a strip at the bottom of the picture, so its own
        // pictures show above it (user 2026-10-17).
        let bottom = track.bottom() - video_middle_insets(input).1;
        (bottom - COVER_STRIP, bottom)
    } else if input.layers.audio_bands {
        let (top, bottom) = video_middle_insets(input);
        (track.top() + top, track.bottom() - bottom)
    } else {
        (track.top() + 2.0, track.top() + track.height() * 0.42)
    }
}

/// A Wrap row in its filmstrip mode (not the narrow video row): a cover is a strip at the
/// bottom of the picture, shown under the mouse or when chosen. It is the row's mode, not
/// whether its filmstrip pictures are read yet (audit 2026-10-17: a row whose pictures were
/// not there drew the cover over the whole video part, always, and every click hit it).
fn cover_strip(input: &TimelineInput<'_>) -> bool {
    input.layers.audio_bands && !input.layers.compact_video
}

/// How far below the top of the video row a slot starts: with audio bands under the A1 band,
/// so a slot marks the light middle and A2; A1 is the base of the slot, not part of it (user
/// 2026-10-15).
fn slot_top(input: &TimelineInput<'_>) -> f32 {
    if input.layers.audio_bands { video_middle_insets(input).0 } else { 0.0 }
}

fn virtual_span_hit<'a>(
    track: Rect,
    range_start: u64,
    duration: u64,
    pos: egui::Pos2,
    spans: &'a [TimelineVirtualSpan<'a>],
    (top, bottom): (f32, f32),
) -> Option<&'a str> {
    spans.iter().rev().find_map(|span| {
        if span.end_frame <= span.start_frame || span.id.trim().is_empty() {
            return None;
        }
        let x0 = x_for_frame(track, range_start, duration, span.start_frame);
        let x1 = x_for_frame(track, range_start, duration, span.end_frame);
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + top),
            egui::pos2(x1.max(x0 + 3.0), track.bottom() - bottom),
        )
        .expand(2.0);
        rect.contains(pos).then_some(span.id)
    })
}

fn split_label_track(row: Rect, label_width: f32) -> (Rect, Rect) {
    let label_width = label_width.clamp(1.0, row.width().max(1.0));
    let label = Rect::from_min_size(row.min, Vec2::new(label_width, row.height()));
    let track = Rect::from_min_max(egui::pos2(label.right(), row.top()), row.right_bottom());
    (label, track)
}

pub fn x_for_frame(track: Rect, range_start: u64, duration_frames: u64, frame: u64) -> f32 {
    let local = frame.saturating_sub(range_start).min(duration_frames);
    x_for_local_frame(track, duration_frames, local)
}

pub fn x_for_local_frame(track: Rect, duration_frames: u64, frame: u64) -> f32 {
    let duration = duration_frames.max(1) as f32;
    let t = frame.min(duration_frames) as f32 / duration;
    track.left() + t * track.width()
}

pub fn frame_for_x(track: Rect, range_start: u64, duration_frames: u64, x: f32) -> u64 {
    if track.width() <= 0.0 {
        return range_start;
    }
    let t = ((x - track.left()) / track.width()).clamp(0.0, 1.0) as f64;
    let duration = duration_frames.max(1);
    let local = (t * duration as f64).round() as u64;
    range_start.saturating_add(local.min(duration.saturating_sub(1)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_uses_supplied_playhead_as_only_authority() {
        let state = TimelineProjection::new(10, 50)
            .with_playhead(12)
            .with_cue_enabled(true);

        assert_eq!(state.range_start(), 10);
        assert_eq!(state.confirmed_frame(), Some(12));
        assert_eq!(state.duration_frames(), 50);
        assert!(state.can_cue());
    }

    #[test]
    fn missing_projection_does_not_create_fallback_position() {
        let state = TimelineProjection::new(0, 300);

        assert_eq!(state.confirmed_frame(), None);
        assert!(!state.can_cue());
    }

    #[test]
    fn loaded_source_shows_in_and_out_across_the_clip_until_a_mark_moves_them() {
        let state = TimelineProjection::new(0, 100).with_playhead(10);
        assert_eq!(state.visible_source_marks(), Some((0, 100)));

        let marked = state.with_source_in_at_confirmed();
        assert_eq!(marked.visible_source_marks(), Some((10, 100)));
    }

    #[test]
    fn empty_projection_draws_no_in_out_marks() {
        assert_eq!(TimelineProjection::default().visible_source_marks(), None);
    }

    #[test]
    fn source_marks_use_confirmed_player_frame() {
        let state = TimelineProjection::new(10, 90)
            .with_playhead(35)
            .with_source_in_at_confirmed()
            .with_playhead(55)
            .with_source_out_at_confirmed();

        assert_eq!(state.source_in_frame, Some(25));
        assert_eq!(state.source_out_frame, Some(45));
    }

    #[test]
    fn source_marks_survive_player_projection_refresh() {
        let previous = TimelineProjection::new(0, 100)
            .with_playhead(20)
            .with_source_in_at_confirmed();
        let refreshed = TimelineProjection::new(0, 100)
            .with_playhead(40)
            .preserving_source_marks_from(&previous);

        assert_eq!(refreshed.confirmed_frame(), Some(40));
        assert_eq!(refreshed.source_in_frame, Some(20));
    }

    #[test]
    fn marking_in_out_or_a_cut_leaves_the_playhead_with_the_click() {
        let marked = TimelineProjection::new(0, 300).with_playhead(40).with_source_in_at_confirmed();
        assert_eq!(marked.source_mark_focus, TimelineSourceMarkFocus::Playhead, "I does not take IN");
        let out = marked.with_playhead(90).with_source_out_at_confirmed();
        assert_eq!(out.source_mark_focus, TimelineSourceMarkFocus::Playhead, "O does not take OUT");
        let cut = TimelineProjection::new(0, 300).with_playhead(10).with_first_cut_at_confirmed();
        let pair = cut.with_playhead(60).with_second_cut_at_confirmed(10);
        assert_eq!((cut.source_mark_focus, pair.source_mark_focus), (TimelineSourceMarkFocus::Playhead, TimelineSourceMarkFocus::Playhead));
    }

    #[test]
    fn the_length_follows_the_playhead_from_in_until_out_is_set() {
        let marked_in = TimelineProjection::new(0, 300).with_playhead(40).with_source_in_at_confirmed();
        let moved = marked_in.with_playhead(115);
        assert_eq!(moved.shown_source_length(moved.visible_source_marks().unwrap()), 75, "IN 40 to the playhead");
        let out = moved.with_source_out_at_confirmed().with_playhead(200);
        assert_eq!(out.shown_source_length(out.visible_source_marks().unwrap()), 75, "OUT set: IN to OUT");
    }

    #[test]
    fn source_marks_survive_a_refresh_that_does_not_know_the_length_yet() {
        let previous = TimelineProjection::new(0, 300)
            .with_playhead(120)
            .with_source_in_at_confirmed()
            .with_playhead(200)
            .with_source_out_at_confirmed();
        let preparing = TimelineProjection::default().preserving_source_marks_from(&previous);
        assert_eq!((preparing.source_in_frame, preparing.source_out_frame), (Some(120), Some(200)));
        let ready = TimelineProjection::new(0, 300).preserving_source_marks_from(&preparing);
        assert_eq!((ready.source_in_frame, ready.source_out_frame), (Some(120), Some(200)));
    }

    #[test]
    fn frame_geometry_is_clamped_to_player_range() {
        let track = Rect::from_min_size(egui::pos2(100.0, 0.0), Vec2::new(200.0, 20.0));

        assert_eq!(x_for_frame(track, 50, 100, 50), 100.0);
        assert_eq!(x_for_frame(track, 50, 100, 150), 300.0);
        assert_eq!(frame_for_x(track, 50, 100, 200.0), 100);
        assert_eq!(frame_for_x(track, 50, 100, -100.0), 50);
        assert_eq!(frame_for_x(track, 50, 100, 999.0), 149);
    }

    #[test]
    fn source_height_matches_v4_lane_stack() {
        assert_eq!(
            height_for_layers(TimelineLayerFlags::source(), AudioLane::None),
            102.0
        );
    }

    #[test]
    fn audio_bands_keep_the_height_of_the_stack_and_put_a1_a2_in_the_video_row() {
        let metrics = TimelineMetrics::default();
        let lanes = TimelineLayerFlags::a1_v_a2().with_overlays();
        let banded = lanes.with_audio_bands();
        assert_eq!(metrics.outer_height(banded, AudioLane::None), metrics.outer_height(lanes, AudioLane::None));
        assert_eq!(metrics.banded_video_height(banded), metrics.video_height + 2.0 * metrics.audio_height);
        assert_eq!(metrics.banded_video_height(banded.with_compact_video(true)), 4.0 * metrics.audio_height, "a narrow video row");
        assert_eq!(
            metrics.banded_video_height(banded) - 2.0 * metrics.audio_height,
            metrics.video_height,
            "A1, A2 and the light middle as high as the source timeline's lanes and video row"
        );
    }

    #[test]
    fn source_preset_keeps_video_row_without_base_video_layer() {
        let layers = TimelineLayerFlags::source();

        assert!(layers.carrier);
        assert!(layers.audio_a1);
        assert!(layers.audio_a2);
        assert!(!layers.base_video);
        assert!(!layers.shot_range);
        assert!(layers.in_out);
        assert!(layers.playhead);
        assert!(layers.video_stack_on());
    }

    #[test]
    fn filmstrip_slot_count_is_stable_while_assets_arrive() {
        assert_eq!(
            filmstrip_slot_count_from_indices([0, FILMSTRIP_FRAME_COUNT - 1]),
            FILMSTRIP_FRAME_COUNT
        );
    }

    #[test]
    fn audio_lane_toggle_is_stateless_projection() {
        assert_eq!(AudioLane::None.toggle(AudioLane::A1), AudioLane::A1);
        assert_eq!(AudioLane::A1.toggle(AudioLane::A1), AudioLane::None);
        assert_eq!(AudioLane::A1.toggle(AudioLane::A2), AudioLane::A2);
    }

    #[test]
    fn passive_overlay_projection_enables_prepared_ui_layers_only() {
        let layers = TimelineLayerFlags::passive_overlay_projection();

        assert!(layers.audio_a1);
        assert!(layers.carrier);
        assert!(layers.base_video);
        assert!(layers.audio_a2);
        assert!(layers.virtual_spans);
        assert!(layers.covers);
        assert!(layers.markers);
        assert!(layers.marker_slots);
        assert!(!layers.shot_range);
        assert!(!layers.in_out);
    }

    #[test]
    fn layer_presets_do_not_create_separate_timeline_engines() {
        let plain = TimelineLayerFlags::a1_v_a2();
        let source = TimelineLayerFlags::source();

        assert!(plain.audio_a1);
        assert!(plain.carrier);
        assert!(plain.base_video);
        assert!(plain.audio_a2);
        assert!(!plain.shot_range);
        assert!(!plain.in_out);
        assert!(!source.base_video);
        assert!(!source.shot_range, "the whole clip needs no outline");
        assert!(source.in_out);
    }

    #[test]
    fn prepared_overlay_hits_use_same_frame_geometry() {
        let track = Rect::from_min_size(egui::pos2(100.0, 0.0), Vec2::new(200.0, 64.0));
        let spans = [
            TimelineVirtualSpan {
                id: "virtual_a",
                label: "tonovi",
                start_frame: 60,
                end_frame: 90,
                has_base_video: true,
                selected: false,
            },
            TimelineVirtualSpan {
                id: "virtual_b",
                label: "offovi",
                start_frame: 70,
                end_frame: 100,
                has_base_video: true,
                selected: true,
            },
        ];
        let covers = [TimelineCoverSpan {
            id: "cover_mid",
            start_frame: 65,
            end_frame: 80,
            selected: false,
            pending: false,
            fill: None,
            label: None,
        }];
        let slots = [TimelineSlotSpan {
            id: "slot_mid",
            start_frame: 60,
            end_frame: 85,
            has_cover: false,
            selected: false,
        }];
        let markers = [TimelineMarkerPin {
            id: "m_mid",
            frame: 75,
            taken: false,
            locked: false,
            part: TimelineMarkerPart::Whole,
            origin: None,
            sound_at: None,
        }];

        // A right click off the strip takes the nearest marker, however far (user 2026-10-20).
        assert_eq!(marker_for_right_click(track, 50, 100, egui::pos2(150.0, 20.0), &markers, false), Some("m_mid"));
        assert_eq!(marker_for_right_click(track, 50, 100, egui::pos2(400.0, 20.0), &markers, false), Some("m_mid"));
        assert_eq!(
            cover_hit(track, 50, 100, egui::pos2(140.0, 8.0), &covers, (2.0, 26.0)),
            Some("cover_mid")
        );
        assert_eq!(slot_of_frame(&slots, 70), Some("slot_mid"), "a click on a strip is the slot of its frame");
        assert_eq!(
            virtual_span_hit(track, 50, 100, egui::pos2(150.0, 34.0), &spans, (VIDEO_BAND, VIDEO_FOOT)),
            Some("virtual_b")
        );
        assert_eq!(frame_for_x(track, 50, 100, 150.0), 75);
    }

    #[test]
    fn program_row_projects_continuous_player_frame_to_local_row() {
        let first = TimelineProgramRow::new(0, 3, 0, 50);
        let second = TimelineProgramRow::new(1, 3, 50, 100);
        let last = TimelineProgramRow::new(2, 3, 100, 125);

        assert_eq!(first.local_playhead_frame(0), Some(0));
        assert_eq!(first.local_playhead_frame(49), Some(49));
        assert_eq!(first.local_playhead_frame(50), None);
        assert_eq!(second.local_playhead_frame(50), Some(0));
        assert_eq!(second.local_playhead_frame(75), Some(25));
        assert_eq!(second.local_playhead_frame(100), None);
        assert_eq!(last.local_playhead_frame(125), Some(25));
    }

    #[test]
    fn program_row_maps_local_ui_request_back_to_program_frame() {
        let row = TimelineProgramRow::new(1, 3, 50, 100);

        assert_eq!(row.program_frame_from_local(0), 50);
        assert_eq!(row.program_frame_from_local(12), 62);
        assert_eq!(row.program_frame_from_local(200), 100);
    }

    #[test]
    fn program_row_projects_ranges_and_boundary_markers_like_v4() {
        let first = TimelineProgramRow::new(0, 3, 0, 50);
        let middle = TimelineProgramRow::new(1, 3, 50, 100);
        let last = TimelineProgramRow::new(2, 3, 100, 125);

        assert_eq!(middle.local_range(40, 70), Some((0, 20)));
        assert_eq!(middle.local_range(70, 130), Some((20, 50)));
        assert_eq!(middle.local_range(0, 20), None);
        assert_eq!(first.local_marker_frame(0), Some(0));
        assert_eq!(first.local_marker_frame(50), Some(50));
        assert_eq!(middle.local_marker_frame(50), None);
        assert_eq!(middle.local_marker_frame(100), Some(50));
        assert_eq!(last.local_marker_frame(100), None);
        assert_eq!(last.local_marker_frame(125), Some(25));
    }
}

/// The wave of one audio lane over the video row (user rule 2026-10-01): as high as
/// that row, its peaks at a display gain of 2^(level-1); the sound is not changed.
/// The peaks of source channel `channel` (zero based): the timeline gets the clip's
/// channels 1..4 in order.
fn channel_peaks<'a>(input: &TimelineInput<'a>, channel: u16) -> &'a [f32] {
    match channel {
        0 => input.a1_peaks,
        1 => input.a2_peaks,
        2 => input.a3_peaks,
        3 => input.a4_peaks,
        _ => &[],
    }
}

/// A lane with a channel choice draws the wave of its chosen channel (the draft while
/// taken); without one, its own peaks.
fn lane_peaks<'a>(input: &TimelineInput<'a>, choice: Option<ChannelChoice>, own: &'a [f32]) -> &'a [f32] {
    // A lane that is off is heard as nothing, so its wave is flat. A banded row (a Wrap row)
    // gets its lanes' waves already of the chosen channels: it has no other channels to pick.
    // The banded row's wave is already each part's channel, level and off as drafted.
    choice.map_or(own, |choice| match () {
        _ if input.layers.audio_bands => own,
        _ if choice.silent() => &[],
        _ => channel_peaks(input, choice.selected),
    })
}

/// The level a lane is heard at (its channel choice's dB, 0 when it is off); 1 without a
/// choice.
fn lane_gain(input: &TimelineInput<'_>, lane: AudioLane) -> f32 {
    if input.layers.audio_bands {
        return 1.0; // a Wrap row's wave holds each part's level already (live 2026-10-22)
    }
    let choice = match lane {
        AudioLane::A1 => input.a1_channel,
        AudioLane::A2 => input.a2_channel,
        _ => None,
    };
    choice.map_or(1.0, |choice| choice.gain())
}

fn paint_wave_over_video(ui: &mut egui::Ui, row: Rect, peaks: &[f32], level: u8, lane_gain: f32, input: &TimelineInput<'_>) {
    let wave = input.theme.audio_wave;
    if level == 0 {
        return;
    }
    let (_, track) = split_label_track(row, input.metrics.label_width);
    let zoom = f32::from(1u16 << (level - 1).min(6));
    let painter = ui.painter().with_clip_rect(track);
    painter.rect_filled(track, 0.0, input.theme.audio_primary_background); // the same lane, only taller
    qnc_wave_view::paint_wave_peaks_gain(&painter, track, peaks, zoom * lane_gain, wave);
    qnc_wave_view::paint_level_marks(&painter, track, zoom, input.theme.level_marks);
    paint_ranges_and_playhead(ui, track, input);
}

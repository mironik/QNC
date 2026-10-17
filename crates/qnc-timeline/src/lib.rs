//! Passive public QNC timeline UI component.
//!
//! The timeline-engine paints a supplied frame-space UI projection and emits
//! user intents. It owns no playback clock, no database, no scanner, no probe,
//! no media processing path, and no Broadcast Player event interpretation.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use qnc_filmstrip::{FILMSTRIP_FRAME_COUNT, FilmstripBackground};
pub use qnc_channel_picker::ChannelChoice;

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
const COVER_STRIP: f32 = 13.0;

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

    pub fn with_source_in_at_confirmed(mut self) -> Self {
        if let Some(frame) = self.confirmed_frame() {
            self.source_in_frame = Some(self.local_frame(frame));
            if let Some(out) = self.source_out_frame {
                if out <= self.source_in_frame.unwrap_or(0) {
                    self.source_out_frame = Some(self.duration_frames());
                }
            }
            self.source_mark_focus = TimelineSourceMarkFocus::In;
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
            self.source_mark_focus = TimelineSourceMarkFocus::Out;
        }
        self.normalize_source_marks()
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
    /// Filmstrip pictures of a banded video row (empty: none).
    pub pictures: &'a [TimelinePicture<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimelineIntent {
    None,
    CueFrame(u64),
    ToggleAudioExpand(AudioLane),
    SelectVirtual { id: String, frame: u64 },
    SelectCover { id: String, frame: u64 },
    SelectMarkerSlot { id: String, frame: u64 },
    SelectMarker { id: String, frame: u64 },
    /// The source channel (zero based) chosen for A1 in the channel picker.
    ChooseA1Channel(u16),
    /// The source channel (zero based) chosen for A2 in its channel picker.
    ChooseA2Channel(u16),
    /// A click on the A1 or A2 label: the lane is taken (as Ctrl+1, Ctrl+2).
    TakeAudioLane(AudioLane),
    /// A click on the V label: Home of the panel (start of the clip or of the program).
    GoHome,
    /// Ctrl+click on the V label of a banded row: filmstrip or narrow video (user
    /// 2026-10-15).
    ToggleVideoMode,
    /// The mouse wheel over the timeline (user 2026-10-16: the timeline behaves the same
    /// in every form): one step a frame, Ctrl held or not.
    Wheel(qnc_wheel_jog::Wheel),
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
                pictures: &[],
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
    let wheel = qnc_wheel_jog::take(ui, outer, ui.id().with("qnc-timeline-wheel"));
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
        .find_map(|(lane, row, choice)| Some((lane, row?, choice.filter(|c| c.open && input.show_lane_labels)?)));
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
                Some((lane, if choice.silent() { &[] } else { channel_peaks(&input, choice.selected) }, level))
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

    match (intent, wheel) {
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
    for (frame, label) in input.trim_playheads {
        let x = x_for_local_frame(track, duration, *frame);
        let color = input.theme.text;
        ui.painter().line_segment([egui::pos2(x, track.top()), egui::pos2(x, track.bottom())], Stroke::new(2.0, color));
        let text = ui.painter().layout_no_wrap(label.to_string(), FontId::proportional(11.0), color);
        let left = (x + 3.0).min(track.right() - text.size().x - 2.0).max(track.left());
        let rect = Rect::from_min_size(egui::pos2(left, track.top() + 1.0), text.size() + Vec2::new(4.0, 0.0));
        ui.painter().rect_filled(rect, 2.0, input.theme.background);
        ui.painter().galley(egui::pos2(left + 2.0, track.top() + 1.0), text, color);
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
        // A click on A1/A2 takes the lane as Ctrl+1/Ctrl+2 does (user rule 2026-10-01).
        return match picker {
            Some(_) => TimelineIntent::TakeAudioLane(lane),
            None => TimelineIntent::ToggleAudioExpand(lane),
        };
    }
    ui.painter().rect_filled(track_rect, 0.0, fill); // no frame: the lane meets the filmstrip
    let gain = lane_gain(input, lane);
    qnc_wave_view::paint_wave_peaks_gain(ui.painter(), track_rect, peaks, gain, wave);
    qnc_wave_view::paint_level_marks(ui.painter(), track_rect, 1.0, input.theme.level_marks);
    paint_lane_level(ui, track_rect, lane, input);
    paint_ranges_and_playhead(ui, track_rect, input);
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
        // A click on A1/A2 takes the lane as Ctrl+1/Ctrl+2 does (user rule 2026-10-01).
        return match picker {
            Some(_) => TimelineIntent::TakeAudioLane(lane),
            None => TimelineIntent::ToggleAudioExpand(lane),
        };
    }
    TimelineIntent::None
}

/// The insets of the light middle of the video row: the dark band above it and the dark
/// part under it (a band as high with audio bands).
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
        if input.layers.markers {
            paint_markers(ui, track, range_start, duration, input.markers, input);
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
    let in_stroke = if input.focus == TimelineFocusPaint::In {
        Stroke::new(3.0, input.theme.focus)
    } else {
        Stroke::new(2.0, input.theme.text)
    };
    let out_stroke = if input.focus == TimelineFocusPaint::Out {
        Stroke::new(3.0, input.theme.focus)
    } else {
        Stroke::new(2.0, input.theme.text)
    };
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
        let base = if cover.pending {
            input.theme.focus
        } else {
            cover.fill.unwrap_or(input.theme.shot_range)
        };
        let fill = if cover.selected {
            base.linear_multiply(0.85)
        } else {
            base.linear_multiply(0.55)
        };
        ui.painter().rect_filled(rect, 1.0, fill);
        if let Some(label) = cover.label.filter(|_| !cover.pending && rect.width() >= 34.0) {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(10.0),
                Color32::BLACK,
            );
        }
        if cover.pending && rect.width() >= 56.0 {
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
    for marker in markers {
        if marker.id.trim().is_empty() {
            continue;
        }
        let x = x_for_frame(track, range_start, duration, marker.frame);
        // With audio bands a marker belongs to the covers: its line runs over the video and
        // A2, never over A1, and it has no "M" over A1's wave (user 2026-10-15).
        let top = track.top() + slot_top(input);
        ui.painter().line_segment(
            [egui::pos2(x, top), egui::pos2(x, track.bottom())],
            Stroke::new(1.0, input.theme.focus),
        );
        if !input.layers.audio_bands {
            ui.painter().text(
                egui::pos2(x + 3.0, track.top() + 2.0),
                Align2::LEFT_TOP,
                "M",
                FontId::proportional(9.0),
                input.theme.focus,
            );
        }
    }
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
        let hit = ctx
            .input(|i| i.pointer.press_origin())
            .and_then(|pos| marker_hit(track, start, duration, pos, input.markers))
            .map(str::to_string);
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
    if response.clicked() {
        if input.layers.covers && input.layers.markers {
            if let Some(id) = marker_hit(
                track,
                input.state.range_start(),
                input.state.duration_frames(),
                pos,
                input.markers,
            ) {
                return TimelineIntent::SelectMarker {
                    id: id.to_string(),
                    frame,
                };
            }
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
                return TimelineIntent::SelectCover {
                    id: id.to_string(),
                    frame,
                };
            }
        }
        if input.layers.covers && input.layers.marker_slots {
            if let Some(id) = slot_hit(
                track,
                input.state.range_start(),
                input.state.duration_frames(),
                pos,
                input.marker_slots,
                slot_top(input),
            ) {
                return TimelineIntent::SelectMarkerSlot {
                    id: id.to_string(),
                    frame,
                };
            }
        }
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

fn marker_hit<'a>(
    track: Rect,
    range_start: u64,
    duration: u64,
    pos: egui::Pos2,
    markers: &'a [TimelineMarkerPin<'a>],
) -> Option<&'a str> {
    let tolerance = 5.0;
    markers.iter().find_map(|marker| {
        let x = x_for_frame(track, range_start, duration, marker.frame);
        (!marker.id.trim().is_empty() && (pos.x - x).abs() <= tolerance).then_some(marker.id)
    })
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
    if input.layers.audio_bands && !input.pictures.is_empty() {
        // Over the filmstrip a cover is a strip at the top of the picture, so its own
        // pictures show under it.
        let top = track.top() + video_middle_insets(input).0;
        (top, top + COVER_STRIP)
    } else if input.layers.audio_bands {
        let (top, bottom) = video_middle_insets(input);
        (track.top() + top, track.bottom() - bottom)
    } else {
        (track.top() + 2.0, track.top() + track.height() * 0.42)
    }
}

/// How far below the top of the video row a slot starts: with audio bands under the A1 band,
/// so a slot marks the light middle and A2; A1 is the base of the slot, not part of it (user
/// 2026-10-15).
fn slot_top(input: &TimelineInput<'_>) -> f32 {
    if input.layers.audio_bands { video_middle_insets(input).0 } else { 0.0 }
}

fn slot_hit<'a>(
    track: Rect,
    range_start: u64,
    duration: u64,
    pos: egui::Pos2,
    slots: &'a [TimelineSlotSpan<'a>],
    top: f32,
) -> Option<&'a str> {
    slots.iter().find_map(|slot| {
        if slot.end_frame <= slot.start_frame || slot.id.trim().is_empty() {
            return None;
        }
        let x0 = x_for_frame(track, range_start, duration, slot.start_frame);
        let x1 = x_for_frame(track, range_start, duration, slot.end_frame);
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + top + 2.0),
            egui::pos2(x1.max(x0 + 3.0), track.bottom() - 2.0),
        )
        .expand(2.0);
        rect.contains(pos).then_some(slot.id)
    })
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
        }];

        assert_eq!(
            marker_hit(track, 50, 100, egui::pos2(150.0, 20.0), &markers),
            Some("m_mid")
        );
        assert_eq!(
            cover_hit(track, 50, 100, egui::pos2(140.0, 8.0), &covers, (2.0, 26.0)),
            Some("cover_mid")
        );
        assert_eq!(
            slot_hit(track, 50, 100, egui::pos2(135.0, 55.0), &slots, 0.0),
            Some("slot_mid")
        );
        assert_eq!(
            slot_hit(track, 50, 100, egui::pos2(135.0, 6.0), &slots, 18.0),
            None,
            "with audio bands A1 is the base of the slot, not part of it"
        );
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
    // A lane that is off is heard as nothing, so its wave is flat.
    choice.map_or(own, |choice| if choice.silent() { &[] } else { channel_peaks(input, choice.selected) })
}

/// The level a lane is heard at (its channel choice's dB, 0 when it is off); 1 without a
/// choice.
fn lane_gain(input: &TimelineInput<'_>, lane: AudioLane) -> f32 {
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

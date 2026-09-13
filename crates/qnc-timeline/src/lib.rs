//! Passive public QNC timeline UI component.
//!
//! The timeline-engine paints a supplied frame-space UI projection and emits
//! user intents. It owns no playback clock, no database, no scanner, no probe,
//! no media processing path, and no Broadcast Player event interpretation.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use qnc_filmstrip::FilmstripBackground;

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
}

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
        }
    }

    pub fn source() -> Self {
        let mut layers = Self::a1_v_a2().with_source_marks();
        layers.base_video = false;
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
            audio_height: 15.0,
            audio_expanded_height: 52.0,
            video_height: 64.0,
            row_gap: 3.0,
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
            audio_primary_background: surface,
            audio_secondary_background: background,
            border,
            text,
            muted,
            playhead: accent,
            focus: Color32::from_rgb(255, 180, 60),
            shot_range: Color32::from_rgb(96, 165, 250),
            in_out_dim: Color32::from_black_alpha(133),
            wave_a1: Color32::from_rgb(16, 185, 129),
            wave_a2: Color32::from_rgb(107, 114, 128),
            wave_a3: Color32::from_rgb(75, 85, 99),
            wave_a4: Color32::from_rgb(55, 65, 81),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineProjection {
    pub range_start_frame: u64,
    pub duration_frames: u64,
    pub playhead_frame: Option<u64>,
    pub cue_enabled: bool,
}

impl Default for TimelineProjection {
    fn default() -> Self {
        Self {
            range_start_frame: 0,
            duration_frames: 1,
            playhead_frame: None,
            cue_enabled: false,
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
    let duration = state.duration_frames();
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
                focus: TimelineFocusPaint::Playhead,
                show_lane_labels: true,
                shot_in_frame: 0,
                shot_out_frame: duration,
                draft_in_frame: 0,
                draft_out_frame: duration,
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

    if input.layers.audio_a1 {
        let row = next_row(
            &mut next_top,
            left,
            right,
            input
                .metrics
                .lane_height(AudioLane::A1, input.expanded_audio),
            input.metrics.row_gap,
        );
        keep_first_intent(
            &mut intent,
            paint_audio_row(
                ui,
                row,
                AudioLane::A1,
                input.a1_peaks,
                input.theme.audio_primary_background,
                input.theme.wave_a1,
                &input,
            ),
        );
    }
    if input.layers.video_stack_on() {
        let row = next_row(
            &mut next_top,
            left,
            right,
            input.metrics.video_height,
            input.metrics.row_gap,
        );
        keep_first_intent(&mut intent, paint_video_row(ui, row, &input));
    }
    if input.layers.audio_a2 {
        let row = next_row(
            &mut next_top,
            left,
            right,
            input
                .metrics
                .lane_height(AudioLane::A2, input.expanded_audio),
            input.metrics.row_gap,
        );
        keep_first_intent(
            &mut intent,
            paint_audio_row(
                ui,
                row,
                AudioLane::A2,
                input.a2_peaks,
                input.theme.audio_secondary_background,
                input.theme.wave_a2,
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
                input.theme.wave_a3,
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
                input.theme.wave_a4,
                &input,
            ),
        );
    }

    intent
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
    if input.show_lane_labels && label_response.clicked() {
        return TimelineIntent::ToggleAudioExpand(lane);
    }
    ui.painter().rect_filled(track_rect, 0.0, fill);
    ui.painter().rect_stroke(
        track_rect,
        0.0,
        Stroke::new(1.0, input.theme.border),
        StrokeKind::Inside,
    );
    qnc_wave_view::paint_wave_peaks(ui.painter(), track_rect, peaks, wave);
    paint_ranges_and_playhead(ui, track_rect, input);
    cue_intent_from_response(&response, track_rect, input)
}

fn paint_video_row(ui: &mut egui::Ui, row: Rect, input: &TimelineInput<'_>) -> TimelineIntent {
    let (label_rect, track_rect) = split_label_track(row, input.metrics.label_width);
    let label = if input.show_lane_labels { "V" } else { "" };
    paint_lane_label(ui, label_rect, label, input);
    let response = ui.interact(
        track_rect,
        ui.make_persistent_id("qnc_timeline_video"),
        Sense::click_and_drag(),
    );
    ui.painter()
        .rect_filled(track_rect, 0.0, input.theme.video_background);
    ui.painter().rect_stroke(
        track_rect,
        0.0,
        Stroke::new(1.0, input.theme.border),
        StrokeKind::Inside,
    );
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
    let count = background.frames.len();
    let slot_width = track.width() / count as f32;
    for (slot_index, frame) in background.frames.iter().enumerate() {
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
        paint_in_out_dim(
            ui,
            track,
            duration,
            input.draft_in_frame,
            input.draft_out_frame,
            input.theme.in_out_dim,
        );
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
                ui.painter().rect_filled(
                    track.shrink2(Vec2::new(0.0, 10.0)),
                    1.0,
                    input.theme.wave_a1.linear_multiply(0.20),
                );
            }
        } else if input.layers.virtual_spans {
            paint_virtual_spans(ui, track, range_start, duration, input.virtual_spans, input);
        }
    } else if input.layers.virtual_spans {
        if input.virtual_spans.is_empty() && !input.base_video_blank {
            ui.painter().rect_filled(
                track.shrink2(Vec2::new(0.0, 10.0)),
                1.0,
                input.theme.wave_a1.linear_multiply(0.20),
            );
        } else {
            paint_virtual_spans(ui, track, range_start, duration, input.virtual_spans, input);
        }
    }
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
        paint_in_out_dim(
            ui,
            track,
            duration,
            input.draft_in_frame,
            input.draft_out_frame,
            input.theme.in_out_dim,
        );
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
    ui.painter().line_segment(
        [
            egui::pos2(in_x, track.top()),
            egui::pos2(in_x, track.bottom()),
        ],
        in_stroke,
    );
    ui.painter().line_segment(
        [
            egui::pos2(out_x, track.top()),
            egui::pos2(out_x, track.bottom()),
        ],
        out_stroke,
    );
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
            egui::pos2(x0, track.top() + 17.0),
            egui::pos2(x1.max(x0 + 3.0), track.bottom() - 10.0),
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
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + 2.0),
            egui::pos2(x1.max(x0 + 3.0), track.top() + track.height() * 0.42),
        );
        let base = if cover.pending {
            input.theme.focus
        } else {
            input.theme.shot_range
        };
        let fill = if cover.selected {
            base.linear_multiply(0.85)
        } else {
            base.linear_multiply(0.55)
        };
        ui.painter().rect_filled(rect, 1.0, fill);
        if cover.pending && rect.width() >= 56.0 {
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "nije na programu",
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
            egui::pos2(x0, track.top() + 2.0),
            egui::pos2(x1.max(x0 + 3.0), track.bottom() - 2.0),
        );
        let base = if slot.selected {
            input.theme.focus
        } else if slot.has_cover {
            input.theme.shot_range
        } else {
            input.theme.playhead
        };
        ui.painter().rect_filled(
            rect,
            1.0,
            base.linear_multiply(if selected_only { 0.28 } else { 0.18 }),
        );
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
        ui.painter().line_segment(
            [egui::pos2(x, track.top()), egui::pos2(x, track.bottom())],
            Stroke::new(1.0, input.theme.focus),
        );
        ui.painter().text(
            egui::pos2(x + 3.0, track.top() + 2.0),
            Align2::LEFT_TOP,
            "M",
            FontId::proportional(9.0),
            input.theme.focus,
        );
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

fn video_intent_from_response(
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
) -> Option<&'a str> {
    covers.iter().rev().find_map(|cover| {
        if cover.end_frame <= cover.start_frame || cover.id.trim().is_empty() {
            return None;
        }
        let x0 = x_for_frame(track, range_start, duration, cover.start_frame);
        let x1 = x_for_frame(track, range_start, duration, cover.end_frame);
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + 2.0),
            egui::pos2(x1.max(x0 + 3.0), track.top() + track.height() * 0.42),
        )
        .expand(2.0);
        rect.contains(pos).then_some(cover.id)
    })
}

fn slot_hit<'a>(
    track: Rect,
    range_start: u64,
    duration: u64,
    pos: egui::Pos2,
    slots: &'a [TimelineSlotSpan<'a>],
) -> Option<&'a str> {
    slots.iter().find_map(|slot| {
        if slot.end_frame <= slot.start_frame || slot.id.trim().is_empty() {
            return None;
        }
        let x0 = x_for_frame(track, range_start, duration, slot.start_frame);
        let x1 = x_for_frame(track, range_start, duration, slot.end_frame);
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + 2.0),
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
) -> Option<&'a str> {
    spans.iter().rev().find_map(|span| {
        if span.end_frame <= span.start_frame || span.id.trim().is_empty() {
            return None;
        }
        let x0 = x_for_frame(track, range_start, duration, span.start_frame);
        let x1 = x_for_frame(track, range_start, duration, span.end_frame);
        let rect = Rect::from_min_max(
            egui::pos2(x0, track.top() + 17.0),
            egui::pos2(x1.max(x0 + 3.0), track.bottom() - 10.0),
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
    fn source_preset_keeps_video_row_without_base_video_layer() {
        let layers = TimelineLayerFlags::source();

        assert!(layers.carrier);
        assert!(layers.audio_a1);
        assert!(layers.audio_a2);
        assert!(!layers.base_video);
        assert!(layers.shot_range);
        assert!(layers.in_out);
        assert!(layers.playhead);
        assert!(layers.video_stack_on());
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
        assert!(source.shot_range);
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
            cover_hit(track, 50, 100, egui::pos2(140.0, 8.0), &covers),
            Some("cover_mid")
        );
        assert_eq!(
            slot_hit(track, 50, 100, egui::pos2(135.0, 55.0), &slots),
            Some("slot_mid")
        );
        assert_eq!(
            virtual_span_hit(track, 50, 100, egui::pos2(150.0, 34.0), &spans),
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

//! Public "Segmenti" panel of an editorial form (QNC v5 `editorial/segment_panel.rs`
//! and `qnc_segment_timeline::show_program`).
//!
//! Passive paint only: a header with the timing of the program and one row per
//! segment. Every row is a local projection of the same program axis, painted by
//! the one public `qnc-timeline`. The panel receives a prepared `SegmentsView` and
//! returns what the user asked for; it never reads a database, plays, seeks or
//! decides the program.

use eframe::egui::{self, Color32, RichText, Vec2};
use qnc_program_segments::{SegmentCommand, SegmentRow, SegmentsView};
use qnc_timeline::{
    AudioLane, TimelineFocusPaint, TimelineInput, TimelineIntent, TimelineLayerFlags,
    TimelineMetrics, TimelineProjection, TimelineTheme, TimelineVirtualSpan,
};

pub const MODULE_ID: &str = "qnc.module.segment-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const PANEL_MARGIN: f32 = 10.0;
const ROW_GAP: f32 = 3.0;
const HEADER_LABEL_SIZE: f32 = 17.0;
const HEADER_VALUE_SIZE: f32 = 22.0;
const EMPTY_MESSAGE: &str = "Nema segmenata — dodaj ton i off segment";

/// Colours of the panel, taken from the timeline theme of the form; the total
/// colour is the v5 duration red.
#[derive(Debug, Clone, Copy)]
struct SegmentPanelStyle {
    surface: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    segment: Color32,
    playhead: Color32,
    total: Color32,
}

impl SegmentPanelStyle {
    fn from_timeline(theme: TimelineTheme) -> Self {
        Self {
            surface: theme.video_background,
            border: theme.border,
            text: theme.text,
            muted: theme.muted,
            segment: theme.playhead,
            playhead: theme.focus,
            total: Color32::from_rgb(255, 120, 120),
        }
    }
}

pub struct SegmentPanelInput<'a> {
    pub segments: &'a SegmentsView,
    /// Program frame of the program player; `None` while no program plays.
    pub playhead_frame: Option<u64>,
    pub timeline_theme: TimelineTheme,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentPanelAction {
    None,
    SelectSegment(String),
    /// A program frame the user pointed at; the program player decides.
    CueProgramFrame(u64),
}

/// The panel while no program plays: returns the segment the user selected.
pub fn show_selecting(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    timeline_theme: TimelineTheme,
) -> Option<SegmentCommand> {
    let input = SegmentPanelInput {
        segments,
        playhead_frame: None,
        timeline_theme,
    };
    match show(ui, input) {
        SegmentPanelAction::SelectSegment(segment_id) => Some(SegmentCommand::Select(segment_id)),
        _ => None,
    }
}

/// The Segment tab of the clip menu (v5 `media_pool::segment_cards`): kind, id and
/// duration per row; the selected row carries Up, Down and Del.
pub fn show_segment_list(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    theme: TimelineTheme,
    select: Color32,
) -> Option<SegmentCommand> {
    let mut command = None;
    egui::ScrollArea::vertical()
        .id_salt("qnc_segment_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            if segments.is_empty() {
                ui.colored_label(theme.muted, EMPTY_LIST_MESSAGE);
                return;
            }
            for row in &segments.rows {
                let stroke = if row.selected {
                    egui::Stroke::new(2.0, select)
                } else {
                    egui::Stroke::new(1.0, theme.border)
                };
                let response = egui::Frame::NONE
                    .stroke(stroke)
                    .fill(theme.label_background)
                    .inner_margin(egui::Margin::symmetric(10, 7))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width().max(180.0));
                        ui.horizontal(|ui| {
                            let kind = RichText::new(row.kind.label()).color(theme.playhead);
                            ui.label(kind.strong());
                            ui.label(RichText::new(&row.segment_id).color(theme.text).small());
                            let duration = RichText::new(&row.duration_label).color(theme.muted);
                            ui.label(duration.small());
                            if row.selected {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| row_buttons(ui, &mut command),
                                );
                            }
                        });
                    })
                    .response;
                if command.is_none() && response.interact(egui::Sense::click()).clicked() {
                    command = Some(SegmentCommand::Select(row.segment_id.clone()));
                }
                ui.add_space(6.0);
            }
        });
    command
}

const EMPTY_LIST_MESSAGE: &str = "Nema segmenata — označi source IN/OUT pa dodaj TON/OFF.";

fn row_buttons(ui: &mut egui::Ui, command: &mut Option<SegmentCommand>) {
    let small = |ui: &mut egui::Ui, text: &str, hint: &str| {
        ui.add(egui::Button::new(RichText::new(text).small()))
            .on_hover_text(hint)
            .clicked()
    };
    if small(ui, "Del", "Obriši segment") {
        *command = Some(SegmentCommand::DeleteSelected);
    }
    if small(ui, "Down", "Pomakni segment kasnije") {
        *command = Some(SegmentCommand::Move { up: false });
    }
    if small(ui, "Up", "Pomakni segment ranije") {
        *command = Some(SegmentCommand::Move { up: true });
    }
}

/// Paints the panel into the whole available rectangle.
pub fn show(ui: &mut egui::Ui, input: SegmentPanelInput<'_>) -> SegmentPanelAction {
    let style = SegmentPanelStyle::from_timeline(input.timeline_theme);
    let panel_rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(panel_rect, 0.0, style.surface);
    ui.painter().rect_stroke(
        panel_rect,
        0.0,
        egui::Stroke::new(1.0, style.border),
        egui::StrokeKind::Inside,
    );
    let content_rect = panel_rect.shrink(PANEL_MARGIN);
    let mut action = SegmentPanelAction::None;
    if !content_rect.is_positive() {
        return action;
    }
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(content_rect);
            header(ui, &input);
            ui.add_space(8.0);
            if input.segments.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.label(RichText::new(EMPTY_MESSAGE).color(style.muted));
                });
                return;
            }
            egui::ScrollArea::vertical()
                .id_salt("qnc_segment_panel_stack")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, ROW_GAP);
                    for row in &input.segments.rows {
                        let intent = paint_row(ui, row, &input);
                        if action == SegmentPanelAction::None {
                            action = intent;
                        }
                    }
                });
        },
    );
    action
}

fn header(ui: &mut egui::Ui, input: &SegmentPanelInput<'_>) {
    let style = SegmentPanelStyle::from_timeline(input.timeline_theme);
    let segments = input.segments;
    ui.horizontal(|ui| {
        ui.label(RichText::new("Segmenti").color(style.text).strong());
        if segments.is_empty() {
            return;
        }
        let playhead = input.playhead_frame.unwrap_or(0);
        let segment_frames = segments
            .segment_at(playhead)
            .or_else(|| segments.selected())
            .map(SegmentRow::duration_frames)
            .unwrap_or(0);
        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            value(ui, &segments.label(segments.total_frames), style.total);
            label(ui, "Trajanje", style.muted);
            ui.add_space(18.0);
            value(ui, &playhead.to_string(), style.playhead);
            label(ui, "frame", style.muted);
            value(ui, &segments.label(playhead), style.playhead);
            label(ui, "Playhead", style.muted);
            ui.add_space(18.0);
            value(ui, &segments.label(segment_frames), style.segment);
            label(ui, "Segment", style.muted);
        });
    });
}

fn label(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(
        RichText::new(text)
            .color(color)
            .size(HEADER_LABEL_SIZE)
            .strong(),
    );
}

fn value(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(
        RichText::new(text)
            .color(color)
            .size(HEADER_VALUE_SIZE)
            .strong(),
    );
}

/// Carrier, A1, picture, A2 and the segment span, as the v5 segment row.
fn layers() -> TimelineLayerFlags {
    let mut layers = TimelineLayerFlags::a1_v_a2();
    layers.virtual_spans = true;
    layers
}

fn paint_row(
    ui: &mut egui::Ui,
    row: &SegmentRow,
    input: &SegmentPanelInput<'_>,
) -> SegmentPanelAction {
    let duration = row.duration_frames().max(1);
    let mut state = TimelineProjection::new(0, duration).with_cue_enabled(true);
    if let Some(local) = input
        .playhead_frame
        .filter(|frame| (row.start_frame..row.end_frame).contains(frame))
    {
        state = state.with_playhead(local - row.start_frame);
    }
    let label = format!("{} {}", row.kind.label(), row.duration_label);
    let spans = [TimelineVirtualSpan {
        id: &row.segment_id,
        label: &label,
        start_frame: 0,
        end_frame: duration,
        has_base_video: row.kind.has_base_video(),
        selected: row.selected,
    }];
    let intent = qnc_timeline::show(
        ui,
        TimelineInput {
            state: &state,
            layers: layers(),
            metrics: TimelineMetrics::default(),
            theme: input.timeline_theme,
            expanded_audio: AudioLane::None,
            focus: TimelineFocusPaint::Playhead,
            show_lane_labels: row.start_frame == 0,
            shot_in_frame: 0,
            shot_out_frame: duration,
            draft_in_frame: 0,
            draft_out_frame: duration,
            draft_in_active: false,
            draft_out_active: false,
            a1_peaks: &[],
            a2_peaks: &[],
            a3_peaks: &[],
            a4_peaks: &[],
            virtual_spans: &spans,
            covers: &[],
            marker_slots: &[],
            markers: &[],
            base_video_blank: !row.kind.has_base_video(),
            filmstrip_background: None,
            video_background: None,
        },
    );
    match intent {
        TimelineIntent::SelectVirtual { id, .. } => SegmentPanelAction::SelectSegment(id),
        TimelineIntent::CueFrame(local) => {
            SegmentPanelAction::CueProgramFrame(row.start_frame + local.min(duration))
        }
        _ => SegmentPanelAction::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_row_shows_picture_audio_and_its_span() {
        let layers = layers();
        assert!(layers.carrier && layers.audio_a1 && layers.base_video && layers.audio_a2);
        assert!(layers.virtual_spans && layers.playhead);
        assert!(!layers.covers && !layers.markers && !layers.in_out);
    }
}

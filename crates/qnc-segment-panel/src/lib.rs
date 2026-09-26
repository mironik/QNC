//! Public "Segmenti" panel of an editorial form (QNC v5 `editorial/segment_panel.rs`,
//! `marker_cover_panel.rs` and `qnc_segment_timeline::show_program`).
//!
//! Passive paint only: a header with the timing of the program, one row per segment
//! with its M markers and M-M slots, the navigation bar and one overview row of the whole program. Every row is a
//! local projection of the same program axis, painted by the one public
//! `qnc-timeline`. The panel receives a prepared `SegmentsView` and returns what the
//! user asked for; it never reads a database, plays, seeks or decides the program.

mod bar;
mod list;

pub use list::show_segment_list;

use eframe::egui::{self, Color32, RichText, Vec2};
use qnc_program_segments::{SegmentCommand, SegmentRow, SegmentsView};
use qnc_timeline::{
    AudioLane, TimelineCoverSpan, TimelineFocusPaint, TimelineInput, TimelineIntent,
    TimelineLayerFlags, TimelineMarkerPin, TimelineMetrics, TimelineProjection, TimelineSlotSpan,
    TimelineTheme, TimelineVirtualSpan,
};

pub const MODULE_ID: &str = "qnc.module.segment-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const PANEL_MARGIN: f32 = 10.0;
const ROW_GAP: f32 = 3.0;
const BOTTOM_GAP: f32 = 4.0;
const PLAYLIST_INPUT_H: f32 = 128.0;
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

/// Paints the panel into the whole available rectangle.
pub fn show(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    timeline_theme: TimelineTheme,
) -> Option<SegmentCommand> {
    let style = SegmentPanelStyle::from_timeline(timeline_theme);
    let panel_rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(panel_rect, 0.0, style.surface);
    ui.painter().rect_stroke(
        panel_rect,
        0.0,
        egui::Stroke::new(1.0, style.border),
        egui::StrokeKind::Inside,
    );
    let content_rect = panel_rect.shrink(PANEL_MARGIN);
    let mut command = None;
    if !content_rect.is_positive() {
        return command;
    }
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.set_clip_rect(content_rect);
            header(ui, segments, style);
            ui.add_space(8.0);
            if segments.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(ui.available_height() * 0.35);
                    ui.label(RichText::new(EMPTY_MESSAGE).color(style.muted));
                });
                return;
            }
            let body = ui.available_rect_before_wrap();
            let playlist_top = body.bottom() - PLAYLIST_INPUT_H.min(body.height());
            let stack_bottom = (playlist_top - BOTTOM_GAP).max(body.top());
            let stack =
                egui::Rect::from_min_max(body.left_top(), egui::pos2(body.right(), stack_bottom));
            let playlist =
                egui::Rect::from_min_max(egui::pos2(body.left(), playlist_top), body.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(stack), |ui| {
                ui.set_clip_rect(stack);
                egui::ScrollArea::vertical()
                    .id_salt("qnc_segment_panel_stack")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, ROW_GAP);
                        for row in &segments.rows {
                            let row_command = paint_row(ui, segments, row, timeline_theme);
                            command = command.take().or(row_command);
                        }
                    });
            });
            ui.scope_builder(egui::UiBuilder::new().max_rect(playlist), |ui| {
                ui.set_clip_rect(playlist);
                ui.separator();
                command = command.take().or(bar::show(ui, segments, timeline_theme));
                ui.add_space(2.0);
                let total = segments.total_frames;
                // Every timeline row needs its own id space: `qnc-timeline` names its
                // click area, so rows sharing one id would lose their clicks.
                let overview = ui
                    .push_id("qnc_segment_program_overview", |ui| {
                        paint_program(ui, segments, (0, total), timeline_theme, true)
                    })
                    .inner;
                command = command.take().or(overview);
            });
        },
    );
    command
}

fn header(ui: &mut egui::Ui, segments: &SegmentsView, style: SegmentPanelStyle) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Segmenti").color(style.text).strong());
        if segments.is_empty() {
            return;
        }
        let playhead = segments.playhead.unwrap_or(0);
        let segment_frames = segments
            .segment_at(playhead)
            .or_else(|| segments.selected())
            .map(SegmentRow::duration_frames)
            .unwrap_or(0);
        ui.add_space(14.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            value(ui, &segments.timecode(segments.total_frames), style.total);
            label(ui, "Trajanje", style.muted);
            ui.add_space(18.0);
            value(ui, &playhead.to_string(), style.playhead);
            label(ui, "frame", style.muted);
            value(ui, &segments.timecode(playhead), style.playhead);
            label(ui, "Playhead", style.muted);
            ui.add_space(18.0);
            value(ui, &segments.timecode(segment_frames), style.segment);
            label(ui, "Segment", style.muted);
        });
    });
}

fn label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let text = RichText::new(text).color(color).size(HEADER_LABEL_SIZE);
    ui.label(text.strong());
}

fn value(ui: &mut egui::Ui, text: &str, color: Color32) {
    let text = RichText::new(text).color(color).size(HEADER_VALUE_SIZE);
    ui.label(text.strong());
}

/// Carrier, A1, picture, A2, segment spans, markers and slots (v5 `segment_layers`).
fn layers() -> TimelineLayerFlags {
    TimelineLayerFlags::a1_v_a2().with_overlays()
}

/// One segment row: the program inside its window.
fn paint_row(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    row: &SegmentRow,
    theme: TimelineTheme,
) -> Option<SegmentCommand> {
    let window = (row.start_frame, row.end_frame);
    ui.push_id(("qnc_segment_row", &row.segment_id), |ui| {
        paint_program(ui, segments, window, theme, row.start_frame == 0)
    })
    .inner
}

/// The program between `start` and `end`, as one `qnc-timeline` row.
fn paint_program(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    (start, end): (u64, u64),
    theme: TimelineTheme,
    lane_labels: bool,
) -> Option<SegmentCommand> {
    let duration = end.saturating_sub(start).max(1);
    let local = |frame: u64| frame.clamp(start, end) - start;
    let mut state = TimelineProjection::new(0, duration).with_cue_enabled(true);
    if let Some(frame) = segments
        .playhead
        .filter(|frame| (start..=end).contains(frame))
    {
        state = state.with_playhead(local(frame));
    }
    let labels = segments
        .rows
        .iter()
        .map(|row| format!("{} {}", row.kind.label(), row.duration_label))
        .collect::<Vec<_>>();
    let spans = segments
        .rows
        .iter()
        .zip(&labels)
        .filter(|(row, _)| row.end_frame > start && row.start_frame < end)
        .map(|(row, label)| TimelineVirtualSpan {
            id: &row.segment_id,
            label,
            start_frame: local(row.start_frame),
            end_frame: local(row.end_frame),
            has_base_video: row.kind.has_base_video(),
            selected: row.selected,
        })
        .collect::<Vec<_>>();
    let slots = segments
        .slots
        .iter()
        .filter(|slot| slot.end_frame > start && slot.start_frame < end)
        .map(|slot| TimelineSlotSpan {
            id: &slot.slot_id,
            start_frame: local(slot.start_frame),
            end_frame: local(slot.end_frame),
            has_cover: slot.has_cover,
            selected: slot.selected,
        })
        .collect::<Vec<_>>();
    let covers = segments
        .covers
        .iter()
        .filter(|cover| cover.end_frame > start && cover.start_frame < end)
        .map(|cover| TimelineCoverSpan {
            id: &cover.cover_id,
            start_frame: local(cover.start_frame),
            end_frame: local(cover.end_frame),
            selected: cover.selected,
            pending: false,
        })
        .collect::<Vec<_>>();
    let markers = segments
        .markers
        .iter()
        // v5 `local_marker_frame`: a marker on a segment border is drawn at the end
        // of the earlier row; only the first row shows frame 0.
        .filter(|pin| (start..=end).contains(&pin.frame) && (pin.frame != start || start == 0))
        .map(|pin| TimelineMarkerPin {
            id: &pin.marker_id,
            frame: local(pin.frame),
        })
        .collect::<Vec<_>>();
    let no_picture = segments
        .segment_at(start)
        .is_some_and(|row| !row.kind.has_base_video() && row.end_frame >= end);
    let intent = qnc_timeline::show(
        ui,
        TimelineInput {
            state: &state,
            layers: layers(),
            metrics: TimelineMetrics::default(),
            theme,
            expanded_audio: AudioLane::None,
            focus: TimelineFocusPaint::Playhead,
            show_lane_labels: lane_labels,
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
            covers: &covers,
            marker_slots: &slots,
            markers: &markers,
            base_video_blank: no_picture,
            filmstrip_background: None,
            video_background: None,
        },
    );
    // Story rows only: a dragged M marker follows the pointer as a draft (Enter confirms).
    if let Some((marker_id, frame)) = qnc_timeline::take_marker_drag(ui.ctx()) {
        let frame = start + frame.min(duration);
        return Some(SegmentCommand::DragMarker { marker_id, frame });
    }
    match intent {
        TimelineIntent::SelectMarker { id, .. } => Some(SegmentCommand::SelectMarker(id)),
        // v5 `program_intent_from_timeline_interact`: a click keeps its frame.
        TimelineIntent::SelectMarkerSlot { id, frame } => Some(SegmentCommand::SelectSlot {
            slot_id: id,
            frame: start + frame.min(duration),
        }),
        // Ctrl+click takes the cover for Delete (Ctrl+ selects); a plain click selects it.
        TimelineIntent::SelectCover { id, frame } => {
            let cover_id = id;
            let frame = start + frame.min(duration);
            Some(if ui.input(|input| input.modifiers.command) {
                SegmentCommand::TakeCover { cover_id, frame }
            } else {
                SegmentCommand::SelectCover { cover_id, frame }
            })
        }
        TimelineIntent::SelectVirtual { frame, .. } => {
            Some(SegmentCommand::Cue(start + frame.min(duration)))
        }
        TimelineIntent::CueFrame(frame) => Some(SegmentCommand::Cue(start + frame.min(duration))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_row_shows_picture_audio_spans_markers_and_slots() {
        let layers = layers();
        assert!(layers.carrier && layers.audio_a1 && layers.base_video && layers.audio_a2);
        assert!(layers.virtual_spans && layers.markers && layers.marker_slots && layers.playhead);
        assert!(!layers.in_out);
    }
}

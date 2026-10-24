//! Public qnc_ui_text::text("story.ui.segments") panel of an editorial form (QNC v5 `editorial/segment_panel.rs`,
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
mod cover_length;
mod header;
mod program;
use header::*;
use program::*;
pub use cover_length::{cover_fill, CoverLengthBand};

use qnc_action_event::ActionEvent;
use eframe::egui::{self, Color32, RichText, Vec2};
use qnc_program_view::{SegmentRow, SegmentsView};
use qnc_timeline::{
    AudioLane, TimelineCoverSpan, TimelineFocusPaint, TimelineInput,
    TimelineLayerFlags, TimelineMarkerPin, TimelineMetrics, TimelinePicture, TimelineProjection, TimelineSlotSpan,
    TimelineTheme, TimelineVirtualSpan,
};

pub const MODULE_ID: &str = "qnc.module.segment-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const PANEL_MARGIN: f32 = 10.0;
const ROW_GAP: f32 = 3.0;
const BOTTOM_GAP: f32 = 4.0;
/// Above the program row: the separator, the bar and the gap under it.
const PLAYLIST_BAR_H: f32 = 18.0 + bar::COMPACT_CTRL_H;
const HEADER_LABEL_SIZE: f32 = 17.0;
const HEADER_VALUE_SIZE: f32 = 22.0;

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

/// The panel name of the segment panel in the layout contracts (where a click happened).
pub const PANEL: &str = "segments";

/// The enlarged slot in the whole available rectangle (user 2026-10-21: a zoom of part of the
/// Wrap row, in the place of the source timeline): the same Wrap row of the same program, its
/// window only narrower; a pointer there is this panel's action, as on its own rows. `None`
/// when no slot is enlarged.
pub fn show_zoom(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    timeline_theme: TimelineTheme,
    lengths: &[CoverLengthBand],
) -> Option<Option<ActionEvent>> {
    let window = segments.zoom?;
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, timeline_theme.background);
    let shown = ui.push_id("qnc_segment_zoom", |ui| paint_program(ui, segments, window, timeline_theme, true, lengths, true)).inner;
    Some(shown)
}

/// The enlarged slot as a body a dock draws under its header in place of its timeline (user
/// 2026-10-22: the dock's buttons stay); `None` when no slot is enlarged.
pub fn zoom_body<'a>(
    segments: &'a SegmentsView,
    timeline_theme: TimelineTheme,
    lengths: &'a [CoverLengthBand],
) -> Option<impl FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a> {
    segments.zoom.map(|_| move |ui: &mut egui::Ui| show_zoom(ui, segments, timeline_theme, lengths).flatten())
}

/// Paints the panel into the whole available rectangle. What it gives is a system action
/// on this panel (user 2026-10-18): a pointer on a Wrap row, a button of its bar.
pub fn show(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    timeline_theme: TimelineTheme,
    lengths: &[CoverLengthBand],
    levels: [Option<f32>; 2],
) -> Option<ActionEvent> {
    // A refused take, on top of the screen so it is noticed (user 2026-10-21).
    qnc_alert::show(ui.ctx(), segments.alert.as_ref(), timeline_theme.lane_silent, timeline_theme.text);
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
                    ui.label(RichText::new(qnc_ui_text::text("story.ui.no_segments")).color(style.muted));
                });
                return;
            }
            let body = ui.available_rect_before_wrap();
            // The bar and a whole program row, its A2 band included (user 2026-10-15).
            let overview = TimelineMetrics::default().outer_height(layers(segments.video_compact), AudioLane::None);
            let playlist_top = body.bottom() - (PLAYLIST_BAR_H + overview).min(body.height());
            let stack_bottom = (playlist_top - BOTTOM_GAP).max(body.top());
            // The rows' scroll bar fills the panel's right margin (user 2026-10-17): the rows
            // keep their width.
            let stack = egui::Rect::from_min_max(body.left_top(), egui::pos2(body.right() + PANEL_MARGIN, stack_bottom));
            let playlist =
                egui::Rect::from_min_max(egui::pos2(body.left(), playlist_top), body.max);
            ui.scope_builder(egui::UiBuilder::new().max_rect(stack), |ui| {
                ui.set_clip_rect(stack);
                // The wheel over a Wrap row moves its playhead (user 2026-10-16), so the rows
                // scroll by a solid bar as wide as the panel's right margin, always shown and
                // easy to see (user 2026-10-17): the wheel over it or a drag of it.
                let mut scroll = egui::style::ScrollStyle::solid();
                (scroll.bar_width, scroll.bar_inner_margin, scroll.bar_outer_margin) = (PANEL_MARGIN, 0.0, 0.0);
                ui.style_mut().spacing.scroll = scroll;
                let visuals = ui.visuals_mut();
                visuals.extreme_bg_color = style.border;
                visuals.widgets.inactive.bg_fill = style.muted;
                visuals.widgets.hovered.bg_fill = style.text;
                visuals.widgets.active.bg_fill = style.playhead;
                egui::ScrollArea::vertical()
                    .id_salt("qnc_segment_panel_stack")
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(0.0, ROW_GAP);
                        // A newly chosen segment (Up/Down, user 2026-10-17) is scrolled into view.
                        let shown_key = egui::Id::new("qnc_segment_panel_scrolled_to");
                        let shown: Option<String> = ui.ctx().data(|data| data.get_temp(shown_key));
                        for (window, row) in segments.wrap_rows() {
                            let top = ui.cursor().top();
                            let row_command = paint_row(ui, segments, row, window, timeline_theme, lengths);
                            if row.selected && shown.as_deref() != Some(row.segment_id.as_str()) {
                                let rect = egui::Rect::from_min_max(egui::pos2(stack.left(), top), egui::pos2(stack.right(), ui.cursor().top()));
                                ui.scroll_to_rect(rect, None);
                                ui.ctx().data_mut(|data| data.insert_temp(shown_key, row.segment_id.clone()));
                            }
                            if command.is_some() && row_command.is_some() && qnc_dev_diagnostics::player_diagnostics_enabled() {
                                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("segment-row command lost {row_command:?} to {command:?}"));
                            }
                            command = command.take().or(row_command);
                        }
                    });
            });
            ui.scope_builder(egui::UiBuilder::new().max_rect(playlist), |ui| {
                ui.set_clip_rect(playlist);
                ui.separator();
                command = command.take().or(bar::show(ui, segments, timeline_theme, levels));
                ui.add_space(2.0);
                let total = segments.total_frames;
                // Every timeline row needs its own id space: `qnc-timeline` names its
                // click area, so rows sharing one id would lose their clicks.
                let overview = ui
                    .push_id("qnc_segment_program_overview", |ui| {
                        paint_program(ui, segments, (0, total), timeline_theme, true, lengths, false)
                    })
                    .inner
                    // A double click on the program row enlarges the whole program (user 2026-10-24).
                    .map(|mut event| {
                        if event.action_id == "wrap_zoom" {
                            event.action_id = "program_zoom".into();
                        }
                        event
                    });
                command = command.take().or(overview);
            });
        },
    );
    command
}

/// Carrier, A1, picture, A2, segment spans, markers and slots (v5 `segment_layers`); A1
/// and A2 in the dark bands above and below the light middle (user 2026-10-15).
/// With `compact` the light middle is narrow and shows no filmstrip (Ctrl+click on V).
fn layers(compact: bool) -> TimelineLayerFlags {
    TimelineLayerFlags::a1_v_a2().with_overlays().with_audio_bands().with_compact_video(compact)
}

/// One segment row: the program inside its window.
fn paint_row(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    row: &SegmentRow,
    window: (u64, u64),
    theme: TimelineTheme,
    lengths: &[CoverLengthBand],
) -> Option<ActionEvent> {
    ui.push_id(("qnc_segment_row", &row.segment_id), |ui| {
        paint_program(ui, segments, window, theme, window.0 == 0, lengths, false)
    })
    .inner
}


#[cfg(test)]
mod tests;

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

/// Paints the panel into the whole available rectangle. What it gives is a system action
/// on this panel (user 2026-10-18): a pointer on a Wrap row, a button of its bar.
pub fn show(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    timeline_theme: TimelineTheme,
    lengths: &[CoverLengthBand],
    levels: [Option<f32>; 2],
) -> Option<ActionEvent> {
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
                        for row in &segments.rows {
                            let top = ui.cursor().top();
                            let row_command = paint_row(ui, segments, row, timeline_theme, lengths);
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
                        paint_program(ui, segments, (0, total), timeline_theme, true, lengths)
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
        ui.label(RichText::new(qnc_ui_text::text("story.ui.segments")).color(style.text).strong());
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
            label(ui, qnc_ui_text::text("source.duration"), style.muted);
            ui.add_space(18.0);
            value(ui, &playhead.to_string(), style.playhead);
            label(ui, qnc_ui_text::text("story.ui.frame"), style.muted);
            value(ui, &segments.timecode(playhead), style.playhead);
            label(ui, qnc_ui_text::text("story.ui.playhead"), style.muted);
            ui.add_space(18.0);
            value(ui, &segments.timecode(segment_frames), style.segment);
            label(ui, qnc_ui_text::text("pool.tab.segment"), style.muted);
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
    theme: TimelineTheme,
    lengths: &[CoverLengthBand],
) -> Option<ActionEvent> {
    let window = (row.start_frame, row.end_frame);
    ui.push_id(("qnc_segment_row", &row.segment_id), |ui| {
        paint_program(ui, segments, window, theme, row.start_frame == 0, lengths)
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
    lengths: &[CoverLengthBand],
) -> Option<ActionEvent> {
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
    // A saved cover shows its whole length (colour and seconds:frames), also where a
    // row shows only part of it (user 2026-10-12).
    let shown = segments
        .covers
        .iter()
        .filter(|cover| cover.end_frame > start && cover.start_frame < end)
        .collect::<Vec<_>>();
    let rate = segments.timebase.unwrap_or((0, 1));
    let labels = shown
        .iter()
        .map(|cover| qnc_program_view::duration_label(cover.end_frame - cover.start_frame, rate.0, rate.1))
        .collect::<Vec<_>>();
    // The cover a Sync is writing, as it grows (user 2026-10-18).
    let draft = segments.sync_draft.filter(|(from, to)| *to > start && *from < end);
    let draft_label = draft.map(|(from, to)| qnc_program_view::duration_label(to - from, rate.0, rate.1));
    let draft_span = draft.zip(draft_label.as_deref()).map(|((from, to), label)| TimelineCoverSpan {
        id: "sync-draft",
        start_frame: local(from),
        end_frame: local(to),
        selected: true,
        pending: true,
        fill: cover_fill(to - from, segments.timebase, lengths),
        label: Some(label),
    });
    let covers = shown
        .iter()
        .zip(&labels)
        .map(|(cover, label)| TimelineCoverSpan {
            id: &cover.cover_id,
            start_frame: local(cover.start_frame),
            end_frame: local(cover.end_frame),
            // The strip of a cover shows while it or its slot is chosen (drawing only).
            selected: cover.selected || segments.selected_slot().is_some_and(|slot| slot.slot_id == cover.slot_id),
            pending: false,
            fill: cover_fill(cover.end_frame - cover.start_frame, segments.timebase, lengths),
            label: Some(label.as_str()),
        })
        .chain(draft_span)
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
            taken: pin.taken,
            locked: pin.locked,
        })
        .collect::<Vec<_>>();
    // v5 `local_peaks_for_row`: this row's part of the program wave.
    let row_wave = |program: &[f32]| qnc_program_waveform::row_peaks(program, segments.total_frames, start, end);
    let (a1, a2) = (row_wave(&segments.peaks.a1), row_wave(&segments.peaks.a2));
    // The trim playheads of the edits being moved, on the row that draws their marker.
    let trim_labels: Vec<(u64, String)> = segments
        .trim_frames
        .iter()
        .filter(|frame| (start..=end).contains(*frame) && (**frame != start || start == 0))
        .map(|frame| (local(*frame), segments.timecode(*frame)))
        .collect();
    let trim_playheads: Vec<(u64, &str)> = trim_labels.iter().map(|(frame, label)| (*frame, label.as_str())).collect();
    // This row's part of the program filmstrip (user 2026-10-15).
    let row_pictures = qnc_program_filmstrip::row_pictures(&segments.pictures, start, end);
    let pictures: Vec<TimelinePicture<'_>> = row_pictures
        .iter()
        .filter_map(|picture| {
            Some(TimelinePicture {
                start_frame: picture.program_start,
                end_frame: picture.program_end,
                frame: picture.strip.frames.get(picture.frame)?,
                strip: &picture.strip,
            })
        })
        .collect();
    let no_picture = segments
        .segment_at(start)
        .is_some_and(|row| !row.kind.has_base_video() && row.end_frame >= end);
    // The sound of the segment or cover taken on this row (user 2026-10-16): the channel
    // column and the lane button show its draft, and its wave follows the drafted level at
    // once (live 2026-10-16: the level was not seen until Enter). The row's wave is drawn
    // at the stored level, so the draft gives the difference.
    let part_choice = |lane: u8| {
        let stored = if lane == 0 {
            segments.part_sound.as_ref().and_then(|part| {
                segments.rows.iter().find(|row| row.segment_id == part.id && row.start_frame == start).map(|row| row.a1_level_db)
            })
        } else {
            segments.part_sound.as_ref().and_then(|part| {
                segments.covers.iter().find(|cover| cover.cover_id == part.id && (start..end).contains(&cover.start_frame)).map(|cover| cover.a2_level_db)
            })
        };
        segments.part_sound.as_ref().filter(|part| part.lane == lane).zip(stored).map(|(part, stored)| {
            let now = qnc_timeline::Lane { channel: part.channel, off: part.off, level_db: part.level_db };
            qnc_timeline::drafted_choice(now, part.channels, true, stored)
        })
    };
    let intent = qnc_timeline::show(
        ui,
        TimelineInput {
            state: &state,
            layers: layers(segments.video_compact),
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
            a1_peaks: &a1,
            a2_peaks: &a2,
            a3_peaks: &[],
            a4_peaks: &[],
            virtual_spans: &spans,
            covers: &covers,
            marker_slots: &slots,
            markers: &markers,
            base_video_blank: no_picture,
            filmstrip_background: None,
            video_background: None,
            a1_channel: part_choice(0),
            a2_channel: part_choice(1),
            wave_over_video: None,
            trim_playheads: &trim_playheads,
            pictures: &pictures,
            // Something taken on the row: a left click keeps it first (the mouse map).
            holding: !segments.trim_frames.is_empty() || segments.part_sound.is_some(),
        },
    );
    // A pointer on the row is the action the mouse map gives it on this panel (piece
    // `qnc-mouse-map`, contracts/input/mouse.json; user rule 2026-10-20): the panel decides
    // no click itself.
    let row_frame = |frame: u64| start + frame.min(duration);
    qnc_timeline::take_marker_drag_event(ui.ctx(), PANEL, &row_frame).or_else(|| qnc_timeline::event(&intent, PANEL, &row_frame))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_segment_row_shows_picture_audio_spans_markers_and_slots() {
        let layers = layers(false);
        assert!(layers.carrier && layers.audio_a1 && layers.base_video && layers.audio_a2);
        assert!(layers.virtual_spans && layers.markers && layers.marker_slots && layers.playhead);
        assert!(!layers.in_out);
    }
}

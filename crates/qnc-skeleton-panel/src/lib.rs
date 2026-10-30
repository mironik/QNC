//! The skeleton panel (user 2026-10-31, Media Assist Video: "every SubSegment in its own
//! row"): the blocks of the story in reading order, one row each, every row its parts on the
//! one public `qnc-timeline`. A click on a row opens that block as a SubSegment in place of the
//! source timeline, where its edges, pauses, joins and A1 are edited with the SubSegment's own
//! keys; a recorded OFF is the part of its take it uses. Send to Story puts the skeleton at the
//! end of the story. Passive: it paints the script's view and gives catalog actions; it never
//! plays, reads or writes.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_script::{seconds_label, ScriptView, SkeletonRow, OPEN_SUBSEGMENT, PANEL, SEND_STORY};
use qnc_timeline::{
    AudioLane, TimelineFocusPaint, TimelineInput, TimelineIntent, TimelineLayerFlags, TimelineMetrics, TimelineProjection, TimelineTheme,
    TimelineVirtualSpan,
};

pub const MODULE_ID: &str = "qnc.module.skeleton-panel";

/// Paints the skeleton over the whole of `ui`.
pub fn show(ui: &mut egui::Ui, view: &ScriptView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let mut inner = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink(8.0)).layout(egui::Layout::top_down(egui::Align::Min)));
    inner.set_clip_rect(rect.intersect(ui.clip_rect()));
    let mut asked = None;
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    let muted = |text: &str| RichText::new(text).color(theme.text_muted).size(theme.font_ui);
    inner.horizontal(|ui| {
        ui.label(RichText::new(qnc_ui_text::text("skeleton.title")).color(theme.text).strong());
        let total: f64 = view.skeleton.iter().filter(|row| row.parts.is_some()).map(|row| row.seconds).sum();
        ui.label(muted(&seconds_label(total)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let ready = view.skeleton.iter().any(|row| row.parts.is_some());
            if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("script.send_story"), ready, false, style).on_hover_text(qnc_ui_text::text("script.send_story_hint")).clicked() {
                asked = Some(ActionEvent::key(SEND_STORY).at(PANEL));
            }
        });
    });
    if !view.message.is_empty() {
        inner.label(RichText::new(&view.message).color(theme.danger).size(theme.font_ui));
    }
    if view.skeleton.is_empty() {
        inner.label(muted(qnc_ui_text::text("skeleton.empty")));
        return asked;
    }
    let timeline = qnc_theme_styles::timeline(theme);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut inner, |ui| {
        for row in &view.skeleton {
            if let Some(event) = paint_row(ui, row, theme, timeline) {
                asked = Some(event);
            }
            ui.add_space(6.0);
        }
    });
    asked
}

/// One block: its mark, length and words, then its parts on its own axis.
fn paint_row(ui: &mut egui::Ui, row: &SkeletonRow, theme: &qnc_desktop_theme::Theme, timeline: TimelineTheme) -> Option<ActionEvent> {
    let open = || ActionEvent::key(OPEN_SUBSEGMENT).with_target(row.block_id.to_string()).at(PANEL);
    let mark = RichText::new(&row.mark).strong().color(if row.open { theme.focus } else { theme.text });
    let words: String = row.text.split_whitespace().take(12).collect::<Vec<_>>().join(" ");
    let mut asked = None;
    ui.horizontal(|ui| {
        if ui.add(egui::Label::new(mark).sense(egui::Sense::click())).clicked() && row.parts.is_some() {
            asked = Some(open());
        }
        ui.label(RichText::new(seconds_label(row.seconds)).color(theme.text_muted).size(theme.font_ui));
        ui.label(RichText::new(words).color(theme.text_muted).size(theme.font_ui));
    });
    let Some(parts) = &row.parts else {
        ui.label(RichText::new(qnc_ui_text::text("skeleton.not_recorded")).color(theme.danger).size(theme.font_ui));
        return asked;
    };
    let duration = parts.total_frames.max(1);
    let state = TimelineProjection::new(0, duration).with_cue_enabled(true);
    let labels: Vec<String> = parts.parts.iter().map(|part| format!("{} {}", part.clip_name, part.length)).collect();
    let ids: Vec<String> = (0..parts.parts.len()).map(|index| format!("part-{index}")).collect();
    let spans: Vec<TimelineVirtualSpan<'_>> = parts
        .parts
        .iter()
        .zip(&labels)
        .zip(&ids)
        .map(|((part, label), id)| TimelineVirtualSpan { id, label, start_frame: part.start, end_frame: part.end, has_base_video: true, selected: row.open })
        .collect();
    let intent = ui
        .push_id(("qnc_skeleton_row", row.block_id), |ui| {
            qnc_timeline::show(
                ui,
                TimelineInput {
                    state: &state,
                    layers: TimelineLayerFlags::a1_v_a2().with_overlays().with_audio_bands(),
                    metrics: TimelineMetrics::default(),
                    theme: timeline,
                    expanded_audio: AudioLane::None,
                    focus: TimelineFocusPaint::Playhead,
                    show_lane_labels: false,
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
                    base_video_blank: false,
                    filmstrip_background: None,
                    video_background: None,
                    a1_channel: None,
                    a2_channel: None,
                    wave_over_video: None,
                    trim_playheads: &[],
                    a1_splits: &[],
                    pictures: &[],
                    holding: false,
                },
            )
        })
        .inner;
    // A click anywhere on the row opens its block as the SubSegment to edit.
    if matches!(intent, TimelineIntent::CueFrame(_) | TimelineIntent::SelectVirtual { .. }) {
        asked = Some(open());
    }
    asked
}

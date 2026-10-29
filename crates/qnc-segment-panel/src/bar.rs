//! The navigation and edit bar under the segment rows (v5 `marker_cover_panel.rs`):
//! M marker, Cover slot, Overwrite on the left; previous/next segment, slot and
//! marker around the program start in the middle; Undo, Redo and Export HI-res on the right.
//! Cover slot needs a selected empty slot, Overwrite a selected slot or cover (v5
//! `quick_cover_target`, `overwrite_cover_target`). Sync/B-roll is a switch with
//! Pokrivalice on the source dock.

use eframe::egui::{self, PointerButton, Pos2, Rect, RichText, Sense, Vec2};
use qnc_action_event::ActionEvent;
use qnc_program_view::SegmentsView;
use qnc_timeline::TimelineTheme;

pub(crate) const COMPACT_CTRL_H: f32 = 22.0;
const EDIT_ACTIONS_W: f32 = 250.0;
const RIGHT_ACTIONS_W: f32 = 210.0;
const EDIT_ACTION_GAP: f32 = 6.0;
const CONTROL_GROUP_GAP: f32 = 10.0;
const TRANSPORT_BTN_W: f32 = 30.0;
const TRANSPORT_GAP: f32 = 5.0;
const TRANSPORT_CONTROLS_W: f32 = TRANSPORT_BTN_W * 7.0 + TRANSPORT_GAP * 6.0;
const FONT_UI: f32 = 14.0;

/// The middle buttons (previous/next segment, slot, marker, program start) are hidden: a
/// level meter is in their place (user 2026-10-16: hidden, not deleted).
const SHOW_NAVIGATION: bool = false;
/// The width of the NORM button beside the level meter.
const NORM_W: f32 = 52.0;

pub(crate) fn show(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    theme: TimelineTheme,
    levels: [Option<f32>; 2],
) -> Option<ActionEvent> {
    let mut command: Option<&'static str> = None;
    let (row, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), COMPACT_CTRL_H),
        Sense::hover(),
    );
    if row.width() < TRANSPORT_CONTROLS_W {
        return None;
    }
    let x = row.center().x - TRANSPORT_CONTROLS_W * 0.5;
    let transport = Rect::from_min_size(
        Pos2::new(x, row.min.y),
        Vec2::new(TRANSPORT_CONTROLS_W, COMPACT_CTRL_H),
    );
    if !SHOW_NAVIGATION {
        // NORM right of the meter (user 2026-10-25): lit while the preview normalises the
        // loudness; a click is the N key.
        let norm = Rect::from_min_max(Pos2::new(transport.max.x - NORM_W, transport.min.y), transport.max);
        let meter = Rect::from_min_max(transport.min, Pos2::new(norm.min.x - EDIT_ACTION_GAP, transport.max.y));
        let style = qnc_level_meter::MeterStyle { background: theme.background, border: theme.border, text: theme.muted };
        let lanes = [("A1", levels[0]), ("A2", levels[1])];
        qnc_level_meter::show(ui, ui.make_persistent_id("qnc_segment_level_meter"), meter.shrink2(Vec2::new(0.0, 1.0)), &lanes, style);
        in_rect(ui, norm, egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.spacing_mut().button_padding = Vec2::new(6.0, 1.0);
            // The look of the B-roll | Sync/B-roll switch (user 2026-10-25): on in the focus colour.
            let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: COMPACT_CTRL_H };
            let button = qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("story.ui.normalized"), true, segments.normalize, style);
            if clicked(button.on_hover_text(qnc_ui_text::text("story.ui.normalize_hint"))) {
                command = Some("toggle_loudness_normalize"); // the N key
            }
        });
    }
    in_rect(
        ui,
        transport,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            if !SHOW_NAVIGATION {
                return;
            }
            ui.spacing_mut().item_spacing.x = TRANSPORT_GAP;
            let icons: [(&str, &str, Option<&'static str>); 7] = [
                (
                    "⏮",
                    qnc_ui_text::text("story.nav.previous_segment"),
                    Some("step_prev_segment"),
                ),
                (
                    "",
                    qnc_ui_text::text("story.nav.previous_slot"),
                    Some("step_prev_marker_slot"),
                ),
                (
                    "⚑",
                    qnc_ui_text::text("story.nav.previous_marker"),
                    Some("program_prev_marker"),
                ),
                (
                    "🏠",
                    qnc_ui_text::text("story.nav.program_start"),
                    Some("playlist_input_start"),
                ),
                (
                    "⚑",
                    qnc_ui_text::text("story.nav.next_marker"),
                    Some("program_next_marker"),
                ),
                (
                    "",
                    qnc_ui_text::text("story.nav.next_slot"),
                    Some("step_next_marker_slot"),
                ),
                (
                    "⏭",
                    qnc_ui_text::text("story.nav.next_segment"),
                    Some("step_next_segment"),
                ),
            ];
            for (icon, hint, target) in icons {
                if clicked(icon_button(ui, icon, hint, theme)) {
                    command = target;
                }
            }
        },
    );
    let edit_right = transport.min.x - CONTROL_GROUP_GAP;
    if edit_right - row.min.x >= 120.0 {
        let width = (edit_right - row.min.x).min(EDIT_ACTIONS_W);
        let edit = Rect::from_min_max(row.min, Pos2::new(row.min.x + width, row.max.y));
        in_rect(
            ui,
            edit,
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = EDIT_ACTION_GAP;
                ui.spacing_mut().button_padding = Vec2::new(6.0, 1.0);
                if clicked(action_button(ui, qnc_ui_text::text("story.ui.m_marker"), true, theme)) {
                    command = Some("add_marker"); // the M key
                }
                let quick = segments.quick_cover_slot().is_ok();
                if clicked(flash_button(ui, qnc_ui_text::text("story.ui.cover_slot"), quick, flashing(ui, segments, false), theme)) {
                    command = Some("quick_overwrite_cover"); // the B key
                }
                let overwrite = segments.overwrite_cover_slot().is_ok();
                if clicked(flash_button(ui, qnc_ui_text::text("story.ui.overwrite"), overwrite, flashing(ui, segments, true), theme)) {
                    command = Some("overwrite_cover"); // the Overwrite key
                }
            },
        );
    }
    let right_left = transport.max.x + CONTROL_GROUP_GAP;
    if row.max.x - right_left >= 110.0 {
        let width = (row.max.x - right_left).min(RIGHT_ACTIONS_W);
        let right = Rect::from_min_max(Pos2::new(row.max.x - width, row.min.y), row.max);
        in_rect(
            ui,
            right,
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.spacing_mut().button_padding = Vec2::new(6.0, 1.0);
                // Export of the story (user 2026-10-08: moved here from under the monitor, where
                // Sync/B-roll was; that went to the source dock as a switch with Pokrivalice).
                // It opens the box of the project's export settings (user 2026-10-24).
                if clicked(action_button(ui, qnc_ui_text::text("story.export_hires"), !segments.is_empty(), theme)) {
                    command = Some("export_open");
                }
                // Right to left: Redo, then Undo, left of Export HI-res.
                for (label, target) in [(qnc_ui_text::text("story.ui.redo"), "redo_object"), (qnc_ui_text::text("story.ui.undo"), "undo_object")] {
                    let enabled = segments.action_enabled(target);
                    if clicked(action_button(ui, label, enabled, theme)) {
                        command = Some(target);
                    }
                }
            },
        );
    }
    command.map(|action_id| ActionEvent::key(action_id).at(crate::PANEL))
}

fn in_rect(ui: &mut egui::Ui, rect: Rect, layout: egui::Layout, add: impl FnOnce(&mut egui::Ui)) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect).layout(layout), add);
}

/// Space and Home belong to the keyboard catalog; the bar never keeps focus.
fn clicked(response: egui::Response) -> bool {
    if response.has_focus() {
        response.surrender_focus();
    }
    response.clicked_by(PointerButton::Primary)
}

/// How long Cover slot or Overwrite stays lit after it was asked for.
const FLASH: std::time::Duration = std::time::Duration::from_millis(400);

/// Whether Cover slot (`false`) or Overwrite (`true`) was asked for a moment ago, by
/// click or key; keeps repainting until the flash is over.
fn flashing(ui: &egui::Ui, segments: &SegmentsView, overwrite: bool) -> bool {
    match segments.cover_pressed {
        Some((pressed, at)) if pressed == overwrite && at.elapsed() < FLASH => {
            ui.ctx().request_repaint_after(FLASH.saturating_sub(at.elapsed()));
            true
        }
        _ => false,
    }
}

/// An action button lit with the focus colour while `flash`.
fn flash_button(ui: &mut egui::Ui, label: &str, enabled: bool, flash: bool, theme: TimelineTheme) -> egui::Response {
    if !flash {
        return action_button(ui, label, enabled, theme);
    }
    let button = egui::Button::new(RichText::new(label).color(theme.text).size(FONT_UI))
        .min_size(Vec2::new(0.0, COMPACT_CTRL_H))
        .fill(theme.focus)
        .stroke(egui::Stroke::new(1.0, theme.focus));
    ui.add_enabled(enabled, button)
}

fn action_button(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    theme: TimelineTheme,
) -> egui::Response {
    let button = egui::Button::new(RichText::new(label).color(theme.text).size(FONT_UI))
        .min_size(Vec2::new(0.0, COMPACT_CTRL_H))
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::new(1.0, theme.border));
    ui.add_enabled(enabled, button)
}

/// Icon button; an empty icon paints the small slot rectangle of v5.
fn icon_button(ui: &mut egui::Ui, icon: &str, hint: &str, theme: TimelineTheme) -> egui::Response {
    let response = ui.add(
        egui::Button::new(RichText::new(icon).color(theme.text).size(18.0))
            .min_size(Vec2::new(TRANSPORT_BTN_W, COMPACT_CTRL_H))
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, theme.border)),
    );
    if icon.is_empty() && ui.is_rect_visible(response.rect) {
        let color = if response.hovered() {
            theme.text
        } else {
            theme.muted
        };
        let slot = Rect::from_center_size(response.rect.center(), Vec2::new(14.0, 5.0));
        ui.painter().rect_stroke(
            slot,
            0.0,
            egui::Stroke::new(1.4, color),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_text(hint)
}

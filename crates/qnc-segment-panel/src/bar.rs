//! The navigation and edit bar under the segment rows (v5 `marker_cover_panel.rs`):
//! M marker, Cover slot, Overwrite on the left; previous/next segment, slot and
//! marker around the program start in the middle; Sync/B-roll on the right.
//! Cover slot needs a selected empty slot, Overwrite a selected slot or cover (v5
//! `quick_cover_target`, `overwrite_cover_target`); Sync/B-roll switches the Sync
//! capture on and off and shows when it is on.

use eframe::egui::{self, PointerButton, Pos2, Rect, RichText, Sense, Vec2};
use qnc_program_segments::{SegmentCommand, SegmentsView};
use qnc_timeline::TimelineTheme;

const COMPACT_CTRL_H: f32 = 22.0;
const EDIT_ACTIONS_W: f32 = 250.0;
const RIGHT_ACTIONS_W: f32 = 210.0;
const EDIT_ACTION_GAP: f32 = 6.0;
const CONTROL_GROUP_GAP: f32 = 10.0;
const TRANSPORT_BTN_W: f32 = 30.0;
const TRANSPORT_GAP: f32 = 5.0;
const TRANSPORT_CONTROLS_W: f32 = TRANSPORT_BTN_W * 7.0 + TRANSPORT_GAP * 6.0;
const FONT_UI: f32 = 14.0;

pub(crate) fn show(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    theme: TimelineTheme,
) -> Option<SegmentCommand> {
    let mut command = None;
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
    in_rect(
        ui,
        transport,
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = TRANSPORT_GAP;
            let icons: [(&str, &str, Option<SegmentCommand>); 7] = [
                (
                    "⏮",
                    "Prethodni segment",
                    Some(SegmentCommand::Step { up: true }),
                ),
                (
                    "",
                    "Prethodni slot",
                    Some(SegmentCommand::StepSlot { up: true }),
                ),
                (
                    "⚑",
                    "Prethodni marker",
                    Some(SegmentCommand::StepMarker { up: true }),
                ),
                (
                    "🏠",
                    "Početak Playlist inputa",
                    Some(SegmentCommand::ProgramStart),
                ),
                (
                    "⚑",
                    "Sljedeći marker",
                    Some(SegmentCommand::StepMarker { up: false }),
                ),
                (
                    "",
                    "Sljedeći slot",
                    Some(SegmentCommand::StepSlot { up: false }),
                ),
                (
                    "⏭",
                    "Sljedeći segment",
                    Some(SegmentCommand::Step { up: false }),
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
                if clicked(action_button(ui, "M marker", true, theme)) {
                    command = Some(SegmentCommand::Marker);
                }
                let quick = segments.quick_cover_slot().is_ok();
                if clicked(action_button(ui, "Cover slot", quick, theme)) {
                    command = Some(SegmentCommand::Cover { overwrite: false });
                }
                let overwrite = segments.overwrite_cover_slot().is_ok();
                if clicked(action_button(ui, "Overwrite", overwrite, theme)) {
                    command = Some(SegmentCommand::Cover { overwrite: true });
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
                let on = segments.sync_enabled;
                let text = if on { theme.focus } else { theme.text };
                let button =
                    egui::Button::new(RichText::new("Sync/B-roll").color(text).size(FONT_UI))
                        .min_size(Vec2::new(0.0, COMPACT_CTRL_H))
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::new(
                            1.0,
                            if on { theme.focus } else { theme.border },
                        ));
                if clicked(ui.add(button).on_hover_text("Sync pokrivalica")) {
                    command = Some(SegmentCommand::ToggleSync);
                }
                // Right to left: Redo, then Undo, left of Sync/B-roll.
                for (label, target) in [("Redo", SegmentCommand::Redo), ("Undo", SegmentCommand::Undo)] {
                    let enabled = segments.action_enabled(match target {
                        SegmentCommand::Undo => "undo_object",
                        _ => "redo_object",
                    });
                    if clicked(action_button(ui, label, enabled, theme)) {
                        command = Some(target);
                    }
                }
            },
        );
    }
    command
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

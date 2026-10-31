//! The S-Story panel (user 2026-10-31): the skeleton's elements in order, each a source clip
//! (OFF or statement) with its length, and Open, which puts the skeleton on its flat timeline in
//! place of the source timeline. A click on an element opens the skeleton at its start. It paints
//! the skeleton view and gives the clicks as the action `skeleton_open`; it holds no state, reads
//! nothing and knows no form.

use eframe::egui::{self, RichText, Sense};
use qnc_action_event::ActionEvent;
use qnc_skeleton::{SkeletonView, OPEN, PANEL};
use qnc_skeleton_db::ElementKind;

pub const MODULE_ID: &str = "qnc.module.skeleton-panel";

/// Paints the skeleton on the whole of `ui`.
pub fn show(ui: &mut egui::Ui, view: &SkeletonView, theme: &qnc_desktop_theme::Theme) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let inner = rect.shrink(8.0);
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    ui.set_clip_rect(inner.intersect(ui.clip_rect()));
    let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height };
    let muted = |text: &str| RichText::new(text).color(theme.text_muted).size(theme.font_ui);
    let mut asked = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(qnc_ui_text::text("skeleton.title")).color(theme.text).strong());
        let open = view.subsegment.is_some();
        if qnc_ui_kit::chrome_button(ui, qnc_ui_text::text("skeleton.open"), !view.elements.is_empty(), open, style).on_hover_text(qnc_ui_text::text("skeleton.open_hint")).clicked() {
            asked = Some(ActionEvent::key(OPEN).at(PANEL));
        }
    });
    if !view.message.is_empty() {
        ui.add(egui::Label::new(muted(&view.message)).wrap());
    }
    if view.elements.is_empty() {
        ui.add(egui::Label::new(muted(qnc_ui_text::text("skeleton.empty"))).wrap());
        return asked;
    }
    ui.add_space(4.0);
    let playhead = view.subsegment.as_ref().and_then(|shown| shown.playhead);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(&mut ui, |ui| {
        for (index, row) in view.elements.iter().enumerate() {
            let end = view.elements.get(index + 1).map_or(view.total_frames, |next| next.start);
            let lit = playhead.is_some_and(|at| row.start <= at && at < end);
            let kind = match row.kind {
                ElementKind::Off => qnc_ui_text::text("skeleton.off"),
                ElementKind::Ton => qnc_ui_text::text("skeleton.ton"),
            };
            let text = format!("{}  {}  {}", kind, row.clip_name, row.length);
            let colour = if lit { theme.focus } else { theme.text };
            let response = ui.add(egui::Label::new(RichText::new(text).color(colour).size(theme.font_ui)).sense(Sense::click()));
            if response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                asked = Some(ActionEvent::key(OPEN).at(PANEL).with_target(index.to_string()));
            }
        }
    });
    asked
}

//! The action bar panel (user 2026-11-01: the Transcribe button of AI Media Assist under its
//! media pool, as Cancel | Select under the Ingest browser). A layout gives it a place and the
//! buttons of that place (label and catalog action); it draws them on the right of the bar,
//! the first one strong (the proposed action), the others plain, and gives the clicked
//! button's catalog action. Passive: no state, no database, no workflow.

use eframe::egui::{self, Rect, Sense, Ui, Vec2};
use qnc_desktop_theme::Theme;

pub const MODULE_ID: &str = "qnc.module.action-bar-panel";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The minimum width of a button, as Select of the Ingest browser.
const BUTTON_WIDTH: f32 = 120.0;

/// One button of the bar.
pub struct BarAction<'a> {
    pub label: &'a str,
    pub action_id: &'a str,
}

/// The bar in `rect` with `actions` from the right, the first one strong; `enabled` tells
/// which catalog actions can be pressed now. Nothing is drawn without actions.
pub fn show<'a>(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    pad: f32,
    actions: impl IntoIterator<Item = BarAction<'a>>,
    enabled: impl Fn(&str) -> bool,
) -> Option<String> {
    let actions: Vec<_> = actions.into_iter().collect();
    if actions.is_empty() {
        return None;
    }
    ui.painter().rect_filled(rect, 0.0, theme.bg);
    let mut clicked = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        ui.allocate_rect(rect, Sense::hover());
        let inner = egui::UiBuilder::new().max_rect(rect.shrink(pad)).layout(egui::Layout::right_to_left(egui::Align::Center));
        ui.scope_builder(inner, |ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            ui.spacing_mut().interact_size = Vec2::new(BUTTON_WIDTH, theme.chrome_control_height);
            for (index, action) in actions.iter().enumerate() {
                let on = enabled(action.action_id);
                let response = if index == 0 {
                    qnc_ui_kit::strong_button(ui, action.label, on, theme.accent, theme.chrome_control_height)
                } else {
                    qnc_ui_kit::chrome_button(ui, action.label, on, false, qnc_theme_styles::button(theme))
                };
                if response.clicked() {
                    clicked = Some(action.action_id.to_string());
                }
            }
        });
    });
    clicked
}

/// The height of the bar: a button with the pad above and below it.
pub fn height(theme_control_height: f32, pad: f32) -> f32 {
    theme_control_height + 2.0 * pad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_is_a_button_with_its_pad_above_and_below() {
        assert_eq!(height(28.0, 6.0), 40.0);
    }
}

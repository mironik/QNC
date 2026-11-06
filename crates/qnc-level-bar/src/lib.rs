//! The level bar (user 2026-10-29: the meter with its NORM button is one block, placed in the
//! middle of the source timeline's bar, its size as on the Segments bar): the level meter
//! block of the player's output A1/A2 and, right of it, NORM (lit while the preview
//! normalises the loudness; a click is the N key, `toggle_loudness_normalize`).
//!
//! Passive: it measures, plays and reads nothing; the caller gives the peaks and NORM's state.

use eframe::egui::{self, PointerButton, Pos2, Rect, Ui, Vec2};

pub const MODULE_ID: &str = "qnc.module.level-bar";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The bar's size (as on the Segments bar: the width of its seven middle buttons).
pub const WIDTH: f32 = 240.0;
pub const HEIGHT: f32 = 22.0;
/// The width of NORM, of the sound assistant's AI and the gap before each.
const NORM_W: f32 = 52.0;
const AI_W: f32 = 34.0;
const GAP: f32 = 6.0;

/// The catalog action of a click on NORM (the N key).
pub const NORMALIZE: &str = "toggle_loudness_normalize";
/// The catalog action of a click on AI (the sound assistant on or off, user 2026-11-06).
pub const SOUND_ASSIST: &str = "toggle_sound_assist";

/// The bar centred in `row`; `normalize` is NORM's state (`None`: the form has no story to
/// normalise, NORM is shown but off), `assist` the sound assistant's (AI) the same. The action
/// of the button clicked, if one was.
pub fn show(ui: &mut Ui, row: Rect, levels: [Option<f32>; 2], (normalize, assist): (Option<bool>, Option<bool>), theme: &qnc_timeline::TimelineTheme) -> Option<&'static str> {
    let rect = Rect::from_center_size(row.center(), Vec2::new(WIDTH, HEIGHT));
    let ai = Rect::from_min_max(Pos2::new(rect.max.x - AI_W, rect.min.y), rect.max);
    let norm = Rect::from_min_max(Pos2::new(ai.min.x - GAP - NORM_W, rect.min.y), Pos2::new(ai.min.x - GAP, rect.max.y));
    let meter = Rect::from_min_max(rect.min, Pos2::new(norm.min.x - GAP, rect.max.y));
    let style = qnc_level_meter::MeterStyle { background: theme.background, border: theme.border, text: theme.muted };
    qnc_level_meter::show(ui, ui.make_persistent_id("qnc_level_bar_meter"), meter.shrink2(Vec2::new(0.0, 1.0)), &[("A1", levels[0]), ("A2", levels[1])], style);
    let norm_clicked = toggle(ui, norm, ("story.ui.normalized", "story.ui.normalize_hint"), normalize, theme);
    let ai_clicked = toggle(ui, ai, ("story.ui.sound_assist", "story.ui.sound_assist_hint"), assist, theme);
    norm_clicked.then_some(NORMALIZE).or(ai_clicked.then_some(SOUND_ASSIST))
}

/// One on/off button of the bar (`None`: shown but off); true when clicked.
fn toggle(ui: &mut Ui, place: Rect, (label, hint): (&'static str, &'static str), state: Option<bool>, theme: &qnc_timeline::TimelineTheme) -> bool {
    let mut clicked = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(place).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().button_padding = Vec2::new(6.0, 1.0);
        let style = qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: HEIGHT };
        let button = qnc_ui_kit::chrome_button(ui, qnc_ui_text::text(label), state.is_some(), state.unwrap_or(false), style);
        let button = button.on_hover_text(qnc_ui_text::text(hint));
        if button.has_focus() {
            button.surrender_focus(); // the keyboard stays with the catalog
        }
        clicked = button.clicked_by(PointerButton::Primary);
    });
    clicked
}

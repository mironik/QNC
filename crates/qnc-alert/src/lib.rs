//! A warning the user cannot miss (user 2026-10-21: when something new is taken while
//! something else waits for Enter, the refusal is shown so that it is noticed, not only in
//! the status line). A red band at the top centre of the screen with the text in large
//! letters, for a few seconds after it was raised. Passive paint only: the text and when it
//! was raised come from the caller; this piece decides and remembers nothing.

use eframe::egui::{self, Align2, Color32, FontId, Sense, Stroke, StrokeKind, Vec2};
use std::time::Instant;

pub const MODULE_ID: &str = "qnc.module.alert";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");


const TEXT_SIZE: f32 = 20.0;
const PAD: Vec2 = Vec2::new(22.0, 12.0);
const TOP: f32 = 48.0;
/// The share of the screen width the band may take.
const SCREEN_SHARE: f32 = 0.8;

/// The warning and how long it is shown (core: `qnc-notice`, docs/99).
pub use qnc_notice::{Alert, SHOWN};

/// Shows `alert` over everything while it is on the screen, in `fill` with `text` letters;
/// asks for a repaint when it is to go.
pub fn show(ctx: &egui::Context, alert: Option<&Alert>, fill: Color32, text: Color32) {
    let Some(alert) = alert else { return };
    let now = Instant::now();
    if !alert.shown_at(now) {
        return;
    }
    ctx.request_repaint_after(SHOWN.saturating_sub(now.saturating_duration_since(alert.raised)));
    let screen = ctx.screen_rect();
    let wrap = screen.width() * SCREEN_SHARE - 2.0 * PAD.x;
    egui::Area::new(egui::Id::new("qnc_alert"))
        .order(egui::Order::Tooltip)
        .anchor(Align2::CENTER_TOP, Vec2::new(0.0, TOP))
        .interactable(false)
        .show(ctx, |ui| {
            let galley = ui.painter().layout(alert.text.clone(), FontId::proportional(TEXT_SIZE), text, wrap.max(80.0));
            let (rect, _) = ui.allocate_exact_size(galley.size() + 2.0 * PAD, Sense::hover());
            ui.painter().rect_filled(rect, 6.0, fill);
            ui.painter().rect_stroke(rect, 6.0, Stroke::new(2.0, text), StrokeKind::Inside);
            ui.painter().galley(rect.min + PAD, galley, text);
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_warning_is_shown_for_a_while_then_goes() {
        let alert = Alert::new("Enter or Esc first");
        assert!(alert.shown_at(alert.raised));
        assert!(alert.shown_at(alert.raised + SHOWN - Duration::from_millis(1)));
        assert!(!alert.shown_at(alert.raised + SHOWN));
    }
}

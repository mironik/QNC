//! The trim zoom (user 2026-10-10: the timeline has no room for four pictures; "a window
//! in the centre of the screen, a kind of quasi zoom", opened by the Ctrl keys that
//! already take something). While a slot (Ctrl+S), a marker (Ctrl+M) or a segment edge
//! (Ctrl+I/O) is taken, a window in the centre of the screen shows its edit pictures
//! large, in one row, each named, with a line where the marker is; it closes when the
//! taken thing is let go (Enter, Escape). Passive paint only: the pictures and their
//! names come from the program view, this piece reads, decodes and decides nothing.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use qnc_program_view::{EditStill, SegmentsView};

pub const MODULE_ID: &str = "qnc.module.trim-overlay";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The largest picture (16:9) and the gaps of the window.
const MAX_PICTURE: Vec2 = Vec2::new(480.0, 270.0);
const GAP: f32 = 10.0;
const MARGIN: f32 = 14.0;
const LABEL_HEIGHT: f32 = 22.0;
/// The share of the screen width the window may take.
const SCREEN_SHARE: f32 = 0.92;

/// Shows the trim zoom over everything while `segments` has edit pictures (the Wrap
/// rows' or, for a segment edge, the source timeline's).
pub fn show(ctx: &egui::Context, segments: &SegmentsView) {
    let stills = if segments.source_edit_stills.is_empty() { &segments.edit_stills } else { &segments.source_edit_stills };
    if stills.is_empty() {
        return;
    }
    let count = stills.len() as f32;
    let screen = ctx.screen_rect();
    let width = ((screen.width() * SCREEN_SHARE - 2.0 * MARGIN - GAP * (count - 1.0)) / count).min(MAX_PICTURE.x);
    let picture = Vec2::new(width.max(16.0), width.max(16.0) * 9.0 / 16.0);
    let size = Vec2::new(count * picture.x + GAP * (count - 1.0), picture.y + LABEL_HEIGHT) + Vec2::splat(2.0 * MARGIN);
    egui::Area::new(egui::Id::new("qnc_trim_overlay"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            let visuals = ui.visuals().clone();
            ui.painter().rect_filled(rect, 6.0, visuals.window_fill);
            ui.painter().rect_stroke(rect, 6.0, visuals.window_stroke, StrokeKind::Inside);
            let top = rect.min + Vec2::splat(MARGIN);
            for (index, still) in stills.iter().enumerate() {
                let left = top.x + index as f32 * (picture.x + GAP);
                let pane = Rect::from_min_size(egui::pos2(left, top.y), picture);
                paint_still(ui, pane, still, &visuals);
                // The marker lies between the picture before it and the one after it.
                if still.after && index > 0 && !stills[index - 1].after {
                    let x = left - GAP / 2.0;
                    let accent = visuals.selection.bg_fill;
                    ui.painter().line_segment([egui::pos2(x, pane.top() - 4.0), egui::pos2(x, pane.bottom() + 4.0)], Stroke::new(3.0, accent));
                }
            }
        });
}

fn paint_still(ui: &mut egui::Ui, pane: Rect, still: &EditStill, visuals: &egui::Visuals) {
    ui.painter().rect_filled(pane, 0.0, Color32::BLACK);
    if let Some(image) = still.image.as_deref().filter(|_| still.picture.is_some()) {
        let _ = qnc_ui_kit::paint_rgba_image(ui, pane, &still.id, image.content_key, image.size, &image.pixels);
    }
    ui.painter().rect_stroke(pane, 0.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), StrokeKind::Inside);
    if still.cut {
        // The picture at the cut being moved (user 2026-10-13: a red frame).
        ui.painter().rect_stroke(pane, 0.0, Stroke::new(4.0, Color32::from_rgb(230, 30, 30)), StrokeKind::Inside);
    }
    ui.painter().text(
        egui::pos2(pane.center().x, pane.bottom() + LABEL_HEIGHT / 2.0 + 2.0),
        Align2::CENTER_CENTER,
        &still.label,
        FontId::proportional(14.0),
        visuals.text_color(),
    );
}

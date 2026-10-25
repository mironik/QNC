//! The trim zoom (user 2026-10-10: the timeline has no room for four pictures; "a window
//! in the centre of the screen, a kind of quasi zoom", opened by the Ctrl keys that
//! already take something). While a slot (Ctrl+S), a marker (Ctrl+M) or a segment edge
//! (Ctrl+I/O) is taken, a window in the centre of the screen shows its edit pictures
//! large, in one row, each named, with a line where the marker is; it closes when the
//! taken thing is let go (Enter, Escape). Passive paint only: the pictures and their
//! names come from the program view, this piece reads, decodes and decides nothing.
//! Under the pictures (user 2026-10-25) the sound around the cut, A1 and A2, read finely
//! from the media, and a thin line through the cut: between the two pictures and
//! across both waves.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};
use qnc_program_view::{ClipEdge, EditStill, SegmentsView, ZoomSound};

pub const MODULE_ID: &str = "qnc.module.trim-overlay";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The largest picture (16:9) and the gaps of the window.
const MAX_PICTURE: Vec2 = Vec2::new(480.0, 270.0);
const GAP: f32 = 10.0;
const MARGIN: f32 = 14.0;
const LABEL_HEIGHT: f32 = 22.0;
/// One lane of the zoom's sound and the gap above the lanes.
const LANE_HEIGHT: f32 = 64.0;
const LANES_GAP: f32 = 8.0;
/// The thin line through the cut.
const CUT_LINE: Color32 = Color32::from_rgb(230, 30, 30);
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
    let sound = segments.zoom_sound.as_ref().filter(|sound| sound.to > sound.from);
    let lanes = if sound.is_some() { LANES_GAP + 2.0 * LANE_HEIGHT + GAP / 2.0 } else { 0.0 };
    let size = Vec2::new(count * picture.x + GAP * (count - 1.0), picture.y + LABEL_HEIGHT + lanes) + Vec2::splat(2.0 * MARGIN);
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
            let row_width = size.x - 2.0 * MARGIN;
            let mut gap_at = None;
            for (index, still) in stills.iter().enumerate() {
                let left = top.x + index as f32 * (picture.x + GAP);
                let pane = Rect::from_min_size(egui::pos2(left, top.y), picture);
                paint_still(ui, pane, still, &visuals);
                // The marker lies between the picture before it and the one after it.
                if still.after && index > 0 && !stills[index - 1].after {
                    let x = left - GAP / 2.0;
                    gap_at.get_or_insert(x);
                    let accent = visuals.selection.bg_fill;
                    ui.painter().line_segment([egui::pos2(x, pane.top() - 4.0), egui::pos2(x, pane.bottom() + 4.0)], Stroke::new(3.0, accent));
                }
            }
            if let Some(sound) = sound {
                let area = Rect::from_min_size(egui::pos2(top.x, top.y + picture.y + LABEL_HEIGHT + LANES_GAP), Vec2::new(row_width, 2.0 * LANE_HEIGHT + GAP / 2.0));
                paint_sound(ui, area, sound, gap_at, top.y, &visuals);
            }
        });
}

/// How high a peak (per mille of full scale) is drawn, 0 to 1: on a dB scale from
/// -54 dBFS, so quiet speech (-40 LUFS on a camera channel) and the pause after a word
/// differ at a glance, as an editor's audio zoom shows them.
fn shown_height(peak: u16) -> f32 {
    if peak == 0 {
        return 0.0;
    }
    let dbfs = 20.0 * (f32::from(peak) / 1000.0).log10();
    ((dbfs + 54.0) / 54.0).clamp(0.0, 1.0)
}

/// A1 and A2 around the cut, each a lane of its peaks mirrored about its middle, and the
/// thin line of the cut: from between the pictures when it lies under that gap, across
/// both lanes at the cut's place in the window.
fn paint_sound(ui: &mut egui::Ui, area: Rect, sound: &ZoomSound, gap_at: Option<f32>, pictures_top: f32, visuals: &egui::Visuals) {
    let span = (sound.to - sound.from) as f32;
    let cut_x = area.left() + area.width() * (sound.cut.saturating_sub(sound.from)) as f32 / span;
    for lane in 0..2 {
        let top = area.top() + lane as f32 * (LANE_HEIGHT + GAP / 2.0);
        let band = Rect::from_min_size(egui::pos2(area.left(), top), Vec2::new(area.width(), LANE_HEIGHT));
        ui.painter().rect_filled(band, 2.0, Color32::from_rgb(17, 103, 88));
        let peaks = &sound.peaks[lane];
        let middle = band.center().y;
        let half = LANE_HEIGHT / 2.0 - 2.0;
        if !peaks.is_empty() {
            let step = band.width() / peaks.len() as f32;
            for (column, peak) in peaks.iter().enumerate() {
                let height = shown_height(*peak) * half;
                let x = band.left() + (column as f32 + 0.5) * step;
                ui.painter().line_segment([egui::pos2(x, middle - height), egui::pos2(x, middle + height.max(0.5))], Stroke::new(step.max(1.0), Color32::from_rgb(80, 190, 148)));
            }
        }
        ui.painter().text(band.left_top() + Vec2::new(6.0, 4.0), Align2::LEFT_TOP, if lane == 0 { "A1" } else { "A2" }, FontId::proportional(12.0), visuals.text_color());
    }
    // One thin line from between the pictures down through both waves when the cut is there.
    let from_top = match gap_at {
        Some(x) if (x - cut_x).abs() < 2.0 => pictures_top - 4.0,
        _ => area.top(),
    };
    ui.painter().line_segment([egui::pos2(cut_x, from_top), egui::pos2(cut_x, area.bottom())], Stroke::new(1.5, CUT_LINE));
}

fn paint_still(ui: &mut egui::Ui, pane: Rect, still: &EditStill, visuals: &egui::Visuals) {
    ui.painter().rect_filled(pane, 0.0, Color32::BLACK);
    if let Some(image) = still.image.as_deref().filter(|_| still.picture.is_some()) {
        let _ = qnc_ui_kit::paint_rgba_image(ui, pane, &still.id, image.content_key, image.size, &image.pixels);
    }
    ui.painter().rect_stroke(pane, 0.0, Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color), StrokeKind::Inside);
    if let Some(edge) = still.clip_edge {
        // The clip ends here: a red line on that side (user 2026-10-13).
        // A band a sixteenth of the picture wide (at least 14 px), well seen at a glance.
        let width = (pane.width() / 16.0).max(14.0);
        let band = match edge {
            ClipEdge::Start => Rect::from_min_max(pane.min, egui::pos2(pane.left() + width, pane.bottom())),
            ClipEdge::End => Rect::from_min_max(egui::pos2(pane.right() - width, pane.top()), pane.max),
        };
        ui.painter().rect_filled(band, 0.0, Color32::from_rgb(230, 30, 30));
    }
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

#[cfg(test)]
mod tests {
    use super::shown_height;

    #[test]
    fn peaks_are_drawn_on_a_db_scale() {
        assert_eq!(shown_height(0), 0.0);
        assert_eq!(shown_height(1000), 1.0);
        assert!((shown_height(50) - 0.52).abs() < 0.02, "-26 dBFS about half");
        assert_eq!(shown_height(1), 0.0, "-60 dBFS below the floor");
    }
}

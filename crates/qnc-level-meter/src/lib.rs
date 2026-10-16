//! A passive sound level meter (user 2026-10-16: on the Segments panel, in place of the
//! middle buttons): one horizontal bar per lane (A1, A2) on a dBFS scale, green, yellow
//! near the top, red at the top, with a short hold of the peak. It is given the peak the
//! player measured at its output since the last state; it measures nothing itself, plays
//! nothing and reads nothing. Only the falling of the bar between two states is its own
//! (display ballistics in the egui memory of its id).

use eframe::egui::{self, Align2, Color32, FontId, Id, Rect, Stroke, StrokeKind, Ui, Vec2};
use std::time::Duration;

pub const MODULE_ID: &str = "qnc.module.level-meter";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The bottom of the scale.
pub const FLOOR_DBFS: f32 = -48.0;
const YELLOW_DBFS: f32 = -12.0;
const RED_DBFS: f32 = -3.0;
/// How fast a bar falls, and how long the peak mark stays.
const FALL_DB_PER_S: f32 = 24.0;
const HOLD: f64 = 1.2;

/// Colours from the form's theme.
#[derive(Debug, Clone, Copy)]
pub struct MeterStyle {
    pub background: Color32,
    pub border: Color32,
    pub text: Color32,
}

#[derive(Debug, Clone, Copy)]
struct Ballistics {
    shown: f32,
    peak: f32,
    peak_at: f64,
    at: f64,
}

impl Default for Ballistics {
    fn default() -> Self {
        Self { shown: FLOOR_DBFS, peak: FLOOR_DBFS, peak_at: 0.0, at: 0.0 }
    }
}

impl Ballistics {
    /// The bar after `now`: up at once to a louder level, down at the fall rate.
    fn step(mut self, level: Option<f32>, now: f64) -> Self {
        let level = level.unwrap_or(FLOOR_DBFS).max(FLOOR_DBFS);
        let fallen = self.shown - FALL_DB_PER_S * (now - self.at).max(0.0) as f32;
        self.shown = level.max(fallen).max(FLOOR_DBFS);
        if self.shown >= self.peak || now - self.peak_at > HOLD {
            self.peak = self.shown;
            self.peak_at = now;
        }
        self.at = now;
        self
    }
}

/// The position (0..1) of a level on the scale.
pub fn scale(dbfs: f32) -> f32 {
    ((dbfs - FLOOR_DBFS) / -FLOOR_DBFS).clamp(0.0, 1.0)
}

/// Bars for `lanes` (label, peak dBFS) in `rect`, one under the other.
pub fn show(ui: &mut Ui, id: Id, rect: Rect, lanes: &[(&str, Option<f32>)], style: MeterStyle) {
    if lanes.is_empty() || !rect.is_positive() {
        return;
    }
    let now = ui.input(|input| input.time);
    let gap = 2.0;
    let height = ((rect.height() - gap * (lanes.len() as f32 - 1.0)) / lanes.len() as f32).max(2.0);
    let label_width = 22.0;
    let mut moving = false;
    for (index, (label, level)) in lanes.iter().enumerate() {
        let top = rect.top() + index as f32 * (height + gap);
        let row = Rect::from_min_size(egui::pos2(rect.left(), top), Vec2::new(rect.width(), height));
        let bar = Rect::from_min_max(egui::pos2(row.left() + label_width, row.top()), row.max);
        let key = id.with(index);
        let state = ui.data_mut(|data| data.get_temp::<Ballistics>(key)).unwrap_or_default().step(*level, now);
        ui.data_mut(|data| data.insert_temp(key, state));
        moving |= state.shown > FLOOR_DBFS || state.peak > FLOOR_DBFS;
        ui.painter().text(egui::pos2(row.left(), row.center().y), Align2::LEFT_CENTER, *label, FontId::proportional(11.0), style.text);
        ui.painter().rect_filled(bar, 1.0, style.background);
        paint_level(ui, bar, state.shown);
        if state.peak > FLOOR_DBFS {
            let x = bar.left() + bar.width() * scale(state.peak);
            ui.painter().line_segment([egui::pos2(x, bar.top()), egui::pos2(x, bar.bottom())], Stroke::new(2.0, colour(state.peak)));
        }
        ui.painter().rect_stroke(bar, 1.0, Stroke::new(1.0, style.border), StrokeKind::Inside);
    }
    if moving {
        ui.ctx().request_repaint_after(Duration::from_millis(30));
    }
}

/// The bar up to `dbfs`, each part in the colour of its zone.
fn paint_level(ui: &Ui, bar: Rect, dbfs: f32) {
    let zones = [(FLOOR_DBFS, YELLOW_DBFS), (YELLOW_DBFS, RED_DBFS), (RED_DBFS, 0.0)];
    for (from, to) in zones {
        if dbfs <= from {
            break;
        }
        let x0 = bar.left() + bar.width() * scale(from);
        let x1 = bar.left() + bar.width() * scale(dbfs.min(to));
        ui.painter().rect_filled(Rect::from_min_max(egui::pos2(x0, bar.top() + 1.0), egui::pos2(x1, bar.bottom() - 1.0)), 0.0, colour(from + 0.01));
    }
}

fn colour(dbfs: f32) -> Color32 {
    if dbfs >= RED_DBFS {
        Color32::from_rgb(220, 60, 50)
    } else if dbfs >= YELLOW_DBFS {
        Color32::from_rgb(230, 200, 60)
    } else {
        Color32::from_rgb(70, 190, 110)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_rises_at_once_and_falls_slowly_and_the_peak_holds() {
        let up = Ballistics::default().step(Some(-6.0), 0.0);
        assert_eq!((up.shown, up.peak), (-6.0, -6.0));
        let half = up.step(None, 0.5);
        assert_eq!(half.shown, -18.0, "24 dB a second");
        assert_eq!(half.peak, -6.0, "the peak holds");
        let later = half.step(None, 2.0);
        assert_eq!(later.shown, FLOOR_DBFS);
        assert_eq!(later.peak, FLOOR_DBFS, "after the hold the peak falls too");
        assert_eq!((scale(0.0), scale(FLOOR_DBFS), scale(-24.0)), (1.0, 0.0, 0.5));
    }
}

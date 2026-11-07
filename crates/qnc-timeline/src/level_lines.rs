//! The level curve of a segment's A1 and a cover's A2 on the lane (docs/98 2.1): a line over
//! the wave, its points, and the line of the playhead's point while it is taken. A right click
//! near the line takes it (as Ctrl+Shift+1/2, user 2026-11-06: a right click is Ctrl+). The
//! timeline only draws and reports; the curve is its owner's.

use eframe::egui::{self, Color32, Pos2, Rect, Stroke};

use crate::{frame_for_x, TimelineInput};

/// The level curve of one segment (lane 0) or cover (lane 1) on the program: where it lies
/// `[from, to)`, its level at both ends and its points (program position, tenths of a dB).
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineLevelCurve {
    pub lane: u8,
    pub from: u64,
    pub to: u64,
    pub start_db10: i16,
    pub end_db10: i16,
    pub points: Vec<(f64, i16)>,
    /// The taken one: drawn as drafted, its point on the playhead line shown.
    pub taken: bool,
}

/// How near the line (pixels up or down) a right click takes it.
const REACH: f32 = 4.0;

/// The height of a level in a lane: 0 dB at seven tenths, -60 dB at the bottom, +12 dB at the top.
fn level_y(band: Rect, db10: f64) -> f32 {
    let t = if db10 >= 0.0 { 0.7 + 0.3 * (db10 / 120.0).min(1.0) } else { 0.7 * (1.0 + (db10 / 600.0).max(-1.0)) };
    band.bottom() - t as f32 * band.height()
}

impl TimelineLevelCurve {
    /// Its corners in program position and level: its start, its points, its end.
    fn corners(&self) -> Vec<(f64, f64)> {
        let mut corners = vec![(self.from as f64, f64::from(self.start_db10))];
        corners.extend(self.points.iter().map(|(at, db10)| (*at, f64::from(*db10))));
        corners.push((self.to as f64, f64::from(self.end_db10)));
        corners
    }

    /// Its level at program position `at`, tenths of a dB.
    fn level_at(&self, at: f64) -> f64 {
        qnc_gain::curve_at(&self.corners(), at)
    }
}

/// The x of a program position on the row (fractional, unclamped).
fn x_at(track: Rect, start: u64, duration: u64, at: f64) -> f32 {
    track.left() + ((at - start as f64) / duration.max(1) as f64) as f32 * track.width()
}

/// Draws every curve of `lane` (0 A1, 1 A2) over its band.
pub(crate) fn paint(ui: &egui::Ui, band: Rect, lane: u8, input: &TimelineInput<'_>) {
    let (start, duration) = (input.state.range_start(), input.state.duration_frames());
    let colors = [input.theme.level_curve, input.theme.lane_taken, input.theme.marker_hover];
    paint_level_curves(ui.painter(), band, (start, duration), lane, input.level_curves, input.state.playhead_frame, colors);
}

/// Draws the level curves of `lane` over `band`, which shows frames `[start, start + length)`
/// (`window`); the point on the playhead line of the taken curve is circled. The one drawing of
/// the timeline and of the trim zoom (rule 0.12). `colors`: a curve, the taken curve, its point.
pub fn paint_level_curves(
    painter: &egui::Painter,
    band: Rect,
    (start, duration): (u64, u64),
    lane: u8,
    curves: &[TimelineLevelCurve],
    playhead: Option<u64>,
    [plain, taken, point]: [Color32; 3],
) {
    let painter = painter.with_clip_rect(band);
    for curve in curves.iter().filter(|curve| curve.lane == lane) {
        if curve.to <= start || curve.from >= start + duration {
            continue;
        }
        let color = if curve.taken { taken } else { plain };
        let line: Vec<Pos2> = curve
            .corners()
            .into_iter()
            .map(|(at, db10)| Pos2::new(x_at(band, start, duration, at), level_y(band, db10)))
            .collect();
        painter.add(egui::Shape::line(line, Stroke::new(if curve.taken { 1.5 } else { 1.0 }, color)));
        for (at, db10) in &curve.points {
            let center = Pos2::new(x_at(band, start, duration, *at), level_y(band, f64::from(*db10)));
            painter.rect_filled(Rect::from_center_size(center, egui::vec2(4.0, 4.0)), 0.0, color);
        }
        // The point the keys control: on the playhead line of the taken curve.
        if let Some(playhead) = playhead.filter(|frame| curve.taken && (curve.from..curve.to).contains(frame)) {
            let at = playhead as f64;
            let level = curve.level_at(at);
            let center = Pos2::new(x_at(band, start, duration, at), level_y(band, level));
            painter.circle_stroke(center, 3.5, Stroke::new(1.5, point));
            // How much it is raised or lowered there (user 2026-11-07: it must show).
            let text = format!("{:+.1} dB", level / 10.0);
            let galley = painter.layout_no_wrap(text, egui::FontId::proportional(13.0), point);
            let mut at = egui::pos2(center.x + 7.0, center.y - galley.size().y - 3.0);
            if at.x + galley.size().x > band.right() {
                at.x = center.x - 7.0 - galley.size().x;
            }
            at.y = at.y.max(band.top() + 1.0);
            painter.galley(at, galley, point);
        }
    }
}

/// The dB scale of a lane's zoom (user 2026-11-07): a faint line and its label at each level, on
/// the same height a curve is drawn (0 dB at seven tenths), so the line shows what it does.
pub fn paint_scale(painter: &egui::Painter, band: Rect, color: Color32) {
    let painter = painter.with_clip_rect(band);
    let faint = color.gamma_multiply(0.35);
    for db in [12, 6, 0, -6, -12, -24, -40] {
        let y = level_y(band, f64::from(db) * 10.0);
        let stroke = Stroke::new(if db == 0 { 1.0 } else { 0.5 }, faint);
        painter.line_segment([Pos2::new(band.left(), y), Pos2::new(band.right(), y)], stroke);
        let text = if db > 0 { format!("+{db}") } else { db.to_string() };
        painter.text(Pos2::new(band.right() - 3.0, y - 1.0), egui::Align2::RIGHT_BOTTOM, text, egui::FontId::proportional(10.0), faint);
    }
}

/// A right click near a curve of `lane` on its band: the program frame clicked.
pub(crate) fn hit(band: Rect, lane: u8, pos: Pos2, input: &TimelineInput<'_>) -> Option<u64> {
    let window = (input.state.range_start(), input.state.duration_frames());
    level_curve_hit(band, window, lane, pos, input.level_curves)
}

/// The frame of a click near a curve of `lane` on `band` showing frames `window` (timeline and
/// trim zoom alike).
pub fn level_curve_hit(band: Rect, (start, duration): (u64, u64), lane: u8, pos: Pos2, curves: &[TimelineLevelCurve]) -> Option<u64> {
    let frame = frame_for_x(band, start, duration, pos.x);
    let curve = curves.iter().find(|curve| curve.lane == lane && (curve.from..curve.to).contains(&frame))?;
    let y = level_y(band, curve.level_at(frame as f64));
    ((pos.y - y).abs() <= REACH).then_some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_level_is_placed_in_its_lane_and_a_curve_is_a_line_between_its_corners() {
        let band = Rect::from_min_size(Pos2::new(0.0, 0.0), egui::vec2(100.0, 20.0));
        assert_eq!(level_y(band, -600.0), 20.0);
        assert!((level_y(band, 0.0) - 6.0).abs() < 1e-4, "0 dB at seven tenths");
        assert_eq!(level_y(band, 120.0), 0.0);
        let curve = TimelineLevelCurve { lane: 0, from: 0, to: 100, start_db10: 0, end_db10: -200, points: vec![(50.0, -200)], taken: false };
        assert_eq!(curve.level_at(25.0), -100.0);
        assert_eq!(curve.level_at(75.0), -200.0);
    }
}

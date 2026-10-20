//! Passive public waveform view component.
//!
//! This component paints prepared peak arrays only. It owns no database,
//! decoder, scanner, probe, player clock or application workflow.

use eframe::egui::{self, Color32, Rect, Stroke};

pub const MODULE_ID: &str = "qnc.module.wave-view";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The level marks of an audio lane (user 2026-10-16, EBU): the alignment level (EBU R 68,
/// -18 dBFS), the permitted maximum (-9 dBFS) and the limit near the top (-1 dBFS).
pub const LEVEL_MARKS_DBFS: [f32; 3] = [-18.0, -9.0, -1.0];

/// Thin lines above and below the middle of a lane at `LEVEL_MARKS_DBFS`, on the scale the
/// peaks are drawn (`gain` as `paint_wave_peaks_gain` uses it); a mark past the lane's
/// edge is not drawn.
pub fn paint_level_marks(painter: &egui::Painter, rect: Rect, gain: f32, colors: [Color32; 3]) {
    let mid = rect.center().y;
    let half = rect.height() * 0.48;
    for (dbfs, color) in LEVEL_MARKS_DBFS.into_iter().zip(colors) {
        let amp = 10f32.powf(dbfs / 20.0) * gain;
        if amp > 1.0 {
            continue;
        }
        for y in [mid - amp * half, mid + amp * half] {
            painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], Stroke::new(0.5, color));
        }
    }
}

pub fn paint_wave_peaks(painter: &egui::Painter, rect: Rect, peaks: &[f32], color: Color32) {
    paint_wave_peaks_gain(painter, rect, peaks, 1.0, color);
}

/// The peaks times `gain` (the lane's level, user 2026-10-16): louder than full scale
/// reaches the lane's edge.
pub fn paint_wave_peaks_gain(painter: &egui::Painter, rect: Rect, peaks: &[f32], gain: f32, color: Color32) {
    // Off screen (a Wrap row scrolled away) nothing is built at all.
    if peaks.is_empty() || rect.width() < 2.0 || !painter.clip_rect().intersects(rect) {
        return;
    }
    painter.add(egui::Shape::mesh(wave_mesh(rect, peaks, gain, color, painter.clip_rect())));
}

/// The wave as one mesh, a one pixel column per peak range: the same look as a line per
/// column, built once instead of a line shape per column for the tessellator (user
/// 2026-10-20: the tessellation of the waves took most of a playing frame).
fn wave_mesh(rect: Rect, peaks: &[f32], gain: f32, color: Color32, clip: Rect) -> egui::Mesh {
    let mid = rect.center().y;
    let half = rect.height() * 0.48;
    let mut mesh = egui::Mesh::default();
    for (x_index, range) in peak_ranges_for_width(peaks.len(), rect.width()).enumerate() {
        let x = rect.left() + x_index as f32;
        if x + 1.0 < clip.left() || x > clip.right() {
            continue;
        }
        let max_peak = peaks[range]
            .iter()
            .map(|peak| peak.abs())
            .fold(0.0_f32, f32::max)
            .mul_add(gain, 0.0)
            .clamp(0.0, 1.0);
        let amp = max_peak * half;
        if amp <= 0.0 {
            continue;
        }
        let column = Rect::from_min_max(egui::pos2(x, mid - amp), egui::pos2(x + 1.0, mid + amp));
        mesh.add_colored_rect(column, color);
    }
    mesh
}

pub fn peak_ranges_for_width(
    peak_count: usize,
    width: f32,
) -> impl Iterator<Item = std::ops::Range<usize>> {
    let bars = width.floor().max(1.0) as usize;
    (0..bars).map(move |index| {
        let start = index * peak_count / bars;
        let end = ((index + 1) * peak_count / bars)
            .max(start + 1)
            .min(peak_count);
        start..end
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_cover_peak_array_without_owning_timeline_state() {
        let ranges = peak_ranges_for_width(10, 4.0).collect::<Vec<_>>();

        assert_eq!(ranges, [0..2, 2..5, 5..7, 7..10]);
    }

    #[test]
    fn a_wave_is_one_column_per_pixel_in_one_mesh() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(4.0, 10.0));
        let mesh = wave_mesh(rect, &[0.5, 0.0, 1.0, 0.25], 1.0, Color32::WHITE, rect);
        assert_eq!(mesh.vertices.len(), 3 * 4, "a silent column draws nothing");
        let tallest = mesh.vertices[4..8].iter().map(|v| v.pos.y).fold(f32::MAX, f32::min);
        assert!((tallest - (5.0 - 4.8)).abs() < 1e-4, "full scale reaches 48 % of the lane above the middle");
    }

    #[test]
    fn at_least_one_range_is_returned_for_narrow_tracks() {
        let ranges = peak_ranges_for_width(3, 0.5).collect::<Vec<_>>();

        assert_eq!(ranges, [0..3]);
    }
}

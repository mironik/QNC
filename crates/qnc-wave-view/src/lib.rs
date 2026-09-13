//! Passive public waveform view component.
//!
//! This component paints prepared peak arrays only. It owns no database,
//! decoder, scanner, probe, player clock or application workflow.

use eframe::egui::{self, Color32, Rect, Stroke};

pub const MODULE_ID: &str = "qnc.module.wave-view";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn paint_wave_peaks(painter: &egui::Painter, rect: Rect, peaks: &[f32], color: Color32) {
    if peaks.is_empty() || rect.width() < 2.0 {
        return;
    }
    let mid = rect.center().y;
    let half = rect.height() * 0.48;
    for (x_index, range) in peak_ranges_for_width(peaks.len(), rect.width()).enumerate() {
        let max_peak = peaks[range]
            .iter()
            .map(|peak| peak.abs())
            .fold(0.0_f32, f32::max)
            .clamp(0.0, 1.0);
        let x = rect.left() + x_index as f32 + 0.5;
        let amp = max_peak * half;
        painter.line_segment(
            [egui::pos2(x, mid - amp), egui::pos2(x, mid + amp)],
            Stroke::new(1.0, color),
        );
    }
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
    fn at_least_one_range_is_returned_for_narrow_tracks() {
        let ranges = peak_ranges_for_width(3, 0.5).collect::<Vec<_>>();

        assert_eq!(ranges, [0..3]);
    }
}

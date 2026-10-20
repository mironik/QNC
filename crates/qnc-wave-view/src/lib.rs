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
    painter.add(egui::Shape::Mesh(cached_mesh(painter.ctx(), rect, peaks, gain, color)));
}

/// Built meshes, kept while they are painted: a wave changes only with its peaks, its rect,
/// its level or its colour (user 2026-10-20: a level step builds that one lane again).
#[derive(Clone, Default)]
struct WaveMeshes(std::collections::HashMap<u64, (std::sync::Arc<egui::Mesh>, u64)>);

/// Frames a mesh not painted is kept.
const KEEP_PASSES: u64 = 120;

fn cached_mesh(ctx: &egui::Context, rect: Rect, peaks: &[f32], gain: f32, color: Color32) -> std::sync::Arc<egui::Mesh> {
    let key = mesh_key(rect, peaks, gain, color);
    let pass = ctx.cumulative_pass_nr();
    let store = egui::Id::new("qnc-wave-meshes");
    ctx.data_mut(|data| {
        let meshes = data.get_temp_mut_or_default::<WaveMeshes>(store);
        if let Some((mesh, used)) = meshes.0.get_mut(&key) {
            *used = pass;
            return mesh.clone();
        }
        meshes.0.retain(|_, (_, used)| pass.saturating_sub(*used) < KEEP_PASSES);
        let mesh = std::sync::Arc::new(wave_mesh(rect, peaks, gain, color));
        meshes.0.insert(key, (mesh.clone(), pass));
        mesh
    })
}

/// What makes a mesh: the rect, the level, the colour and every peak (a fast fold of their
/// bits; a different wave of the same size is another key).
fn mesh_key(rect: Rect, peaks: &[f32], gain: f32, color: Color32) -> u64 {
    let mut key: u64 = 0xcbf2_9ce4_8422_2325;
    let mut fold = |value: u32| key = (key ^ u64::from(value)).wrapping_mul(0x0000_0100_0000_01b3);
    for value in [rect.min.x, rect.min.y, rect.max.x, rect.max.y, gain] {
        fold(value.to_bits());
    }
    fold(u32::from_le_bytes(color.to_array()));
    fold(peaks.len() as u32);
    peaks.iter().for_each(|peak| fold(peak.to_bits()));
    key
}

/// The wave as one filled shape: the line through the peaks of each pixel column above the
/// middle, its mirror below, the band between them filled (two points a column; user
/// 2026-10-20: cheaper than a column of its own per pixel, and smoother).
fn wave_mesh(rect: Rect, peaks: &[f32], gain: f32, color: Color32) -> egui::Mesh {
    let mid = rect.center().y;
    let half = rect.height() * 0.48;
    let mut mesh = egui::Mesh::default();
    for (x_index, range) in peak_ranges_for_width(peaks.len(), rect.width()).enumerate() {
        let max_peak = peaks[range]
            .iter()
            .map(|peak| peak.abs())
            .fold(0.0_f32, f32::max)
            .mul_add(gain, 0.0)
            .clamp(0.0, 1.0);
        let amp = max_peak * half;
        let x = rect.left() + x_index as f32 + 0.5;
        mesh.colored_vertex(egui::pos2(x, mid - amp), color);
        mesh.colored_vertex(egui::pos2(x, mid + amp), color);
        if x_index > 0 {
            let at = 2 * x_index as u32;
            mesh.add_triangle(at - 2, at - 1, at);
            mesh.add_triangle(at - 1, at + 1, at);
        }
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
    fn a_wave_is_one_filled_band_two_points_a_column() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(4.0, 10.0));
        let mesh = wave_mesh(rect, &[0.5, 0.0, 1.0, 0.25], 1.0, Color32::WHITE);
        assert_eq!(mesh.vertices.len(), 4 * 2);
        assert_eq!(mesh.indices.len(), 3 * 6, "two triangles between neighbouring columns");
        assert!((mesh.vertices[4].pos.y - (5.0 - 4.8)).abs() < 1e-4, "full scale reaches 48 % of the lane above the middle");
        assert_ne!(mesh_key(rect, &[0.5], 1.0, Color32::WHITE), mesh_key(rect, &[0.5], 2.0, Color32::WHITE), "another level, another mesh");
    }

    #[test]
    fn at_least_one_range_is_returned_for_narrow_tracks() {
        let ranges = peak_ranges_for_width(3, 0.5).collect::<Vec<_>>();

        assert_eq!(ranges, [0..3]);
    }
}

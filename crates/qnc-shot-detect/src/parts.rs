//! The usable parts of a clip (user 2026-10-27: "a camera clip is one take, its cuts say
//! nothing; find the parts a cutter can take as B-roll"). Between each two neighbouring
//! pictures the camera's move is measured as the shift of the whole picture: a part is a run
//! where the camera stands (or barely moves, a held hand) or pans evenly, sharp and not black,
//! inside one shot, at least `PART_LEAST_SECONDS` long. A jerk, a whip pan or a soft or black
//! stretch ends a part.

use crate::{histogram_distance, measure, Measure, Picture, Shot, CUT_LEAST, SOFT_SHARE};

/// The shortest usable part.
pub const PART_LEAST_SECONDS: f64 = 3.0;
/// The camera stands: the picture moves at most this share of its width a second.
const STEADY: f32 = 0.04;
/// Faster than this is a whip pan or a jerk, never usable.
const PAN_MOST: f32 = 0.5;
/// An even pan: two neighbouring moves differ by at most this share of the larger.
const PAN_EVEN: f32 = 0.5;
/// The farthest shift searched between two pictures, as a share of their width (a second at
/// the fastest usable pan is half the width; the pictures are about half a second apart).
const SEARCH_SHARE: f32 = 0.2;

/// How the camera holds a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Camera {
    Steady,
    Pan,
}

impl Camera {
    pub fn name(self) -> &'static str {
        match self {
            Self::Steady => "steady",
            Self::Pan => "pan",
        }
    }
}

/// One usable part: clip frames `[from, to)`, how the camera holds it and its sharpest frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub from: u64,
    pub to: u64,
    pub camera: Camera,
    pub key_frame: u64,
}

/// The camera's move between two pictures: the shift of the whole picture (pixels) and how
/// much the pictures still differ after it (0-1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Move {
    pub dx: i32,
    pub dy: i32,
    pub residual: f32,
}

/// The usable parts of a clip from its pictures (as for `shots`), its `shots` and its frame
/// rate.
pub fn parts(mut pictures: Vec<Picture>, shots: &[Shot], rate: f64) -> Vec<Part> {
    pictures.retain(|p| p.width >= 16 && p.height >= 16 && p.luma.len() == p.width * p.height);
    pictures.sort_by_key(|p| p.frame);
    pictures.dedup_by_key(|p| p.frame);
    if pictures.len() < 2 || !(rate > 0.0) {
        return Vec::new();
    }
    let measures: Vec<Measure> = pictures.iter().map(measure).collect();
    let sharp = crate::upper_quarter(measures.iter().map(|m| m.sharpness).collect());
    let usable_picture = |m: &Measure| m.brightness > crate::BLACK_LUMA && !(sharp > 0.0 && m.sharpness < SOFT_SHARE * sharp);
    // Each pair of neighbouring pictures: its camera, or none when the pair is not usable.
    let mut velocity = Vec::with_capacity(pictures.len() - 1);
    let mut kind: Vec<Option<Camera>> = Vec::with_capacity(pictures.len() - 1);
    for (i, pair) in pictures.windows(2).enumerate() {
        let seconds = (pair[1].frame - pair[0].frame) as f64 / rate;
        let same_shot = shot_of(shots, pair[0].frame) == shot_of(shots, pair[1].frame)
            && histogram_distance(&measures[i].histogram, &measures[i + 1].histogram) < CUT_LEAST;
        let moved = camera_move(&pair[0], &pair[1]);
        let v = (moved.dx as f32 / pair[0].width as f32 / seconds as f32, moved.dy as f32 / pair[0].width as f32 / seconds as f32);
        let speed = v.0.hypot(v.1);
        velocity.push(v);
        let ok = same_shot && usable_picture(&measures[i]) && usable_picture(&measures[i + 1]) && seconds <= 1.5;
        kind.push(match () {
            _ if !ok || speed > PAN_MOST => None,
            _ if speed <= STEADY => Some(Camera::Steady),
            _ => Some(Camera::Pan),
        });
    }
    // A pan is a part only while its moves are even.
    for i in 0..kind.len() {
        if kind[i] != Some(Camera::Pan) {
            continue;
        }
        let even = |j: usize| {
            let (a, b) = (velocity[i], velocity[j]);
            let larger = a.0.hypot(a.1).max(b.0.hypot(b.1));
            kind[j] == Some(Camera::Pan) && (a.0 - b.0).hypot(a.1 - b.1) <= PAN_EVEN * larger
        };
        let before = i > 0 && even(i - 1);
        let after = i + 1 < kind.len() && even(i + 1);
        if !before && !after {
            kind[i] = None;
        }
    }
    let least = (PART_LEAST_SECONDS * rate).round() as u64;
    let mut found = Vec::new();
    let mut start = 0;
    while start < kind.len() {
        let Some(camera) = kind[start] else {
            start += 1;
            continue;
        };
        let mut end = start;
        while end + 1 < kind.len() && kind[end + 1] == Some(camera) {
            end += 1;
        }
        let (from, to) = (pictures[start].frame, pictures[end + 1].frame);
        if to - from >= least {
            let key = (start..=end + 1).max_by(|&a, &b| measures[a].sharpness.total_cmp(&measures[b].sharpness)).unwrap_or(start);
            found.push(Part { from, to, camera, key_frame: pictures[key].frame });
        }
        start = end + 1;
    }
    found
}

fn shot_of(shots: &[Shot], frame: u64) -> Option<usize> {
    shots.iter().position(|shot| shot.from <= frame && frame < shot.to)
}

/// The shift of picture `b` against `a` that matches them best: searched on pictures half the
/// size, then refined by a pixel at full size.
pub fn camera_move(a: &Picture, b: &Picture) -> Move {
    let (small_a, small_b) = (half(a), half(b));
    let reach = |side: usize| ((side as f32 * SEARCH_SHARE) as i32).max(1);
    let (rx, ry) = (reach(small_a.width), reach(small_a.height));
    let mut best = (0, 0, f32::MAX);
    for dy in -ry..=ry {
        for dx in -rx..=rx {
            let cost = difference(&small_a, &small_b, dx, dy, (rx, ry));
            if cost < best.2 {
                best = (dx, dy, cost);
            }
        }
    }
    let margin = (2 * rx + 1, 2 * ry + 1);
    let mut fine = (best.0 * 2, best.1 * 2, f32::MAX);
    for dy in best.1 * 2 - 1..=best.1 * 2 + 1 {
        for dx in best.0 * 2 - 1..=best.0 * 2 + 1 {
            let cost = difference(a, b, dx, dy, margin);
            if cost < fine.2 {
                fine = (dx, dy, cost);
            }
        }
    }
    Move { dx: fine.0, dy: fine.1, residual: fine.2 }
}

/// The mean difference (0-1) of `a` and `b` shifted by (`dx`, `dy`), over `a` without a border
/// of (`mx`, `my`) pixels.
fn difference(a: &Picture, b: &Picture, dx: i32, dy: i32, (mx, my): (i32, i32)) -> f32 {
    let (w, h) = (a.width.min(b.width) as i32, a.height.min(b.height) as i32);
    let (mut sum, mut count) = (0u64, 0u64);
    for y in my..h - my {
        let by = y + dy;
        if by < 0 || by >= h {
            continue;
        }
        for x in mx..w - mx {
            let bx = x + dx;
            if bx < 0 || bx >= w {
                continue;
            }
            sum += a.luma[(y * a.width as i32 + x) as usize].abs_diff(b.luma[(by * b.width as i32 + bx) as usize]) as u64;
            count += 1;
        }
    }
    if count == 0 {
        return f32::MAX;
    }
    sum as f32 / count as f32 / 255.0
}

fn half(picture: &Picture) -> Picture {
    let (w, h) = (picture.width / 2, picture.height / 2);
    let at = |x: usize, y: usize| picture.luma[y * picture.width + x] as u16;
    let luma = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w * 2, i / w * 2);
            ((at(x, y) + at(x + 1, y) + at(x, y + 1) + at(x + 1, y + 1)) / 4) as u8
        })
        .collect();
    Picture { frame: picture.frame, width: w, height: h, luma }
}

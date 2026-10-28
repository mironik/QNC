//! The shots of a clip from small grey pictures taken through it (user 2026-10-27: Video AI,
//! shot analysis in AI Media Assist). Pure: it gets the pictures and the clip's length in
//! frames and gives the shots, each with what it looks like (black, soft, moving, still) and
//! its sharpest picture. It reads no media, decodes nothing and writes nowhere.
//!
//! A camera clip is one take; its "shots" are where the picture changes to another scene
//! (a swing of the camera, a cut of a clip edited before). A change is a jump of the brightness
//! histogram between two neighbouring pictures, far above the changes around it.

pub const MODULE_ID: &str = "qnc.module.shot-detect";

mod parts;
pub use parts::{camera_move, parts, Camera, Move, Part, PART_LEAST_SECONDS};

/// One small grey picture of the clip at `frame` (row by row, `width * height` bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub frame: u64,
    pub width: usize,
    pub height: usize,
    pub luma: Vec<u8>,
}

/// What a shot looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Nearly all of it black (lens cap, the camera started in the dark).
    Black,
    /// Clearly less sharp than the clip's sharp pictures (out of focus, shaking).
    Soft,
    /// The picture keeps changing (a moving camera or a busy scene).
    Moving,
    /// The picture hardly changes (a camera on a tripod, a still scene).
    Still,
}

impl Look {
    pub fn name(self) -> &'static str {
        match self {
            Self::Black => "black",
            Self::Soft => "soft",
            Self::Moving => "moving",
            Self::Still => "still",
        }
    }
}

/// One shot: clip frames `[from, to)`, its look and the frame of its sharpest picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    pub from: u64,
    pub to: u64,
    pub looks: Vec<Look>,
    pub key_frame: u64,
    /// Median brightness (0-255), sharpness and change between neighbouring pictures (0-1).
    pub brightness: f32,
    pub sharpness: f32,
    pub motion: f32,
}

/// A change of scene: the histogram distance (0-1) at least this, or far above the ones around.
const CUT_ALWAYS: f32 = 0.5;
const CUT_LEAST: f32 = 0.25;
const CUT_OVER_AROUND: f32 = 3.0;
/// The pictures around a change it is weighed against, each side.
const AROUND: usize = 4;
/// A shot has at least this many pictures (a short flash is not a shot).
const SHOT_LEAST: usize = 2;
/// Black: mean brightness at most this, for at least that share of its pictures.
const BLACK_LUMA: f32 = 24.0;
const BLACK_SHARE: f32 = 0.8;
/// Soft: median sharpness under this share of the clip's sharp pictures (its upper quarter).
const SOFT_SHARE: f32 = 0.4;
/// Moving and still, by the median change of neighbouring pictures.
const MOVING: f32 = 0.06;
const STILL: f32 = 0.015;

struct Measure {
    frame: u64,
    brightness: f32,
    sharpness: f32,
    histogram: [f32; 32],
}

/// The shots of a clip of `frames` frames from its pictures (in any order; at most one a frame).
pub fn shots(mut pictures: Vec<Picture>, frames: u64) -> Vec<Shot> {
    pictures.retain(|picture| picture.width >= 3 && picture.height >= 3 && picture.luma.len() == picture.width * picture.height && picture.frame < frames.max(1));
    pictures.sort_by_key(|picture| picture.frame);
    pictures.dedup_by_key(|picture| picture.frame);
    if pictures.is_empty() {
        return Vec::new();
    }
    let measures: Vec<Measure> = pictures.iter().map(measure).collect();
    // The change from each picture to the next.
    let distance: Vec<f32> = measures.windows(2).map(|pair| histogram_distance(&pair[0].histogram, &pair[1].histogram)).collect();
    let motion: Vec<f32> = pictures.windows(2).map(|pair| mean_difference(&pair[0], &pair[1])).collect();
    let mut starts = vec![0usize];
    for (at, &jump) in distance.iter().enumerate() {
        let around = median(distance.iter().enumerate().filter(|(i, _)| i.abs_diff(at) <= AROUND && *i != at).map(|(_, d)| *d).collect());
        let cut = jump >= CUT_ALWAYS || (jump >= CUT_LEAST && jump >= CUT_OVER_AROUND * around + 0.02);
        if cut {
            starts.push(at + 1);
        }
    }
    // A shot shorter than SHOT_LEAST is a flash: it joins the shot before, and when the picture
    // after it is the scene before it again, so does the shot after.
    let mut index = 1;
    while index < starts.len() {
        let end = starts.get(index + 1).copied().unwrap_or(measures.len());
        if end - starts[index] >= SHOT_LEAST {
            index += 1;
            continue;
        }
        let flash = starts.remove(index);
        if index < starts.len() && histogram_distance(&measures[flash - 1].histogram, &measures[starts[index]].histogram) < CUT_LEAST {
            starts.remove(index);
        }
    }
    let sharp = upper_quarter(measures.iter().map(|m| m.sharpness).collect());
    let mut shots = Vec::with_capacity(starts.len());
    for (index, &start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(measures.len());
        let part = &measures[start..end];
        let brightness = median(part.iter().map(|m| m.brightness).collect());
        let sharpness = median(part.iter().map(|m| m.sharpness).collect());
        // The changes inside the shot (not the one into it).
        let moved = median(motion.get(start..end.saturating_sub(1)).unwrap_or(&[]).to_vec());
        let dark = part.iter().filter(|m| m.brightness <= BLACK_LUMA).count() as f32 / part.len() as f32;
        let mut looks = Vec::new();
        if dark >= BLACK_SHARE {
            looks.push(Look::Black);
        } else {
            if sharp > 0.0 && sharpness < SOFT_SHARE * sharp {
                looks.push(Look::Soft);
            }
            if end - start >= 2 {
                if moved >= MOVING {
                    looks.push(Look::Moving);
                } else if moved <= STILL {
                    looks.push(Look::Still);
                }
            }
        }
        let key = part.iter().max_by(|a, b| a.sharpness.total_cmp(&b.sharpness)).map_or(part[0].frame, |m| m.frame);
        let from = if index == 0 { 0 } else { part[0].frame };
        let to = starts.get(index + 1).map_or(frames, |&next| measures[next].frame);
        shots.push(Shot { from, to, looks, key_frame: key, brightness, sharpness, motion: moved });
    }
    shots
}

fn measure(picture: &Picture) -> Measure {
    let (w, h, luma) = (picture.width, picture.height, &picture.luma);
    let mut histogram = [0f32; 32];
    let mut sum = 0u64;
    for &value in luma {
        histogram[(value >> 3) as usize] += 1.0;
        sum += value as u64;
    }
    let count = luma.len() as f32;
    histogram.iter_mut().for_each(|bin| *bin /= count);
    // Sharpness: the mean square of the picture's Laplacian (edges and detail).
    let mut energy = 0f64;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let at = |dx: isize, dy: isize| luma[(y as isize + dy) as usize * w + (x as isize + dx) as usize] as i32;
            let laplace = 4 * at(0, 0) - at(-1, 0) - at(1, 0) - at(0, -1) - at(0, 1);
            energy += (laplace * laplace) as f64;
        }
    }
    let sharpness = (energy / ((w - 2) * (h - 2)) as f64) as f32;
    Measure { frame: picture.frame, brightness: sum as f32 / count, sharpness, histogram }
}

fn histogram_distance(a: &[f32; 32], b: &[f32; 32]) -> f32 {
    0.5 * a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>()
}

/// How much two pictures differ, 0-1 (they are compared over their common size).
fn mean_difference(a: &Picture, b: &Picture) -> f32 {
    let (w, h) = (a.width.min(b.width), a.height.min(b.height));
    let mut sum = 0u64;
    for y in 0..h {
        for x in 0..w {
            sum += a.luma[y * a.width + x].abs_diff(b.luma[y * b.width + x]) as u64;
        }
    }
    sum as f32 / (w * h) as f32 / 255.0
}

fn median(mut values: Vec<f32>) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    values[values.len() / 2]
}

fn upper_quarter(mut values: Vec<f32>) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    values[values.len() * 3 / 4]
}

#[cfg(test)]
mod tests;

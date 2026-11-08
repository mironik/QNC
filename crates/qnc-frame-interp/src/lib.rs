//! AI in-between pictures of a cut (user 2026-11-07: "joining a composite statement with AI
//! generated in-between pictures"; docs/97 3.6 smooth cut). The pictures around a cut are
//! replaced: the last `n/2` before it and the first `n/2` after it give way to `n` pictures made
//! between the picture before them and the picture after them, so the length and the sound stay
//! (as Morph Cut). The model is RIFE 4.x run by rife-ncnn-vulkan (MIT, nihui) on the graphics
//! card through Vulkan; the program and its model folder are this computer's (not hardcoded:
//! the caller gives them). It decodes nothing and writes only in the folder it is given.

use std::path::{Path, PathBuf};
use std::process::Command;

pub const MODULE_ID: &str = "qnc.module.frame-interp";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The program that runs the model and the model's folder.
#[derive(Debug, Clone)]
pub struct Interpolator {
    pub program: PathBuf,
    pub model: PathBuf,
}

/// The pictures of a cut a smooth cut of `n` pictures replaces, around the cut between the
/// last picture `out_last` of the part before it and the first `in_first` of the part after it:
/// the picture the transition starts from (before them), the one it ends on (after them), and
/// how many come from each side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub from_picture: u64,
    pub to_picture: u64,
    pub before: u64,
    pub after: u64,
}

/// The span of a smooth cut of `n` pictures (`n` even is shared equally, an odd one gives the
/// extra picture to the part after the cut).
pub fn span(out_last: u64, in_first: u64, n: u64) -> Span {
    let before = n / 2;
    let after = n - before;
    Span { from_picture: out_last.saturating_sub(before), to_picture: in_first + after, before, after }
}

/// The id of the clip of the in-between pictures of one cut: the same two parts cut at the
/// same pictures with the same count are the same clip, whenever they are asked for.
pub fn clip_id(before_clip: &str, after_clip: &str, out_last: u64, in_first: u64, n: u64) -> String {
    let text = format!("{before_clip}|{after_clip}|{out_last}|{in_first}|{n}");
    let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3));
    format!("smooth-{hash:016x}")
}

/// The times between the two pictures of the `n` in-between ones, `k / (n + 1)`.
pub fn times(n: u64) -> Vec<f64> {
    (1..=n).map(|k| k as f64 / (n + 1) as f64).collect()
}

impl Interpolator {
    /// The `n` pictures between `from` and `to` (PNG files of the same size) written into
    /// `out_dir` as `inbetween_01.png`..; their paths in order.
    ///
    /// The program reads a time step in the computer's decimal format (a comma here), so the
    /// times are given as a count instead: the two pictures as a sequence asked for
    /// `2 * (n + 1)` pictures, which are at times `i / (n + 1)`; the ones at `1..=n` are kept.
    /// The model is loaded once for all of them.
    pub fn between(&self, from: &Path, to: &Path, n: u64, out_dir: &Path) -> Result<Vec<PathBuf>, String> {
        let io = |error: std::io::Error| format!("{}: {error}", out_dir.display());
        let (pair, made_dir) = (out_dir.join("pair"), out_dir.join("made"));
        for dir in [out_dir, &pair, &made_dir] {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        std::fs::copy(from, pair.join("00000001.png")).map_err(io)?;
        std::fs::copy(to, pair.join("00000002.png")).map_err(io)?;
        let output = Command::new(&self.program)
            .arg("-i")
            .arg(&pair)
            .arg("-o")
            .arg(&made_dir)
            .arg("-n")
            .arg((2 * (n + 1)).to_string())
            .arg("-m")
            .arg(&self.model)
            .output()
            .map_err(|error| format!("{}: {error}", self.program.display()))?;
        if !output.status.success() {
            return Err(format!("{}: {}", self.program.display(), String::from_utf8_lossy(&output.stderr).trim()));
        }
        let mut made = Vec::new();
        for index in 1..=n {
            let picture = made_dir.join(format!("{:08}.png", index + 1));
            let out = out_dir.join(format!("inbetween_{index:02}.png"));
            std::fs::rename(&picture, &out).map_err(|error| format!("{}: {error}", picture.display()))?;
            made.push(out);
        }
        Ok(made)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_smooth_cut_replaces_the_pictures_around_the_cut_and_keeps_the_length() {
        // The cut between picture 5269 (the last before) and 5374 (the first after), 8 pictures.
        let around = span(5269, 5374, 8);
        assert_eq!(around, Span { from_picture: 5265, to_picture: 5378, before: 4, after: 4 });
        assert_eq!(times(3), vec![0.25, 0.5, 0.75]);
        assert_eq!(span(10, 20, 5).after, 3, "the odd picture after the cut");
    }
}

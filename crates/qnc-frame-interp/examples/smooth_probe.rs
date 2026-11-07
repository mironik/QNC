//! A probe of a smooth cut on real pictures (user 2026-11-07): reads one second before and one
//! after a cut of an original (read only), makes the in-between pictures with RIFE and writes
//! into OUT_DIR the hard cut and the smooth cut as two clips and a sheet of the transition.
//!
//! Usage: smooth_probe FFMPEG RIFE_PROGRAM RIFE_MODEL ORIGINAL FPS OUT_LAST IN_FIRST N OUT_DIR
use std::path::{Path, PathBuf};
use std::process::Command;

fn run(command: &mut Command) -> Result<(), String> {
    let output = command.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).lines().rev().take(4).collect::<Vec<_>>().join(" | "))
    }
}

/// `count` pictures of `original` from picture `first` as PNG files `prefix_0001.png`..
fn pictures(ffmpeg: &str, original: &str, fps: f64, first: u64, count: u64, dir: &Path, prefix: &str) -> Result<Vec<PathBuf>, String> {
    let pattern = dir.join(format!("{prefix}_%04d.png"));
    run(Command::new(ffmpeg)
        .args(["-v", "error", "-y", "-ss", &format!("{:.6}", first as f64 / fps), "-i", original, "-frames:v", &count.to_string(), "-an", "-pix_fmt", "rgb24"])
        .arg(&pattern))?;
    Ok((1..=count).map(|index| dir.join(format!("{prefix}_{index:04}.png"))).collect())
}

/// The pictures `list` as a clip at `fps` (a numbered copy in `dir`, then encoded).
fn clip(ffmpeg: &str, list: &[PathBuf], fps: f64, dir: &Path, name: &str) -> Result<PathBuf, String> {
    let seq = dir.join(format!("{name}_seq"));
    std::fs::create_dir_all(&seq).map_err(|error| error.to_string())?;
    for (index, picture) in list.iter().enumerate() {
        std::fs::copy(picture, seq.join(format!("{:04}.png", index + 1))).map_err(|error| error.to_string())?;
    }
    let out = dir.join(format!("{name}.mp4"));
    run(Command::new(ffmpeg)
        .args(["-v", "error", "-y", "-framerate", &fps.to_string(), "-i"])
        .arg(seq.join("%04d.png"))
        .args(["-c:v", "mpeg4", "-q:v", "2", "-pix_fmt", "yuv420p"])
        .arg(&out))?;
    Ok(out)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [ffmpeg, program, model, original, fps, out_last, in_first, n, out_dir] = args.as_slice() else {
        return Err("usage: smooth_probe FFMPEG RIFE_PROGRAM RIFE_MODEL ORIGINAL FPS OUT_LAST IN_FIRST N OUT_DIR".into());
    };
    let fps: f64 = fps.parse().map_err(|_| "FPS")?;
    let (out_last, in_first, n): (u64, u64, u64) = (out_last.parse().map_err(|_| "OUT_LAST")?, in_first.parse().map_err(|_| "IN_FIRST")?, n.parse().map_err(|_| "N")?);
    let dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let second = fps.round() as u64;
    let before_first = out_last + 1 - second;
    let started = std::time::Instant::now();
    let before = pictures(ffmpeg, original, fps, before_first, second, &dir, "before")?;
    let after = pictures(ffmpeg, original, fps, in_first, second, &dir, "after")?;
    println!("read {} + {} pictures in {:.1} s", before.len(), after.len(), started.elapsed().as_secs_f64());
    let span = qnc_frame_interp::span(out_last, in_first, n);
    let from = &before[(span.from_picture - before_first) as usize];
    let to = &after[(span.to_picture - in_first) as usize];
    let interpolator = qnc_frame_interp::Interpolator { program: program.into(), model: model.into() };
    let started = std::time::Instant::now();
    let made = interpolator.between(from, to, n, &dir.join("inbetween"))?;
    println!("{n} in-between pictures in {:.1} s ({} .. {})", started.elapsed().as_secs_f64(), span.from_picture, span.to_picture);
    let cut: Vec<PathBuf> = before.iter().chain(&after).cloned().collect();
    let keep_before = (span.from_picture - before_first + 1) as usize;
    let keep_after = (span.to_picture - in_first) as usize;
    let smooth: Vec<PathBuf> = before[..keep_before].iter().chain(&made).chain(&after[keep_after..]).cloned().collect();
    assert_eq!(cut.len(), smooth.len(), "the length stays");
    println!("{}", clip(ffmpeg, &cut, fps, &dir, "cut")?.display());
    println!("{}", clip(ffmpeg, &smooth, fps, &dir, "smooth")?.display());
    // The transition on one sheet: the picture it starts from, the in-between ones, the one it ends on.
    let sheet: Vec<PathBuf> = std::iter::once(from.clone()).chain(made).chain(std::iter::once(to.clone())).collect();
    let seq = dir.join("sheet_seq");
    std::fs::create_dir_all(&seq).map_err(|error| error.to_string())?;
    for (index, picture) in sheet.iter().enumerate() {
        std::fs::copy(picture, seq.join(format!("{:04}.png", index + 1))).map_err(|error| error.to_string())?;
    }
    let columns = sheet.len().div_ceil(2);
    let sheet_file = dir.join("sheet.png");
    run(Command::new(ffmpeg)
        .args(["-v", "error", "-y", "-i"])
        .arg(seq.join("%04d.png"))
        .args(["-vf", &format!("scale=480:-1,tile={columns}x2"), "-frames:v", "1"])
        .arg(&sheet_file))?;
    println!("{}", sheet_file.display());
    Ok(())
}

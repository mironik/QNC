//! A part of an MP4 or QuickTime original written in the container of the original (user
//! 2026-10-23: Ingest SubClip, "it must be like the original"), by a stream copy of the
//! ffmpeg of the installed decoder catalog. The part starts on the intra picture at or
//! before its IN (user: "take the timecodes of the intra pictures"): the stream copy starts
//! there and the pictures of that group shown before it (they refer to the group before)
//! are dropped, so every picture of the part decodes and its frame count is exact. The
//! timecode of its first frame is written into it. The source is only read.

use std::{
    io::Read,
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

pub const MODULE_ID: &str = "qnc.module.mp4-cut";

/// The first frame of the original the part starts on (the intra picture at or before `from`).
pub fn intra_at_or_before(ffmpeg: &Path, source: &Path, from: u64, (num, den): (u64, u64)) -> Result<u64, String> {
    let start = first_picture_time(ffmpeg, source, None)?;
    let key = first_picture_time(ffmpeg, source, Some(seconds(from, (num, den))))?;
    let frames = (key - start) * num as f64 / den.max(1) as f64;
    Ok(frames.round().max(0.0) as u64)
}

/// Writes frames `first..to` of `source` to `dest` (`first` from [`intra_at_or_before`]),
/// with `timecode` (HH:MM:SS:FF of its first frame) when the original has one.
/// Returns the frames of the part.
pub fn write(ffmpeg: &Path, source: &Path, dest: &Path, (first, to): (u64, u64), rate: (u64, u64), timecode: Option<&str>, cancel: &AtomicBool) -> Result<u64, String> {
    if to <= first {
        return Err(qnc_ui_text::text("cut.range_outside_clip").into());
    }
    let format = match source.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("mp4") => "mp4",
        Some("mov") => "mov",
        _ => return Err(qnc_ui_text::text("cut.not_mp4").into()),
    };
    // The packets of the part as the cut will see them: it ends before the first intra
    // picture shown at or after `to`, so no picture of the part refers past its end.
    let span = seconds(to - first + 2 * rate.0 / rate.1.max(1), rate);
    let (base, packets) = packets(ffmpeg, source, &seconds(first, rate), &span)?;
    let end_time = (to - first) as f64 * rate.1 as f64 / rate.0.max(1) as f64;
    let end = packets.iter().find(|p| p.2 && p.1 as f64 * base >= end_time - 1e-6);
    let mut drop = String::from("lt(pts\\,0)");
    let mut length = seconds(to - first, rate);
    let mut frames = packets.iter().filter(|p| p.1 >= 0).count() as u64;
    if let Some((dts, pts, _)) = end {
        drop = format!("lt(pts\\,0)+gte(dts\\,{dts})");
        // Sound ends with the last picture: the pictures kept are those before it in time.
        let _ = pts;
        let kept = packets.iter().filter(|p| p.1 >= 0 && p.0 < *dts).count() as u64;
        length = seconds(kept, rate);
        frames = kept;
    }
    let partial = dest.with_extension("partial");
    let mut command = Command::new(ffmpeg);
    command
        .args(["-y", "-hide_banner", "-nostdin", "-v", "error", "-ss", &seconds(first, rate), "-i"])
        .arg(source)
        .args(["-t", &length, "-map", "0:v:0", "-map", "0:a?", "-c", "copy", "-map_metadata", "0"])
        // The pictures of the first group shown before its intra picture are dropped, and
        // the packets from the intra picture after the part on.
        .args(["-bsf:v", &format!("noise=drop={drop}")]);
    if let Some(timecode) = timecode {
        command.args(["-timecode", timecode]);
    }
    command.args(["-f", format]).arg(&partial).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    background(&mut command);
    let result = wait(command.spawn().map_err(|e| format!("ffmpeg cut: {e}"))?, cancel)
        .and_then(|_| std::fs::rename(&partial, dest).map_err(|e| e.to_string()))
        .map(|()| frames);
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result
}

/// The picture packets from a seek to `at` for `span` seconds, as a stream copy sees them:
/// the time base and (dts, pts, key) of each, in decoding order.
fn packets(ffmpeg: &Path, source: &Path, at: &str, span: &str) -> Result<(f64, Vec<(i64, i64, bool)>), String> {
    let mut command = Command::new(ffmpeg);
    command
        .args(["-hide_banner", "-nostdin", "-v", "error", "-ss", at, "-i"])
        .arg(source)
        .args(["-t", span, "-map", "0:v:0", "-c", "copy", "-f", "framecrc", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    background(&mut command);
    let output = command.output().map_err(|e| format!("ffmpeg cut: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let base = time_base(&text)?;
    let packets = text
        .lines()
        .filter(|line| line.starts_with("0,"))
        .filter_map(|line| {
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            Some((fields.get(1)?.parse().ok()?, fields.get(2)?.parse().ok()?, !line.contains("F=0x0")))
        })
        .collect();
    Ok((base, packets))
}

fn time_base(text: &str) -> Result<f64, String> {
    text.lines()
        .find_map(|line| line.strip_prefix("#tb 0: "))
        .and_then(|tb| tb.trim().split_once('/'))
        .and_then(|(n, d)| Some(n.trim().parse::<f64>().ok()? / d.trim().parse::<f64>().ok()?))
        .ok_or_else(|| qnc_ui_text::text("cut.index_order_not_readable").to_string())
}

/// The time of the first picture packet, from the start or from a seek to `at` (the
/// stream copy lands on the intra picture at or before it).
fn first_picture_time(ffmpeg: &Path, source: &Path, at: Option<String>) -> Result<f64, String> {
    let mut command = Command::new(ffmpeg);
    command.args(["-hide_banner", "-nostdin", "-v", "error"]);
    if let Some(at) = &at {
        command.args(["-ss", at]);
    }
    command
        .arg("-i")
        .arg(source)
        .args(["-map", "0:v:0", "-c", "copy", "-copyts", "-frames:v", "1", "-f", "framecrc", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    background(&mut command);
    let output = command.output().map_err(|e| format!("ffmpeg cut: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let base = text
        .lines()
        .find_map(|line| line.strip_prefix("#tb 0: "))
        .and_then(|tb| tb.trim().split_once('/'))
        .and_then(|(n, d)| Some((n.trim().parse::<f64>().ok()?, d.trim().parse::<f64>().ok()?)))
        .ok_or(qnc_ui_text::text("cut.index_order_not_readable"))?;
    let pts = text
        .lines()
        .find(|line| line.starts_with("0,"))
        .and_then(|line| line.split(',').nth(2))
        .and_then(|pts| pts.trim().parse::<f64>().ok())
        .ok_or(qnc_ui_text::text("cut.index_order_not_readable"))?;
    Ok(pts * base.0 / base.1)
}

/// The time of frame `frame` at `rate`, written exactly enough for a seek.
fn seconds(frame: u64, (num, den): (u64, u64)) -> String {
    format!("{:.6}", frame as f64 * den as f64 / num.max(1) as f64)
}

fn wait(mut child: std::process::Child, cancel: &AtomicBool) -> Result<(), String> {
    let mut stderr = child.stderr.take();
    loop {
        if cancel.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(qnc_ui_text::text("cut.interrupted").into());
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(status) => {
                let mut text = String::new();
                if let Some(stderr) = stderr.as_mut() {
                    let _ = stderr.take(4096).read_to_string(&mut text);
                }
                return Err(format!("ffmpeg cut: {status} {}", text.trim()));
            }
            None => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

/// Background work: below normal priority on Windows, so the player keeps the processor.
fn background(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_time_is_exact_enough_for_a_seek() {
        assert_eq!(seconds(146, (50, 1)), "2.920000");
        assert_eq!(seconds(3, (30000, 1001)), "0.100100");
    }

    /// On a real original (`QNC_MP4_SAMPLE`, read only; `QNC_FFMPEG`), the part starts on an
    /// intra picture at or before its IN.
    #[test]
    fn a_real_original_is_cut_from_an_intra_picture() {
        let (Ok(sample), Ok(ffmpeg)) = (std::env::var("QNC_MP4_SAMPLE"), std::env::var("QNC_FFMPEG")) else { return };
        let (source, ffmpeg) = (Path::new(&sample), Path::new(&ffmpeg));
        let first = intra_at_or_before(ffmpeg, source, 150, (50, 1)).unwrap();
        assert!(first <= 150);
        let dest = std::env::temp_dir().join("qnc-mp4-cut-test.mp4");
        write(ffmpeg, source, &dest, (first, 250), (50, 1), Some("01:00:00:00"), &AtomicBool::new(false)).unwrap();
        assert!(dest.is_file());
        let _ = std::fs::remove_file(dest);
    }
}

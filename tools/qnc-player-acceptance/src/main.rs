//! Automatic acceptance of the Broadcast Player (AGENTS 8.3 point 6, user request
//! 2026-10-02: "stabiliziraj player"). It plays clips and the story program of the
//! active project through the same `qnc-source-preview` the forms use, without a form,
//! and measures what a viewer gets: time to ready, Play to the first new picture, how
//! many of the frames the player advanced reached the monitor, the longest gap between
//! pictures, stutters, and the stalls the player logged.
//!
//! It refuses to run on a real project: the preview writes "a player works" into the
//! project database, so the project the root names must live inside that root (an
//! isolated copy, AGENTS 0.4). The card is only read.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use qnc_source_preview::{PreviewContext, SourcePreview};

struct Args {
    root: PathBuf,
    clips: Vec<String>,
    program_from: Vec<u64>,
    seconds: u64,
}

fn args() -> Result<Args, String> {
    let mut parsed = Args { root: PathBuf::new(), clips: Vec::new(), program_from: Vec::new(), seconds: 10 };
    let mut list = std::env::args().skip(1);
    while let Some(arg) = list.next() {
        let value = list.next().ok_or(format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--root" => parsed.root = PathBuf::from(value),
            "--clip" => parsed.clips.push(value),
            "--program" => parsed.program_from.push(value.parse().map_err(|_| "--program FRAME")?),
            "--seconds" => parsed.seconds = value.parse().map_err(|_| "--seconds N")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if parsed.root.as_os_str().is_empty() {
        return Err("usage: qnc-player-acceptance --root <isolated QNC root> [--clip NAME]... [--program FRAME]... [--seconds N]".into());
    }
    Ok(parsed)
}

/// What one play gave.
#[derive(Default)]
struct Run {
    name: String,
    prepare_ms: Option<u128>,
    first_picture_ms: Option<u128>,
    frames_advanced: u64,
    pictures: u64,
    longest_gap_ms: u128,
    longest_gap_at: u64,
    stutters: u64,
    error: Option<String>,
    started: u128,
    ended: u128,
    /// Each new picture on the monitor: when it came (unix ns) and its frame.
    pictures_at: Vec<(u128, u64)>,
    /// Frames per second of the played source (num, den).
    rate: (u64, u64),
}

fn unix_ns() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos())
}

fn unix_ms() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis())
}

/// Polls until the player can start, or an error, or the timeout.
fn wait_ready(preview: &mut SourcePreview, timeout: Duration) -> Result<u128, String> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        preview.poll();
        if let Some(error) = preview.player_view().error.clone() {
            if !error.contains("NotFinal") {
                return Err(error);
            }
        }
        if preview.player_view().can_start_playback() {
            return Ok(start.elapsed().as_millis());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    Err(format!("not ready in {} s", timeout.as_secs()))
}

/// Plays for `seconds` from where the player stands and measures the pictures.
fn play(preview: &mut SourcePreview, run: &mut Run, seconds: u64) {
    let frame_ms = preview
        .player_view()
        .source_timebase()
        .map_or(40.0, |tb| 1000.0 * tb.fps_den as f64 / tb.fps_num as f64);
    let first_frame = preview.player_view().confirmed_source_frame().unwrap_or(0);
    if let Some(tb) = preview.player_view().source_timebase() {
        run.rate = (tb.fps_num as u64, tb.fps_den as u64);
    }
    let mut last_key = preview.view().monitor_frame.as_ref().map(|f| (f.generation, f.sequence));
    preview.toggle_play();
    let start = Instant::now();
    let mut last_picture = start;
    let mut last_frame = first_frame;
    while start.elapsed() < Duration::from_secs(seconds) {
        preview.poll();
        let now = Instant::now();
        if let Some(frame) = preview.player_view().confirmed_source_frame() {
            last_frame = last_frame.max(frame);
        }
        let key = preview.view().monitor_frame.as_ref().map(|f| (f.generation, f.sequence));
        if key.is_some() && key != last_key {
            last_key = key;
            run.pictures += 1;
            if let Some(picture) = preview.player_view().picture.as_ref() {
                run.pictures_at.push((unix_ns(), picture.header.frame));
            }
            if run.first_picture_ms.is_none() {
                run.first_picture_ms = Some(now.duration_since(start).as_millis());
            } else {
                let gap = now.duration_since(last_picture).as_millis();
                if gap > run.longest_gap_ms {
                    run.longest_gap_ms = gap;
                    run.longest_gap_at = last_frame;
                }
                // More than 3 frame times: a picture the viewer misses (the 1 ms poll on
                // Windows can itself be late by about 15 ms).
                if gap as f64 > 3.0 * frame_ms {
                    run.stutters += 1;
                }
            }
            last_picture = now;
        }
        if let Some(error) = preview.player_view().error.clone() {
            run.error = Some(error);
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    if preview.player_view().playing() {
        preview.toggle_play();
    }
    run.frames_advanced = last_frame.saturating_sub(first_frame);
}

/// The stalls and video decoder reopens the player logged during a run.
fn logged(root: &Path, from: u128, to: u128) -> (u64, u64) {
    let log = qnc_dev_diagnostics::log_path(root, qnc_dev_diagnostics::DiagnosticsStream::Player);
    let text = std::fs::read_to_string(log).unwrap_or_default();
    let (mut stalls, mut reopens) = (0, 0);
    for line in text.lines() {
        let Some((stamp, rest)) = line.split_once(' ') else { continue };
        let Ok(stamp) = stamp.parse::<u128>() else { continue };
        if stamp < from || stamp > to {
            continue;
        }
        if rest.starts_with("player-rebuffer reason=") {
            stalls += 1;
        } else if rest.starts_with("player-video reopen") {
            reopens += 1;
        }
    }
    (stalls, reopens)
}

/// Picture minus sound, in frames, over a run.
struct AvOffsets {
    median: f64,
    lag_worst: f64,
    lead_worst: f64,
    points: usize,
}

/// For every picture that reached the monitor, the frame being heard at that moment:
/// from the `AV_A` lines of the player (the sample the audio device plays and when),
/// carried to the picture's time at the sample rate. Positive: picture ahead of sound.
fn av_offsets(root: &Path, run: &Run) -> Option<AvOffsets> {
    let (num, den) = run.rate;
    if num == 0 || den == 0 || run.pictures_at.is_empty() {
        return None;
    }
    let log = qnc_dev_diagnostics::log_path(root, qnc_dev_diagnostics::DiagnosticsStream::Player);
    let text = std::fs::read_to_string(log).ok()?;
    let field = |line: &str, key: &str| -> Option<u128> {
        line.split_whitespace().find_map(|part| part.strip_prefix(key)?.parse().ok())
    };
    let audio: Vec<(u128, u128, u128)> = text
        .lines()
        .filter(|line| line.contains(" AV_A "))
        .filter_map(|line| Some((field(line, "unix_ns=")?, field(line, "sample=")?, field(line, "rate=")?)))
        .filter(|(at, _, _)| *at / 1_000_000 >= run.started && *at / 1_000_000 <= run.ended)
        .collect();
    let mut offsets: Vec<f64> = run
        .pictures_at
        .iter()
        .filter_map(|(at, frame)| {
            let (heard_at, sample, rate) = audio.iter().min_by_key(|(t, _, _)| t.abs_diff(*at))?;
            if heard_at.abs_diff(*at) > 300_000_000 || *rate == 0 {
                return None;
            }
            let now = *sample as f64 + (*at as f64 - *heard_at as f64) * *rate as f64 / 1e9;
            let heard_frame = now * num as f64 / (*rate as f64 * den as f64);
            Some(*frame as f64 - heard_frame)
        })
        .collect();
    if offsets.is_empty() {
        return None;
    }
    offsets.sort_by(f64::total_cmp);
    Some(AvOffsets {
        median: offsets[offsets.len() / 2],
        lag_worst: offsets[offsets.len() / 100],
        lead_worst: offsets[offsets.len() - 1 - offsets.len() / 100],
        points: offsets.len(),
    })
}

fn measure(preview: &mut SourcePreview, run: &mut Run, seconds: u64) {
    run.started = unix_ms();
    match wait_ready(preview, Duration::from_secs(20)) {
        Ok(ms) => {
            run.prepare_ms = Some(ms);
            play(preview, run, seconds);
        }
        Err(error) => run.error = Some(error),
    }
    run.ended = unix_ms();
}

fn main() -> Result<(), String> {
    let args = args()?;
    std::env::set_var("QNC_ROOT", &args.root);
    std::env::set_var("QNC_PLAYER_DIAGNOSTICS", "1");
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(&args.root).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let settings = reader.settings_reader();
    let project_dir = settings
        .local_workspace_dir(&snapshot.settings)
        .map_err(|e| e.to_string())?
        .ok_or("the project has no local directory")?;
    let root = args.root.canonicalize().map_err(|e| e.to_string())?;
    let dir = project_dir.canonicalize().map_err(|e| e.to_string())?;
    if !dir.starts_with(&root) {
        return Err(format!(
            "refused: the project {} is not inside {} (use an isolated copy)",
            dir.display(),
            root.display()
        ));
    }
    let content = qnc_content_read::ContentReader::for_project(settings, &snapshot.settings)?;
    let clips = content.summaries()?;
    let bindings = qnc_source_bindings::load(&args.root)?;
    let mut preview = SourcePreview::new();
    preview.configure(PreviewContext::new(settings.clone(), snapshot.settings.clone(), content, bindings));

    let mut runs = Vec::new();
    for wanted in &args.clips {
        let mut run = Run { name: format!("clip {wanted}"), ..Run::default() };
        match clips.iter().find(|clip| clip.name.contains(wanted.as_str())) {
            Some(clip) => {
                preview.open(&clip.clip_id);
                measure(&mut preview, &mut run, args.seconds);
            }
            None => run.error = Some("no such clip".into()),
        }
        runs.push(run);
    }
    for from in &args.program_from {
        let mut run = Run { name: format!("program from {from}"), ..Run::default() };
        preview.show_program_frame(*from, true);
        measure(&mut preview, &mut run, args.seconds);
        runs.push(run);
    }
    preview.close();
    std::thread::sleep(Duration::from_millis(300));

    let mut failed = false;
    let mut report = Vec::new();
    for run in &runs {
        let (stalls, reopens) = logged(&args.root, run.started, run.ended + 200);
        let shown = if run.frames_advanced > 0 {
            100.0 * run.pictures as f64 / run.frames_advanced as f64
        } else {
            0.0
        };
        let av = av_offsets(&args.root, run);
        // Picture vs sound: the median within one frame, and no point where the picture
        // lags the sound by more than two frames (AGENTS 8.3 point 6).
        let av_ok = av.as_ref().is_none_or(|a| a.median.abs() <= 1.0 && a.lag_worst >= -2.0);
        let ok = run.error.is_none() && stalls == 0 && run.stutters == 0 && shown >= 99.0 && av_ok;
        failed |= !ok;
        println!(
            "{} {:<24} ready {:>5} ms | first picture {:>4} ms | frames {:>5} | shown {:>5.1}% | longest gap {:>4} ms at {} | stutters {:>3} | stalls {} | decoder reopens {} | A/V {}{}",
            if ok { "PASS" } else { "FAIL" },
            run.name,
            run.prepare_ms.map_or("-".into(), |v| v.to_string()),
            run.first_picture_ms.map_or("-".into(), |v| v.to_string()),
            run.frames_advanced,
            shown,
            run.longest_gap_ms,
            run.longest_gap_at,
            run.stutters,
            stalls,
            reopens,
            av.as_ref().map_or("-".to_string(), |a| format!("median {:+.1} f, picture behind sound up to {:.1} f, ahead up to {:.1} f ({} points)", a.median, -a.lag_worst.min(0.0), a.lead_worst.max(0.0), a.points)),
            run.error.as_ref().map_or(String::new(), |e| format!(" | error {e}")),
        );
        report.push(serde_json::json!({
            "name": run.name, "ok": ok, "prepare_ms": run.prepare_ms,
            "first_picture_ms": run.first_picture_ms, "frames_advanced": run.frames_advanced,
            "pictures": run.pictures, "shown_percent": shown, "longest_gap_ms": run.longest_gap_ms,
            "stutters": run.stutters, "stalls": stalls, "decoder_reopens": reopens, "error": run.error,
            "av_median_frames": av.as_ref().map(|a| a.median), "av_picture_behind_frames": av.as_ref().map(|a| -a.lag_worst.min(0.0)),
            "av_picture_ahead_frames": av.as_ref().map(|a| a.lead_worst.max(0.0)),
        }));
    }
    let out = qnc_dev_diagnostics::log_dir(&args.root).join("player-acceptance.json");
    let _ = std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap_or_default());
    if failed {
        Err("player acceptance failed".into())
    } else {
        Ok(())
    }
}

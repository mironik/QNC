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
        let ok = run.error.is_none() && stalls == 0 && run.stutters == 0 && shown >= 99.0;
        failed |= !ok;
        println!(
            "{} {:<24} ready {:>5} ms | first picture {:>4} ms | frames {:>5} | shown {:>5.1}% | longest gap {:>4} ms at {} | stutters {:>3} | stalls {} | decoder reopens {}{}",
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
            run.error.as_ref().map_or(String::new(), |e| format!(" | error {e}")),
        );
        report.push(serde_json::json!({
            "name": run.name, "ok": ok, "prepare_ms": run.prepare_ms,
            "first_picture_ms": run.first_picture_ms, "frames_advanced": run.frames_advanced,
            "pictures": run.pictures, "shown_percent": shown, "longest_gap_ms": run.longest_gap_ms,
            "stutters": run.stutters, "stalls": stalls, "decoder_reopens": reopens, "error": run.error,
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

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
    steps: Vec<String>,
    seconds: u64,
}

fn args() -> Result<Args, String> {
    let mut parsed = Args { root: PathBuf::new(), clips: Vec::new(), program_from: Vec::new(), steps: Vec::new(), seconds: 10 };
    let mut list = std::env::args().skip(1);
    while let Some(arg) = list.next() {
        let value = list.next().ok_or(format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--root" => parsed.root = PathBuf::from(value),
            "--clip" => parsed.clips.push(value),
            "--program" => parsed.program_from.push(value.parse().map_err(|_| "--program FRAME")?),
            "--steps" => parsed.steps.push(value),
            "--seconds" => parsed.seconds = value.parse().map_err(|_| "--seconds N")?,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if parsed.root.as_os_str().is_empty() {
        return Err("usage: qnc-player-acceptance --root <isolated QNC root> [--clip NAME]... [--program FRAME]... [--steps NAME]... [--seconds N]".into());
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

/// What the arrow keys gave on one clip: the time from a press until the monitor shows
/// the asked frame, the time the call itself held the caller (the form's thread), and
/// how a burst of presses ended.
#[derive(Default)]
struct Steps {
    name: String,
    single_ms: Vec<u128>,
    call_us: Vec<u128>,
    missed: u64,
    burst_ms: Option<u128>,
    burst_pictures: u64,
    error: Option<String>,
}

/// The frame of the picture the monitor shows now.
fn shown_frame(preview: &SourcePreview) -> Option<u64> {
    preview.player_view().picture.as_ref().map(|picture| picture.header.frame)
}

/// Polls until the monitor shows `frame`; the time it took, or None after `timeout`.
fn wait_shown(preview: &mut SourcePreview, frame: u64, timeout: Duration) -> Option<u128> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        preview.poll();
        if shown_frame(preview) == Some(frame) {
            return Some(start.elapsed().as_millis());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    None
}

/// Ten single steps forward, ten back (each waits for its picture), then a burst of
/// ten presses 80 ms apart (a fast hand): when the last asked frame shows and how many
/// pictures the monitor showed on the way.
fn steps(preview: &mut SourcePreview, run: &mut Steps) {
    if let Err(error) = wait_ready(preview, Duration::from_secs(20)) {
        run.error = Some(error);
        return;
    }
    std::thread::sleep(Duration::from_millis(500));
    preview.poll();
    let Some(mut at) = preview.player_view().confirmed_source_frame() else {
        run.error = Some("no confirmed frame".into());
        return;
    };
    at += 30;
    preview.cue(at);
    if wait_shown(preview, at, Duration::from_secs(5)).is_none() {
        run.error = Some("the start frame did not show".into());
        return;
    }
    std::thread::sleep(Duration::from_millis(500));
    for delta in [1i64; 10].into_iter().chain([-1i64; 10]) {
        let target = at.saturating_add_signed(delta);
        let call = Instant::now();
        preview.step(delta);
        run.call_us.push(call.elapsed().as_micros());
        match wait_shown(preview, target, Duration::from_secs(3)) {
            Some(_) => run.single_ms.push(call.elapsed().as_millis()),
            None => run.missed += 1,
        }
        at = target;
        std::thread::sleep(Duration::from_millis(150));
    }
    let target = at + 10;
    let start = Instant::now();
    let mut last = shown_frame(preview);
    let mut next_press = start;
    let mut pressed = 0;
    while start.elapsed() < Duration::from_secs(5) {
        if pressed < 10 && Instant::now() >= next_press {
            let call = Instant::now();
            preview.step(1);
            run.call_us.push(call.elapsed().as_micros());
            pressed += 1;
            next_press += Duration::from_millis(80);
        }
        preview.poll();
        let shown = shown_frame(preview);
        if shown != last {
            last = shown;
            run.burst_pictures += 1;
        }
        if pressed == 10 && shown == Some(target) {
            run.burst_ms = Some(start.elapsed().as_millis());
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn percentile(values: &[u128], p: f64) -> u128 {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    sorted.get(((sorted.len() as f64 * p) as usize).min(sorted.len().saturating_sub(1))).copied().unwrap_or(0)
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
    // The player logs where the diagnostics find a QNC root: an isolated copy without
    // the root files falls back to the QNC the player was built in. Read the stalls and
    // sound timing there, or a run measured nothing and passed.
    let log_root = qnc_dev_diagnostics::locate_qnc_root().unwrap_or_else(|| args.root.clone());
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
    let mut step_runs = Vec::new();
    for wanted in &args.steps {
        let mut run = Steps { name: format!("steps {wanted}"), ..Steps::default() };
        match clips.iter().find(|clip| clip.name.contains(wanted.as_str())) {
            Some(clip) => {
                preview.open(&clip.clip_id);
                steps(&mut preview, &mut run);
            }
            None => run.error = Some("no such clip".into()),
        }
        step_runs.push(run);
    }
    preview.close();
    std::thread::sleep(Duration::from_millis(300));

    let mut failed = false;
    let mut report = Vec::new();
    for run in &runs {
        let (stalls, reopens) = logged(&log_root, run.started, run.ended + 200);
        let shown = if run.frames_advanced > 0 {
            100.0 * run.pictures as f64 / run.frames_advanced as f64
        } else {
            0.0
        };
        let av = av_offsets(&log_root, run);
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
    for run in &step_runs {
        // Every press is seen: a step shows its picture within two frame times, and a
        // burst of presses ends on the last asked frame.
        let median = percentile(&run.single_ms, 0.5);
        let ok = run.error.is_none() && run.missed == 0 && median <= 80 && run.burst_ms.is_some();
        failed |= !ok;
        println!(
            "{} {:<24} step to picture median {:>4} ms p90 {:>4} ms max {:>4} ms | missed {} | call on caller median {} us max {} us | burst of 10: {} ms, {} pictures{}",
            if ok { "PASS" } else { "FAIL" },
            run.name,
            median,
            percentile(&run.single_ms, 0.9),
            percentile(&run.single_ms, 1.0),
            run.missed,
            percentile(&run.call_us, 0.5),
            percentile(&run.call_us, 1.0),
            run.burst_ms.map_or("never".into(), |v| v.to_string()),
            run.burst_pictures,
            run.error.as_ref().map_or(String::new(), |e| format!(" | error {e}")),
        );
        report.push(serde_json::json!({
            "name": run.name, "ok": ok, "single_ms": run.single_ms, "step_median_ms": median, "step_max_ms": percentile(&run.single_ms, 1.0),
            "missed": run.missed, "call_us_median": percentile(&run.call_us, 0.5), "call_us_max": percentile(&run.call_us, 1.0),
            "burst_ms": run.burst_ms, "burst_pictures": run.burst_pictures, "error": run.error,
        }));
    }
    let out = qnc_dev_diagnostics::log_dir(&log_root).join("player-acceptance.json");
    let _ = std::fs::write(&out, serde_json::to_string_pretty(&report).unwrap_or_default());
    if failed {
        Err("player acceptance failed".into())
    } else {
        Ok(())
    }
}

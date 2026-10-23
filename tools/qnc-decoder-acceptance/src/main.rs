//! Acceptance of a decoder jobs plug-in (contract `qnc.decoder-jobs` 1): every job is done by
//! the built-in decoder and by the plug-in on the same medium, and the results must be the same
//! (pictures byte for byte, key pictures, sound sample for sample). It also checks that a
//! cancelled job ends in time and that a program speaking another contract is refused.
//!
//! The medium is only read; every result goes into a temporary folder of the system.
//!
//! ```text
//! qnc-decoder-acceptance --plugin <program path> --decoder <ffmpeg path> --media <file>
//!     --fps <num>/<den> --duration <seconds> [--container <name> --streams 1,2 --channels 2]
//! ```

use qnc_decoder_jobs::{AudioStreams, Frames, JobsClient, KeyPictures, PictureMode, Run};
use qnc_ffmpeg_decode as ffmpeg;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

struct Args {
    plugin: PathBuf,
    decoder: PathBuf,
    media: PathBuf,
    fps: (u32, u32),
    duration: f64,
    audio: Option<(String, Vec<u32>, u32)>,
}

fn args() -> Result<Args, String> {
    let mut map = BTreeMap::new();
    let mut it = std::env::args().skip(1);
    while let Some(key) = it.next() {
        let value = it.next().ok_or_else(|| format!("{key} needs a value"))?;
        map.insert(key, value);
    }
    let get = |key: &str| map.get(key).cloned().ok_or_else(|| format!("missing {key}"));
    let (num, den) = get("--fps")?.split_once('/').map(|(a, b)| (a.to_string(), b.to_string())).ok_or("--fps is num/den")?;
    let audio = match (map.get("--container"), map.get("--streams"), map.get("--channels")) {
        (Some(container), Some(streams), Some(channels)) => Some((
            container.clone(),
            streams.split(',').map(|s| s.trim().parse::<u32>().map_err(|e| e.to_string())).collect::<Result<_, _>>()?,
            channels.parse::<u32>().map_err(|e| e.to_string())?,
        )),
        (None, None, None) => None,
        _ => return Err("--container, --streams and --channels go together".into()),
    };
    Ok(Args {
        plugin: get("--plugin")?.into(),
        decoder: get("--decoder")?.into(),
        media: get("--media")?.into(),
        fps: (num.parse().map_err(|_| "--fps")?, den.parse().map_err(|_| "--fps")?),
        duration: get("--duration")?.parse().map_err(|_| "--duration is seconds")?,
        audio,
    })
}

/// The files of a folder by name and their bytes.
fn files(dir: &Path) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        out.insert(entry.file_name().to_string_lossy().into_owned(), std::fs::read(entry.path()).map_err(|e| e.to_string())?);
    }
    Ok(out)
}

fn same_pictures(name: &str, built_in: &Path, plugin: &Path) -> Result<String, String> {
    let (a, b) = (files(built_in)?, files(plugin)?);
    if a.is_empty() {
        return Err(format!("{name}: the built-in decoder made no pictures"));
    }
    if a != b {
        let names = |m: &BTreeMap<String, Vec<u8>>| m.keys().cloned().collect::<Vec<_>>().join(",");
        return Err(format!("{name}: pictures differ (built-in {}, plug-in {})", names(&a), names(&b)));
    }
    Ok(format!("{name}: {} pictures, the same bytes", a.len()))
}

fn main() {
    let args = match args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let never = AtomicBool::new(false);
    let client = JobsClient::new(args.plugin.clone());
    let mut failed = 0;
    let mut report = |result: Result<String, String>| match result {
        Ok(line) => println!("PASS {line}"),
        Err(line) => {
            failed += 1;
            println!("FAIL {line}");
        }
    };
    let temp = |name: &str| tempfile::Builder::new().prefix(&format!("qnc-accept-{name}-")).tempdir().map_err(|e| e.to_string());

    // Filmstrip pictures, each mode.
    let seconds: Vec<f64> = (0..13).map(|i| args.duration * (i as f64 + 0.5) / 13.0).collect();
    for (mode, ff_mode) in [
        (PictureMode::Keyframe, ffmpeg::FfmpegFilmstripMode::KeyframeSeek),
        (PictureMode::Random, ffmpeg::FfmpegFilmstripMode::RandomSeek),
        (PictureMode::Exact, ffmpeg::FfmpegFilmstripMode::ExactSeek),
    ] {
        report((|| {
            let (a, b) = (temp("frames-a")?, temp("frames-b")?);
            let frames: Vec<_> = seconds.iter().map(|&seek_sec| ffmpeg::FfmpegFilmstripFrame { seek_sec }).collect();
            let timebase = qnc_frame_timebase::FrameTimebase::new(i64::from(args.fps.0), i64::from(args.fps.1))?;
            let started = Instant::now();
            ffmpeg::extract_filmstrip_frames_with_cancel(&args.decoder, &args.media, &frames, timebase, ff_mode, [160, 90], a.path(), &never)?;
            let built_in = started.elapsed();
            let job = Frames { decoder: args.decoder.clone(), media: args.media.clone(), seconds: seconds.clone(), frame_rate: args.fps, mode, thumb: [160, 90], out_dir: b.path().into() };
            let started = Instant::now();
            client.frames(&job, &never)?;
            let through = started.elapsed();
            same_pictures(&format!("filmstrip {mode:?}"), a.path(), b.path())
                .map(|line| format!("{line} (built-in {} ms, plug-in {} ms)", built_in.as_millis(), through.as_millis()))
        })());
    }

    // The pictures around a trim.
    report((|| {
        let (a, b) = (temp("run-a")?, temp("run-b")?);
        let start = args.duration / 3.0;
        ffmpeg::extract_frame_run_with_cancel(&args.decoder, &args.media, start, 17, [480, 270], a.path(), &never)?;
        let job = Run { decoder: args.decoder.clone(), media: args.media.clone(), start_sec: start, count: 17, thumb: [480, 270], out_dir: b.path().into() };
        client.run(&job, &never)?;
        same_pictures("trim run", a.path(), b.path())
    })());

    // Key pictures.
    report((|| {
        let a = ffmpeg::key_pictures_with_cancel(&args.decoder, &args.media, 0.0, args.duration, &never)?;
        let job = KeyPictures { decoder: args.decoder.clone(), media: args.media.clone(), start_sec: 0.0, duration_sec: args.duration };
        let b = client.key_pictures(&job, &never)?;
        if a.is_empty() || a != b {
            return Err(format!("key pictures differ (built-in {}, plug-in {})", a.len(), b.len()));
        }
        Ok(format!("key pictures: {} times, the same", a.len()))
    })());

    // Sound for the wave, and a cancelled sound job.
    if let Some((container, streams, channels)) = &args.audio {
        report((|| {
            let mut a = Vec::new();
            let built_in = Instant::now();
            ffmpeg::audio_streams_with_cancel(&args.decoder, &args.media, container, streams, *channels, &never, &mut |s| {
                a.extend_from_slice(s);
                Ok(())
            })?;
            let built_in = built_in.elapsed();
            let job = AudioStreams { decoder: args.decoder.clone(), media: args.media.clone(), container: container.clone(), streams: streams.clone(), channels: *channels };
            let started = Instant::now();
            let mut b = Vec::new();
            client.audio_streams(&job, &never, &mut |s| {
                b.extend_from_slice(s);
                Ok(())
            })?;
            if a.is_empty() || a.len() != b.len() || a.iter().zip(&b).any(|(x, y)| x.to_bits() != y.to_bits()) {
                return Err(format!("sound differs (built-in {} samples, plug-in {})", a.len(), b.len()));
            }
            Ok(format!("sound: {} samples, the same bits (built-in {} ms, plug-in {} ms)", a.len(), built_in.as_millis(), started.elapsed().as_millis()))
        })());
        report((|| {
            let job = AudioStreams { decoder: args.decoder.clone(), media: args.media.clone(), container: container.clone(), streams: streams.clone(), channels: *channels };
            let cancel = AtomicBool::new(false);
            let started = Instant::now();
            let mut got = 0usize;
            let result = client.audio_streams(&job, &cancel, &mut |s| {
                got += s.len();
                if started.elapsed() > Duration::from_millis(150) {
                    cancel.store(true, Ordering::Relaxed);
                }
                Ok(())
            });
            let took = started.elapsed();
            match result {
                Err(error) if error == "cancelled" && took < Duration::from_secs(4) => Ok(format!("cancel: ended in {} ms after {got} samples", took.as_millis())),
                Err(error) => Err(format!("cancel: {error} after {} ms", took.as_millis())),
                Ok(()) => Err(format!("cancel: the job ran to its end ({} ms; a short medium cannot test this)", took.as_millis())),
            }
        })());
    }

    // A program speaking another contract is refused.
    report((|| {
        let spec = qnc_plugin_process::PluginSpec { executable: args.plugin.clone(), contract: "qnc.other".into(), version: 1, timeout: Duration::from_secs(10) };
        match qnc_plugin_process::Plugin::new(spec).ready() {
            Err(error) if error.contains("speaks") => Ok("another contract: refused".into()),
            other => Err(format!("another contract was not refused: {other:?}")),
        }
    })());

    println!("{}", if failed == 0 { "ACCEPTED".to_string() } else { format!("NOT ACCEPTED: {failed} failed") });
    std::process::exit(if failed == 0 { 0 } else { 1 });
}

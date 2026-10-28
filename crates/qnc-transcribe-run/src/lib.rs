//! One transcription of a range of a clip (user 2026-10-26: asked for a clip's IN/OUT, or its
//! whole length, in Media Assist Audio AI; around a cut in the trim zoom). The project says
//! whether it transcribes, which camera channel carries the speech and the language; the AI
//! catalog of this host says the engine of each purpose. The sound of that channel is read
//! from the camera original on this machine (`qnc-sound-range`), brought to the 16 kHz the
//! contract takes, written into an empty temporary folder and given to the engine's program;
//! its words come back as frames of the clip. Nothing is written but that temporary WAV; no
//! probe.
//!
//! Two engines at once (user 2026-10-26: "the zoom does not stop the background
//! transcription, it transcribes on the processor; the background one slows down or pauses
//! when the user needs the resources"; Fast or Quality beside Transcribe). The trim zoom's
//! few seconds run now on the fast engine. A background range runs in pieces of 30 s on the
//! engine it was asked with; before each piece it waits while the player plays or the user
//! edits, and a piece the user interrupts that way is stopped and done again once the player
//! is idle.

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};

pub use qnc_ai_catalog::Quality;
use qnc_playback_activity::PlaybackLevel;
use qnc_still_locator::StillLocator;
use qnc_transcribe_jobs::{Backend, Transcribe, Transcriber};
pub use qnc_transcript_db::{ClipTranscript, Line, Word};

pub const MODULE_ID: &str = "qnc.module.transcribe-run";
/// Seconds of one piece of a background range.
const PIECE_SECONDS: u64 = 30;
/// Sound quieter than this (RMS of a tenth of a second) is not given to the engine (user
/// 2026-10-26: "sound under some level is skipped, not analysed").
const SILENCE_FLOOR_DBFS: f32 = -42.0;
/// How often a waiting or running background piece looks at the player.
const LOOK_EVERY: Duration = Duration::from_millis(200);

/// What to transcribe: clip frames `[from, to)` of a clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub clip_id: String,
    pub from: u64,
    pub to: u64,
}

/// How far a background transcription is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    /// Its sound is being read.
    Reading,
    /// `seconds` of its `of` seconds of sound are transcribed.
    Heard { seconds: f64, of: f64 },
}

/// When a transcription runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// Now, whatever the player does (the trim zoom's few seconds).
    Now,
    /// In pieces, each only while the player is idle.
    YieldToPlayer,
}

/// Transcribes `ask` of the project `locator` reads with the `quality` engine at `pace`,
/// handing every line to `on_line` as it is heard. A project that does not transcribe, a
/// missing engine or model, or a clip without its sound on this machine is an error.
pub fn transcribe(locator: &StillLocator, ask: &Ask, quality: Quality, pace: Pace, cancel: &AtomicBool, on_line: &mut dyn FnMut(&Line), on_progress: &mut dyn FnMut(Progress)) -> Result<ClipTranscript, String> {
    if ask.to <= ask.from {
        return Err(qnc_ui_text::text("transcribe.empty_range").into());
    }
    let settings = locator.settings();
    if !settings.transcription_enabled().map_err(|e| e.to_string())? {
        return Err(qnc_ui_text::text("transcribe.off_in_project").into());
    }
    let engine = qnc_ai_catalog::installed_transcriber_for(quality)?;
    let place = locator.locate_sound(&ask.clip_id)?;
    let job = Job {
        locator,
        engine: &engine,
        language: settings.speech_language().map_err(|e| e.to_string())?,
        channel: u16::try_from(settings.transcribe_channel().map_err(|e| e.to_string())?)
            .map_err(|_| qnc_ui_text::text("transcribe.no_channel").to_string())?,
        place: &place,
    };
    let (lines, language, made_by) = match pace {
        Pace::Now => job.piece(ask.from, ask.to, cancel, on_line)?,
        Pace::YieldToPlayer => background(&job, ask, cancel, on_line, on_progress)?,
    };
    let model = engine.model.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(ClipTranscript {
        clip_id: ask.clip_id.clone(),
        from_frame: ask.from,
        to_frame: ask.to,
        language,
        engine: format!("{made_by} {model}"),
        lines,
    })
}

/// A background range: its sound read once from the start of the media (sound only where the
/// media allows) and, while it is read, given to the engine in pieces cut at quiet places
/// (user 2026-10-27: a clip of 3:24 waited about 35 s for its whole sound before the first
/// word). Reading and hearing each run only while the player is idle. A piece the user stops
/// by taking the player is done again alone once idle; the pieces done before it stay (user
/// 2026-10-26: "an interruption must not throw the work away"). A program that ended in a
/// piece is started again for it once.
fn background(job: &Job, ask: &Ask, cancel: &AtomicBool, on_line: &mut dyn FnMut(&Line), on_progress: &mut dyn FnMut(Progress)) -> Result<(Vec<Line>, String, String), String> {
    on_progress(Progress::Reading);
    let busy = player_busy(job.locator);
    let (_, _, _, rate) = job.place.channel(job.channel).ok_or_else(|| qnc_ui_text::text("transcribe.no_channel").to_string())?;
    let total = ask.to.saturating_sub(ask.from) as f64 * f64::from(job.place.fps.1) / f64::from(job.place.fps.0.max(1));
    let read = Mutex::new(Read::default());
    let quit = AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| read_sound(job, ask, &*busy, &quit, &read));
        // The reader stops with this run, however it ends.
        let _stop_reading = Quit(&quit);
        hear_while_read(job, ask, (&*busy, cancel), (&read, rate as usize, total), on_line, on_progress)
    })
}

/// The sound read so far and how the read ended.
#[derive(Default)]
struct Read {
    samples: Vec<f32>,
    /// The media seconds of the first sample.
    start: Option<f64>,
    done: bool,
    error: Option<String>,
}

struct Quit<'a>(&'a AtomicBool);

impl Drop for Quit<'_> {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

fn lock(read: &Mutex<Read>) -> std::sync::MutexGuard<'_, Read> {
    read.lock().unwrap_or_else(|e| e.into_inner())
}

/// Reads the range's sound into `read` while the player is idle; a read the player stops goes
/// on from the frames it already has.
fn read_sound(job: &Job, ask: &Ask, busy: &(dyn Fn() -> PlaybackLevel + Send + Sync), quit: &AtomicBool, read: &Mutex<Read>) {
    let mut frames = 0u64;
    let outcome = loop {
        if wait_idle(busy, quit).is_err() {
            break Ok(());
        }
        let (made, stopped) = while_idle(busy, quit, |stop| {
            qnc_sound_range::stream_from_start(job.place, job.channel, ask.from + frames, ask.to, stop, &mut |samples, covered, at| {
                let mut read = lock(read);
                read.start.get_or_insert(at);
                read.samples.extend_from_slice(samples);
                frames += covered;
            })
        });
        match made {
            Ok(true) => break Ok(()),
            Ok(false) | Err(_) if stopped => log(&format!("transcribe background read {} yields to the player", ask.clip_id)),
            Ok(false) => break Ok(()),
            Err(error) => break Err(error),
        }
    };
    let mut read = lock(read);
    read.done = true;
    read.error = outcome.err();
}

/// Gives the sound to the engine as it is read: a piece once 35 s of sound beyond it are in
/// (it is cut at the quietest place of its last seconds), the rest once the read is done. Only
/// the sound over the floor goes to the engine.
fn hear_while_read(
    job: &Job,
    ask: &Ask,
    (busy, cancel): (&(dyn Fn() -> PlaybackLevel + Send + Sync), &AtomicBool),
    (read, rate, total): (&Mutex<Read>, usize, f64),
    on_line: &mut dyn FnMut(&Line),
    on_progress: &mut dyn FnMut(Progress),
) -> Result<(Vec<Line>, String, String), String> {
    let window = (PIECE_SECONDS as usize + 5) * rate;
    let (mut processed, mut heard_seconds) = (0usize, 0.0f64);
    // Where the time goes, for the log (live 2026-10-27: "the speed is broken").
    let (started, mut engine_time, mut waited) = (std::time::Instant::now(), Duration::ZERO, Duration::ZERO);
    let mut read_took = None;
    let (mut lines, mut language, mut made_by) = (Vec::new(), String::new(), String::new());
    loop {
        let next = {
            let read = lock(read);
            if let Some(error) = &read.error {
                return Err(error.clone());
            }
            let available = read.samples.len() - processed;
            if read.done && read_took.is_none() {
                read_took = Some(started.elapsed());
            }
            match read.start {
                Some(start) if read.done || available >= window => {
                    let end = if read.done { read.samples.len() } else { processed + window };
                    Some((read.samples[processed..end].to_vec(), start, read.done))
                }
                _ if read.done => break,
                _ => None,
            }
        };
        let Some((block, start, last)) = next else {
            if cancel.load(Ordering::Relaxed) {
                return Err(qnc_ui_text::text("transcribe.cancelled").into());
            }
            std::thread::sleep(Duration::from_millis(100));
            waited += Duration::from_millis(100);
            continue;
        };
        let block = match last {
            true => &block[..],
            false => &block[..qnc_transcribe_jobs::quiet_pieces(&block, rate, PIECE_SECONDS as usize)[0].end],
        };
        let block_start = start + processed as f64 / rate as f64;
        for span in qnc_transcribe_jobs::loud_spans(block, rate, SILENCE_FLOOR_DBFS) {
            for piece in qnc_transcribe_jobs::quiet_pieces(&block[span.clone()], rate, PIECE_SECONDS as usize) {
                let range = span.start + piece.start..span.start + piece.end;
                let samples = qnc_transcribe_jobs::to_whisper_rate(&block[range.clone()], rate as u32)?;
                let piece_start = block_start + range.start as f64 / rate as f64;
                let heard_at = std::time::Instant::now();
                let (heard, piece_language, by) = hear(job, ask, (busy, cancel), &samples, piece_start)?;
                engine_time += heard_at.elapsed();
                for line in &heard {
                    on_line(line);
                }
                lines.extend(heard);
                if language.is_empty() {
                    language = piece_language;
                }
                made_by = by;
                heard_seconds += range.len() as f64 / rate as f64;
            }
        }
        processed += block.len();
        on_progress(Progress::Heard { seconds: (processed as f64 / rate as f64).min(total), of: total });
        if last {
            break;
        }
    }
    log(&format!(
        "transcribe background {} {:.1}s of {:.1}s over {SILENCE_FLOOR_DBFS} dBFS; read {:.1}s, engine {:.1}s, waited for the read {:.1}s, all {:.1}s",
        ask.clip_id,
        heard_seconds,
        total,
        read_took.unwrap_or_else(|| started.elapsed()).as_secs_f64(),
        engine_time.as_secs_f64(),
        waited.as_secs_f64(),
        started.elapsed().as_secs_f64()
    ));
    // A clip with no sound over the floor gave the engine nothing: no words, in the language
    // the project asked for (live 2026-10-27: seven such clips failed for an empty language).
    if language.is_empty() {
        language = job.language.clone();
    }
    Ok((lines, language, made_by))
}

/// One piece (16 kHz, from `start` media seconds) through the engine while the player is idle.
fn hear(job: &Job, ask: &Ask, (busy, cancel): (&(dyn Fn() -> PlaybackLevel + Send + Sync), &AtomicBool), samples: &[f32], start: f64) -> Result<(Vec<Line>, String, String), String> {
    let mut ended_once = false;
    loop {
        wait_idle(busy, cancel)?;
        let (made, stopped) = while_idle(busy, cancel, |stop| job.transcribe(samples, start, (ask.from, ask.to), stop, &mut |_| {}));
        if cancel.load(Ordering::Relaxed) {
            return Err(qnc_ui_text::text("transcribe.cancelled").into());
        }
        match made {
            Ok(made) => return Ok(made),
            Err(_) if stopped => log(&format!("transcribe background {} {:.1}s yields to the player", ask.clip_id, start)),
            Err(error) if !ended_once => {
                log(&format!("transcribe background {} {:.1}s: {error}; once again", ask.clip_id, start));
                forget_transcriber(&job.engine.program);
                ended_once = true;
            }
            Err(error) => return Err(error),
        }
    }
}

/// Runs `work` with a stop flag set when the job is cancelled or the user takes the player;
/// its result and whether the player (playing) stopped it.
fn while_idle<T>(busy: &(dyn Fn() -> PlaybackLevel + Send + Sync), cancel: &AtomicBool, work: impl FnOnce(&AtomicBool) -> T) -> (T, bool) {
    let (stop, done, by_player) = (AtomicBool::new(false), AtomicBool::new(false), AtomicBool::new(false));
    let made = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    stop.store(true, Ordering::Relaxed);
                    return;
                }
                if busy() == PlaybackLevel::Playing {
                    by_player.store(true, Ordering::Relaxed);
                    stop.store(true, Ordering::Relaxed);
                    return;
                }
                std::thread::sleep(LOOK_EVERY);
            }
        });
        let made = work(&stop);
        done.store(true, Ordering::Relaxed);
        made
    });
    (made, by_player.load(Ordering::Relaxed))
}

/// Waits while the player plays; while the user edits the transcription runs below the
/// player's priority (user 2026-10-26: waiting for every touch made it not worth it).
fn wait_idle(busy: &(dyn Fn() -> PlaybackLevel + Send + Sync), cancel: &AtomicBool) -> Result<(), String> {
    while busy() == PlaybackLevel::Playing {
        if cancel.load(Ordering::Relaxed) {
            return Err(qnc_ui_text::text("transcribe.cancelled").into());
        }
        std::thread::sleep(LOOK_EVERY);
    }
    Ok(())
}

/// The player's work in this project, as the previews report it to the project database.
fn player_busy(locator: &StillLocator) -> Arc<dyn Fn() -> PlaybackLevel + Send + Sync> {
    match qnc_content_store::ContentTarget::for_project(locator.reader(), locator.settings()) {
        Ok(target) => qnc_playback_activity::playback_level(target),
        Err(_) => Arc::new(|| PlaybackLevel::Idle),
    }
}

struct Job<'a> {
    locator: &'a StillLocator,
    engine: &'a qnc_ai_catalog::Transcriber,
    language: String,
    channel: u16,
    place: &'a qnc_still_locator::LocatedSound,
}

impl Job<'_> {
    /// Clip frames `[from, to)` now: read where they are and transcribed.
    fn piece(&self, from: u64, to: u64, cancel: &AtomicBool, on_line: &mut dyn FnMut(&Line)) -> Result<(Vec<Line>, String, String), String> {
        let sound = qnc_sound_range::read(self.place, self.channel, from, to, &mut |_| !cancel.load(Ordering::Relaxed))?;
        if cancel.load(Ordering::Relaxed) {
            return Err(qnc_ui_text::text("transcribe.cancelled").into());
        }
        let samples = qnc_transcribe_jobs::to_whisper_rate(&sound.samples, sound.rate)?;
        self.transcribe(&samples, sound.from_seconds, (from, to), cancel, on_line)
    }

    /// `samples` (16 kHz, from `start` seconds of the media) through the engine: its lines as
    /// clip frames within `range`, the language and the program.
    fn transcribe(&self, samples: &[f32], start: f64, range: (u64, u64), cancel: &AtomicBool, on_line: &mut dyn FnMut(&Line)) -> Result<(Vec<Line>, String, String), String> {
        let place = self.place;
        let folder = std::env::temp_dir().join(format!("qnc-transcribe-{}-{}", std::process::id(), unique()));
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let wav = folder.join("sound.wav");
        let result = qnc_transcribe_jobs::write_wav(&wav, samples).and_then(|()| {
            let job = Transcribe {
                model: self.engine.model.clone(),
                vad_model: self.engine.vad_model.clone(),
                backend: if self.engine.backend == "vulkan" { Backend::Vulkan } else { Backend::Cpu },
                wav,
                language: self.language.clone(),
                threads: self.engine.threads,
            };
            // Seconds of the WAV as clip frames: the WAV starts at `start`.
            let fps = f64::from(place.fps.0) / f64::from(place.fps.1.max(1));
            let frame = |seconds: f64| {
                let at = ((start + seconds - place.frame_zero_seconds) * fps).round().max(0.0) as u64;
                at.clamp(range.0, range.1)
            };
            let to_line = |segment: &qnc_transcribe_jobs::Segment| Line {
                from: frame(segment.start),
                to: frame(segment.end),
                text: segment.text.clone(),
                words: segment
                    .words
                    .iter()
                    .map(|word| Word { from: frame(word.start), to: frame(word.end).max(frame(word.start)), text: word.text.clone() })
                    .collect(),
            };
            let transcriber = transcriber(&self.engine.program)?;
            let transcript = transcriber.transcribe(&job, cancel, &mut |segment| on_line(&to_line(segment)))?;
            Ok((transcript.segments.iter().map(to_line).collect(), transcript.language, transcript.engine))
        });
        let _ = std::fs::remove_dir_all(&folder);
        result
    }
}

fn kept() -> std::sync::MutexGuard<'static, HashMap<String, Arc<Transcriber>>> {
    static KEPT: OnceLock<Mutex<HashMap<String, Arc<Transcriber>>>> = OnceLock::new();
    KEPT.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// A program that ended is started again for the next job.
fn forget_transcriber(program: &str) {
    kept().remove(program);
}

/// The transcription program named `program` in this process, kept between jobs so its model
/// stays loaded; each program its own, so the zoom's runs beside the background one.
fn transcriber(program: &str) -> Result<Arc<Transcriber>, String> {
    let mut kept = kept();
    if let Some(found) = kept.get(program) {
        return Ok(found.clone());
    }
    let made = Arc::new(Transcriber::beside_application(program)?);
    kept.insert(program.to_string(), made.clone());
    Ok(made)
}

fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}

fn unique() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()
}

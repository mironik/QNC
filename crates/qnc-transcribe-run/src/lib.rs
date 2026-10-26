//! One transcription of a range of a clip (user 2026-10-26: asked for a clip's IN/OUT, or its
//! whole length, in Media Assist Audio AI; around a cut in the trim zoom). The project says
//! whether it transcribes, which camera channel carries the speech and the language; the AI
//! catalog of this host says the engine. The sound of that channel is read from the camera
//! original on this machine (`qnc-sound-range`), brought to the 16 kHz the contract takes,
//! written into an empty temporary folder and given to the engine's program; its words come
//! back as frames of the clip. Nothing is written but that temporary WAV; no probe.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
};

use qnc_still_locator::StillLocator;
use qnc_transcribe_jobs::{Backend, Transcribe, Transcriber};
pub use qnc_transcript_db::{ClipTranscript, Line, Word};

pub const MODULE_ID: &str = "qnc.module.transcribe-run";

/// What to transcribe: clip frames `[from, to)` of a clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ask {
    pub clip_id: String,
    pub from: u64,
    pub to: u64,
}

/// Transcribes `ask` of the project `locator` reads, handing every line to `on_line` as it
/// is heard. A project that does not transcribe, a missing engine or model, or a clip without
/// its sound on this machine is an error.
pub fn transcribe(locator: &StillLocator, ask: &Ask, cancel: &AtomicBool, on_line: &mut dyn FnMut(&Line)) -> Result<ClipTranscript, String> {
    if ask.to <= ask.from {
        return Err(qnc_ui_text::text("transcribe.empty_range").into());
    }
    let settings = locator.settings();
    if !settings.transcription_enabled().map_err(|e| e.to_string())? {
        return Err(qnc_ui_text::text("transcribe.off_in_project").into());
    }
    let language = settings.speech_language().map_err(|e| e.to_string())?;
    let channel = settings.transcribe_channel().map_err(|e| e.to_string())?;
    let engine = qnc_ai_catalog::installed_transcriber()?;
    let place = locator.locate_sound(&ask.clip_id)?;
    let channel = u16::try_from(channel).map_err(|_| qnc_ui_text::text("transcribe.no_channel").to_string())?;
    let sound = qnc_sound_range::read(&place, channel, ask.from, ask.to, &mut |_| !cancel.load(Ordering::Relaxed))?;
    if cancel.load(Ordering::Relaxed) {
        return Err(qnc_ui_text::text("transcribe.cancelled").into());
    }
    let samples = qnc_transcribe_jobs::to_whisper_rate(&sound.samples, sound.rate)?;
    let folder = std::env::temp_dir().join(format!("qnc-transcribe-{}-{}", std::process::id(), unique()));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let wav = folder.join("sound.wav");
    let result = qnc_transcribe_jobs::write_wav(&wav, &samples).and_then(|()| {
        let job = Transcribe {
            model: engine.model.clone(),
            vad_model: engine.vad_model.clone(),
            backend: if engine.backend == "vulkan" { Backend::Vulkan } else { Backend::Cpu },
            wav,
            language,
            threads: engine.threads,
        };
        // Seconds of the WAV as clip frames: the WAV starts where the read sound starts.
        let fps = f64::from(place.fps.0) / f64::from(place.fps.1.max(1));
        let frame = |seconds: f64| {
            let at = ((sound.from_seconds + seconds - place.frame_zero_seconds) * fps).round().max(0.0) as u64;
            at.clamp(ask.from, ask.to)
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
        let transcript = with_transcriber(&engine.program, |transcriber| {
            transcriber.transcribe(&job, cancel, &mut |segment| on_line(&to_line(segment)))
        })?;
        let model = engine.model.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Ok(ClipTranscript {
            clip_id: ask.clip_id.clone(),
            from_frame: ask.from,
            to_frame: ask.to,
            language: transcript.language,
            engine: format!("{} {model}", transcript.engine),
            lines: transcript.segments.iter().map(to_line).collect(),
        })
    });
    let _ = std::fs::remove_dir_all(&folder);
    result
}

/// The one transcription program of this process, kept between jobs: its model stays loaded.
fn with_transcriber<T>(program: &str, run: impl FnOnce(&Transcriber) -> Result<T, String>) -> Result<T, String> {
    static KEPT: OnceLock<Mutex<Option<(String, Transcriber)>>> = OnceLock::new();
    let mut kept = KEPT.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if kept.as_ref().is_none_or(|(name, _)| name != program) {
        *kept = Some((program.to_string(), Transcriber::beside_application(program)?));
    }
    run(&kept.as_ref().expect("kept above").1)
}

fn unique() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default()
}

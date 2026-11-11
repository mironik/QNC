//! Speech to text as a plug-in program (contract `qnc.transcribe` 2) with the Croatian wav2vec2
//! CTC model (ParlaSpeech-HR) in pure Rust, on the processor, below the player and the desktop
//! (user 2026-11-07: "a second pass of a chosen part with wav2vec2", Parakeet heard "jedna
//! tisuća devetsto šezdeset devet" where the speaker said "tisuću devetsto šezdeset devete").
//! It writes lower case with no punctuation, numbers as the Croatian norm writes them (1969.).
//! The WAV goes through the model in windows of at most 30 s cut at a quiet place; a segment
//! ends at a pause of more than a second. The model is loaded once and kept for the next job.

use qnc_transcribe_jobs::{Backend, Engine, Segment, Transcribe, Word, WHISPER_RATE};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

const WINDOW_SECONDS: usize = 30;
const PAUSE_SECONDS: f64 = 1.0;

#[derive(Default)]
struct Wav2Vec2Engine {
    kept: Option<(PathBuf, qnc_wav2vec2::Wav2Vec2)>,
}

impl Engine for Wav2Vec2Engine {
    fn backends(&self) -> Vec<Backend> {
        vec![Backend::Cpu]
    }

    fn transcribe(&mut self, job: &Transcribe, cancel: &AtomicBool, on_segment: &mut dyn FnMut(Segment) -> Result<(), String>) -> Result<String, String> {
        if job.backend != Backend::Cpu {
            return Err("this program runs wav2vec2 on the processor only".into());
        }
        if self.kept.as_ref().is_none_or(|(model, _)| *model != job.model) {
            self.kept = None;
            self.kept = Some((job.model.clone(), qnc_wav2vec2::Wav2Vec2::load(&job.model)?));
        }
        let model = &self.kept.as_ref().expect("kept above").1;
        let samples = &job.samples;
        let rate = WHISPER_RATE as usize;
        for window in qnc_transcribe_jobs::quiet_pieces(&samples, rate, WINDOW_SECONDS) {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let offset = window.start as f64 / rate as f64;
            let emissions = model.emissions(&samples[window])?;
            let words: Vec<Word> = model
                .greedy_words(&emissions)
                .into_iter()
                .map(|word| Word {
                    start: offset + word.first as f64 * qnc_wav2vec2::FRAME_SECONDS,
                    end: offset + (word.last + 1) as f64 * qnc_wav2vec2::FRAME_SECONDS,
                    p: word.p() as f32,
                    text: word.text,
                })
                .collect();
            for segment in segments(words) {
                on_segment(segment)?;
            }
        }
        Ok(job.language.clone())
    }
}

/// The words in segments: one ends before a pause of more than a second; its first word starts
/// with a capital (the model writes lower case).
fn segments(words: Vec<Word>) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current: Vec<Word> = Vec::new();
    let count = words.len();
    for (index, word) in words.iter().enumerate() {
        current.push(word.clone());
        let pause = words.get(index + 1).is_some_and(|next| next.start - word.end > PAUSE_SECONDS);
        if pause || index + 1 == count {
            if let Some(first) = current.first_mut() {
                let mut letters = first.text.chars();
                first.text = letters.next().map(|letter| letter.to_uppercase().chain(letters).collect()).unwrap_or_default();
            }
            let text = current.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ");
            segments.push(Segment { start: current[0].start, end: current[current.len() - 1].end, text, words: std::mem::take(&mut current) });
        }
    }
    segments
}

fn main() {
    qnc_dev_diagnostics::log_panics("qnc-transcribe-wav2vec2");
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::Threading::{GetCurrentProcess, SetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS};
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
    let mut engine = Wav2Vec2Engine::default();
    if let Err(error) = qnc_transcribe_jobs::serve(&mut engine, "qnc.transcribe.wav2vec2", env!("CARGO_PKG_VERSION"), "wav2vec2 ctc parlaspeech-hr (candle)") {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, start: f64) -> Word {
        Word { start, end: start + 0.3, text: text.into(), p: 1.0 }
    }

    #[test]
    fn words_become_segments_at_pauses_with_a_capital() {
        let lines: Vec<String> = segments(vec![word("kad", 0.0), word("je", 0.4), word("1969.", 0.8), word("u", 3.0)]).into_iter().map(|s| s.text).collect();
        assert_eq!(lines, ["Kad je 1969.", "U"], "the ordinal's full stop does not end a segment, the pause does");
    }
}

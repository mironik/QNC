//! whisper.cpp behind the transcription contract.

use qnc_transcribe_jobs::{read_wav, Backend, Engine, Segment, Transcribe, Word, WHISPER_RATE};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};
use whisper_rs::{
    DtwMode, DtwModelPreset, DtwParameters, FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    WhisperVadParams,
};

/// The longest piece of sound given to the engine in one go: every piece's segments are sent
/// when it is done, so the application hears from the program well within its timeout.
const PIECE_SECONDS: usize = 240;
/// A piece ends at the quietest tenth of a second of its last seconds, not inside a word.
const CUT_SEARCH_SECONDS: usize = 10;

#[derive(Default)]
pub struct Whisper {
    /// The loaded model, kept while the next job asks for the same model and backend.
    loaded: Option<(PathBuf, Backend, WhisperContext)>,
}

impl Engine for Whisper {
    fn backends(&self) -> Vec<Backend> {
        super::backends()
    }

    fn transcribe(
        &mut self,
        job: &Transcribe,
        cancel: &AtomicBool,
        on_segment: &mut dyn FnMut(Segment) -> Result<(), String>,
    ) -> Result<String, String> {
        if !self.backends().contains(&job.backend) {
            return Err(format!("this build does not run the {:?} backend", job.backend));
        }
        let samples = read_wav(&job.wav)?;
        let context = self.context(job)?;
        let mut state = context.create_state().map_err(|e| format!("whisper state: {e}"))?;
        let mut language = job.language.clone();
        let mut previous = String::new();
        for (from, to) in pieces(&samples) {
            if cancel.load(Ordering::Relaxed) {
                return Err("cancelled".into());
            }
            let params = params(job, &language, cancel)?;
            state.full(params, &samples[from..to]).map_err(|e| {
                if cancel.load(Ordering::Relaxed) {
                    "cancelled".to_string()
                } else {
                    format!("whisper: {e}")
                }
            })?;
            if language == "auto" {
                // The first piece decides the language; the next pieces keep it.
                if let Some(found) = whisper_rs::get_lang_str(state.full_lang_id_from_state()) {
                    language = found.to_string();
                }
            }
            let offset = from as f64 / f64::from(WHISPER_RATE);
            for index in 0..state.full_n_segments() {
                let Some(segment) = state.get_segment(index) else { continue };
                if let Some(segment) = to_segment(context, &segment, offset) {
                    // A segment that repeats the one before is the engine looping on ambience
                    // (measured 2026-10-26: one sentence 60 times over a tennis match; v5 drops
                    // them the same way): the first is kept.
                    let said = normalized(&segment.text);
                    if said.is_empty() || said == previous {
                        continue;
                    }
                    previous = said;
                    on_segment(segment)?;
                }
            }
        }
        Ok(language)
    }
}

impl Whisper {
    fn context(&mut self, job: &Transcribe) -> Result<&WhisperContext, String> {
        let same = self
            .loaded
            .as_ref()
            .is_some_and(|(model, backend, _)| *model == job.model && *backend == job.backend);
        if !same {
            self.loaded = None;
            let mut parameters = WhisperContextParameters::default();
            parameters.use_gpu(job.backend == Backend::Vulkan);
            // Word times from the alignment heads of this model (DTW); it needs plain attention.
            parameters.flash_attn(false);
            parameters.dtw_parameters(DtwParameters {
                mode: DtwMode::ModelPreset { model_preset: DtwModelPreset::LargeV3Turbo },
                dtw_mem_size: 128 << 20,
            });
            let context = WhisperContext::new_with_params(&job.model, parameters)
                .map_err(|e| format!("whisper model {}: {e}", job.model.display()))?;
            self.loaded = Some((job.model.clone(), job.backend, context));
        }
        Ok(&self.loaded.as_ref().expect("loaded above").2)
    }
}

fn params<'a>(job: &'a Transcribe, language: &'a str, cancel: &AtomicBool) -> Result<FullParams<'a, 'a>, String> {
    let mut params = FullParams::new(SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 });
    params.set_n_threads(job.threads.max(1) as i32);
    params.set_language(Some(language));
    params.set_translate(false);
    // Each window on its own: a mistake is not carried into the next one (no repeating loops).
    params.set_no_context(true);
    params.set_suppress_nst(true);
    params.set_suppress_blank(true);
    // whisper.cpp's own retries at a higher temperature when a window decodes badly: without
    // them a speech window can come out as "Hvala." (measured 2026-10-26 on a news clip); the
    // voice activity model keeps silence and ambience away instead.
    params.set_temperature(0.0);
    params.set_temperature_inc(0.2);
    params.set_logprob_thold(-1.0);
    params.set_entropy_thold(2.4);
    params.set_no_speech_thold(0.6);
    params.set_token_timestamps(true);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    if let Some(vad) = &job.vad_model {
        let vad = vad.to_str().ok_or("the voice activity model path is not text")?;
        params.set_vad_model_path(Some(vad));
        params.enable_vad(true);
        params.set_vad_params(WhisperVadParams::new());
    }
    // The raw callback with the flag itself as its data: whisper-rs 0.16's `set_abort_callback_safe`
    // reads its closure as the wrong type and aborts every run ("failed to encode").
    // SAFETY: the flag is the job's and outlives the engine's run that calls this; the callback
    // only reads it.
    unsafe {
        params.set_abort_callback(Some(abort_when_cancelled));
        params.set_abort_callback_user_data(cancel as *const AtomicBool as *mut std::ffi::c_void);
    }
    Ok(params)
}

unsafe extern "C" fn abort_when_cancelled(flag: *mut std::ffi::c_void) -> bool {
    // SAFETY: `flag` is the job's `AtomicBool` set in `params`.
    !flag.is_null() && unsafe { &*(flag as *const AtomicBool) }.load(Ordering::Relaxed)
}

/// The pieces of the sound, each at most [`PIECE_SECONDS`], cut at a quiet point.
fn pieces(samples: &[f32]) -> Vec<(usize, usize)> {
    let rate = WHISPER_RATE as usize;
    let longest = PIECE_SECONDS * rate;
    let mut pieces = Vec::new();
    let mut from = 0;
    while samples.len() - from > longest {
        let search_from = from + longest - CUT_SEARCH_SECONDS * rate;
        let window = rate / 10;
        let quietest = (search_from..from + longest - window)
            .step_by(window)
            .min_by(|a, b| {
                let energy = |at: usize| samples[at..at + window].iter().map(|s| s * s).sum::<f32>();
                energy(*a).total_cmp(&energy(*b))
            })
            .map(|at| at + window / 2)
            .unwrap_or(from + longest);
        pieces.push((from, quietest));
        from = quietest;
    }
    if from < samples.len() {
        pieces.push((from, samples.len()));
    }
    pieces
}

/// One segment with its words: a word starts at a token that begins with a space; its time is
/// the token's aligned time (DTW), else its timestamp; it ends where the next word starts.
fn to_segment(context: &WhisperContext, segment: &whisper_rs::WhisperSegment, offset: f64) -> Option<Segment> {
    let centi = |t: i64| offset + t as f64 / 100.0;
    let (start, end) = (centi(segment.start_timestamp()), centi(segment.end_timestamp()));
    let text = segment.to_str_lossy().ok()?.trim().to_string();
    if text.is_empty() {
        return None;
    }
    let eot = context.token_eot();
    let mut words: Vec<(f64, Vec<u8>, f32, usize)> = Vec::new();
    for index in 0..segment.n_tokens() {
        let Some(token) = segment.get_token(index) else { continue };
        if token.token_id() >= eot {
            continue; // special and timestamp tokens
        }
        let data = token.token_data();
        let Ok(bytes) = token.to_bytes() else { continue };
        let at = if data.t_dtw >= 0 { centi(data.t_dtw) } else { centi(data.t0) };
        if bytes.first() == Some(&b' ') || words.is_empty() {
            words.push((at, bytes.to_vec(), data.p, 1));
        } else if let Some(word) = words.last_mut() {
            word.1.extend_from_slice(bytes);
            word.2 += data.p;
            word.3 += 1;
        }
    }
    let starts: Vec<f64> = words.iter().map(|w| w.0.clamp(start, end)).collect();
    let words = words
        .into_iter()
        .enumerate()
        .filter_map(|(i, (_, bytes, p, n))| {
            let text = String::from_utf8_lossy(&bytes).trim().to_string();
            if text.is_empty() {
                return None;
            }
            let word_start = starts[i];
            let word_end = starts.get(i + 1).copied().unwrap_or(end).max(word_start);
            Some(Word { start: word_start, end: word_end, text, p: p / n as f32 })
        })
        .collect();
    Some(Segment { start, end, text, words })
}

/// The words of a text, lowercase, without punctuation: two segments saying the same compare equal.
fn normalized(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_same_words_compare_equal_whatever_their_case_and_punctuation() {
        assert_eq!(super::normalized("A ne Srbija, se vidite."), super::normalized("a ne srbija se vidite"));
        assert_ne!(super::normalized("Hvala."), super::normalized("Hvala vam."));
        assert_eq!(super::normalized("Čilić!"), "čilić");
    }
}

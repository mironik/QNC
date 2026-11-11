//! The fine times of the letters of the words a script shows (user 2026-11-05: "a finer
//! timecode for cutting by letters, made automatically in the background, only for the text
//! shown in the script"). The words of a clip that have no letters in the project database are
//! laid on their speech by the letter aligner of this host (`qnc.letter-align`), a run of
//! words at a time, off the form thread and only while the player is idle; the letters are
//! kept in the project database (`qnc-letter-db`) and read again from there. The database is
//! the truth; what is held here is only the job and the words that failed in this run.

use std::{
    collections::HashSet,
    sync::{mpsc, Arc},
};

use qnc_letter_db::{Letter, WordLetters};
use qnc_still_locator::StillLocator;

pub const MODULE_ID: &str = "qnc.module.letter-times";

/// The longest run of words laid on its speech at once, in seconds.
const RUN_SECONDS: f64 = 20.0;
/// Speech read on each side of a run, in seconds (a word's first sound is often early).
const MARGIN_SECONDS: f64 = 0.3;

/// A word of a clip without letters: its clip frames and its text, as the transcript has it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Word {
    pub from: u64,
    pub to: u64,
    pub text: String,
}

/// The words of one clip that need their letters, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Need {
    pub clip_id: String,
    /// The clip's frames a second (how long a run of its words is).
    pub fps: f64,
    pub words: Vec<Word>,
}

impl Need {
    /// The few words of a short window (the trim zoom's seconds): laid as one run, whatever
    /// the clip's rate.
    pub fn few(clip_id: String, words: Vec<Word>) -> Self {
        Self { clip_id, fps: f64::INFINITY, words }
    }
}

struct Job {
    clip_id: String,
    words: Vec<Word>,
    done: mpsc::Receiver<Result<(), String>>,
}

#[derive(Default)]
pub struct LetterTimes {
    project_id: Option<String>,
    aligner: Option<Arc<qnc_letter_align::Aligner>>,
    job: Option<Job>,
    /// Words laying failed for in this run: not asked again (they would be asked every poll).
    failed: HashSet<(String, Word)>,
}

impl LetterTimes {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether a run of words is being laid on its speech.
    pub fn working(&self) -> bool {
        self.job.is_some()
    }

    /// Lays the next run of words of `needs` while the player is `idle`; true when a run was
    /// kept (the caller reads the letters again).
    pub fn poll(&mut self, locator: &StillLocator, needs: &[Need], idle: bool) -> bool {
        if self.project_id.as_deref() != Some(locator.project_id()) {
            *self = Self { project_id: Some(locator.project_id().to_string()), ..Self::default() };
        }
        let kept = self.take_finished();
        if self.job.is_some() || !idle || !locator.settings().ai_feature(qnc_ai_features::LETTER_TIMES).unwrap_or(false) {
            return kept;
        }
        let next = needs.iter().find_map(|need| {
            let words: Vec<Word> = need.words.iter().filter(|word| !self.failed.contains(&(need.clip_id.clone(), (*word).clone()))).cloned().collect();
            let first = words.first()?.from;
            let run: Vec<Word> = words.into_iter().take_while(|word| (word.to.saturating_sub(first) as f64) / need.fps.max(1.0) <= RUN_SECONDS).collect();
            (!run.is_empty()).then(|| (need.clip_id.clone(), run))
        });
        if let Some((clip_id, words)) = next {
            self.start(locator, clip_id, words);
        }
        kept
    }

    fn take_finished(&mut self) -> bool {
        let Some(job) = &self.job else { return false };
        match job.done.try_recv() {
            Ok(Ok(())) => {
                self.job = None;
                true
            }
            Ok(Err(error)) => {
                log(&format!("letter-times {} {} words: {error}", job.clip_id, job.words.len()));
                for word in &job.words {
                    self.failed.insert((job.clip_id.clone(), word.clone()));
                }
                self.job = None;
                false
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.job = None;
                false
            }
            Err(mpsc::TryRecvError::Empty) => false,
        }
    }

    fn start(&mut self, locator: &StillLocator, clip_id: String, words: Vec<Word>) {
        let aligner = match (&self.aligner, qnc_ai_catalog::installed_letter_aligner()) {
            (Some(aligner), _) => Ok(aligner.clone()),
            (None, Ok(engine)) => qnc_plugin_process::beside_application(&engine.program).map(|program| Arc::new(qnc_letter_align::Aligner::new(&program))),
            (None, Err(error)) => Err(error),
        };
        let (send, done) = mpsc::channel();
        match aligner {
            Ok(aligner) => {
                self.aligner = Some(aligner.clone());
                let (locator, clip, run) = (locator.clone(), clip_id.clone(), words.clone());
                let spawned = std::thread::Builder::new().name("qnc-letter-times".into()).spawn(move || _ = send.send(lay(&locator, &aligner, &clip, &run)));
                if spawned.is_err() {
                    return;
                }
            }
            Err(error) => _ = send.send(Err(error)),
        }
        self.job = Some(Job { clip_id, words, done });
    }
}

/// The letters of `words` of `clip_id`: their speech read where the clip's sound is on this
/// computer (the project's speech channel), laid on by the aligner and kept.
fn lay(locator: &StillLocator, aligner: &qnc_letter_align::Aligner, clip_id: &str, words: &[Word]) -> Result<(), String> {
    let place = locator.locate_sound(clip_id)?;
    let channel = u16::try_from(locator.settings().transcribe_channel().map_err(|e| e.to_string())?).map_err(|_| "no speech channel".to_string())?;
    let fps = f64::from(place.fps.0) / f64::from(place.fps.1.max(1));
    let margin = (MARGIN_SECONDS * fps).round() as u64;
    let (first, last) = (words.first().ok_or("no words")?.from, words.iter().map(|word| word.to).max().unwrap_or(0));
    let sound = qnc_sound_range::read(&place, channel, first.saturating_sub(margin), last + margin, &mut |_| true)?;
    let samples = qnc_transcribe_jobs::to_whisper_rate(&sound.samples, sound.rate)?;
    // The speech itself goes to the aligner (contract version 2): no file is written.
    let model = qnc_ai_catalog::model_name(&qnc_ai_catalog::installed_letter_aligner()?.model);
    let wav = qnc_plugin_process::Bytes(qnc_transcribe_jobs::wav_bytes(&samples)?);
    let laid = aligner.align(&qnc_letter_align::AlignRequest { model, wav, words: words.iter().map(|word| word.text.clone()).collect() });
    // Seconds of the speech as clip frames, fractions kept: the speech starts at `from_seconds`.
    let frame = |seconds: f64| ((sound.from_seconds + seconds - place.frame_zero_seconds) * fps).max(0.0);
    let kept: Vec<WordLetters> = words
        .iter()
        .zip(laid?)
        .map(|(word, laid)| WordLetters {
            word_from: word.from,
            text: word.text.clone(),
            letters: laid.letters.into_iter().map(|letter| Letter { letter: letter.letter, from: letter.start.map(frame), to: letter.end.map(frame) }).collect(),
        })
        .collect();
    let target = qnc_db_broker::ProjectDbTarget::for_project(locator.reader(), locator.settings()).map_err(|error| error)?;
    qnc_letter_db::save(&target, clip_id, "wav2vec2-parlaspeech-hr", kept)
}

fn log(line: &str) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}

//! The words heard in the takes of a script's voice records (user 2026-11-08: "while the OFF is
//! read wav2vec2 transcribes it, and that checks that it was read correctly"; first step: after
//! Stop). For every take without its words, off the form thread and only while the player is idle
//! and nothing records: the take's own WAV is read, brought to what the engine takes and
//! transcribed by the Croatian engine of this computer; the words, with their seconds in the take,
//! are kept with the take through the script's database. The database is the truth; what is held
//! here is the job and the takes that failed in this run.

use std::{
    collections::HashSet,
    sync::{atomic::AtomicBool, mpsc},
};

use qnc_script_db::{HeardWord, Voice};
use qnc_still_locator::StillLocator;

pub const MODULE_ID: &str = "qnc.module.take-heard";

/// A take whose words are to be heard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub block_id: i64,
    pub take: u16,
    pub wav_uri: String,
}

/// The takes of `voices` without their words, the last recorded first.
pub fn wanted<'a>(voices: impl IntoIterator<Item = &'a Voice>) -> Vec<Wanted> {
    let mut takes: Vec<Wanted> = voices
        .into_iter()
        .flat_map(|voice| voice.takes.iter().filter(|take| take.heard.is_none()).map(|take| Wanted { block_id: voice.block_id, take: take.take, wav_uri: take.wav_uri.clone() }))
        .collect();
    takes.reverse();
    takes
}

#[derive(Default)]
pub struct TakeHeard {
    project_id: Option<String>,
    job: Option<(Wanted, mpsc::Receiver<Result<(), String>>)>,
    /// Takes that failed in this run: not asked again (they would be asked every poll).
    failed: HashSet<(i64, u16)>,
    note: Option<String>,
}

impl TakeHeard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Why takes are not being heard, if they are not.
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Starts the next of `takes` while `idle`; true when one was finished (the caller reads the
    /// script again).
    pub fn poll(&mut self, locator: &StillLocator, takes: &[Wanted], idle: bool) -> bool {
        if self.project_id.as_deref() != Some(locator.project_id()) {
            *self = Self { project_id: Some(locator.project_id().to_string()), ..Self::default() };
        }
        let finished = self.take_finished();
        if !takes.is_empty() {
            if let Some(reason) = barred(locator) {
                self.note = Some(reason);
                return finished;
            }
        }
        if self.job.is_none() && idle {
            if let Some(take) = takes.iter().find(|take| !self.failed.contains(&(take.block_id, take.take))) {
                let (send, done) = mpsc::channel();
                let (locator, work) = (locator.clone(), take.clone());
                let spawned = std::thread::Builder::new().name("qnc-take-heard".into()).spawn(move || _ = send.send(hear(&locator, &work)));
                if spawned.is_ok() {
                    self.job = Some((take.clone(), done));
                }
            }
        }
        finished
    }

    fn take_finished(&mut self) -> bool {
        let Some((take, done)) = &self.job else { return false };
        match done.try_recv() {
            Ok(Ok(())) => {
                log(&format!("take-heard block={} take={} heard", take.block_id, take.take));
                self.job = None;
                self.note = None;
                true
            }
            Ok(Err(error)) => {
                log(&format!("take-heard block={} take={} failed: {error}", take.block_id, take.take));
                self.failed.insert((take.block_id, take.take));
                self.note = Some(error);
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
}

/// Why this project's takes cannot be heard: it does not transcribe, or its speech is not the
/// language of the engine.
fn barred(locator: &StillLocator) -> Option<String> {
    let settings = locator.settings();
    match settings.transcription_enabled() {
        Ok(true) => {}
        Ok(false) => return Some(qnc_ui_text::text("script.read_check_off").into()),
        Err(error) => return Some(error.to_string()),
    }
    match settings.speech_language() {
        Ok(language) if language == "hr" => None,
        Ok(_) => Some(qnc_ui_text::text("script.read_check_language").into()),
        Err(error) => Some(error.to_string()),
    }
}

/// The words of a take, kept with it.
fn hear(locator: &StillLocator, take: &Wanted) -> Result<(), String> {
    let settings = locator.settings();
    let file = qnc_voice_take::voice_file(locator, &take.wav_uri)?;
    let (samples, rate) = qnc_voice_record::read_take(&file)?;
    let samples = qnc_transcribe_jobs::to_whisper_rate(&samples, rate)?;
    let language = settings.speech_language().map_err(|e| e.to_string())?;
    let transcript = qnc_transcribe_run::hear_samples(&samples, &language, &AtomicBool::new(false))?;
    let words = transcript.segments.iter().flat_map(|segment| &segment.words).map(|word| HeardWord { text: word.text.trim().to_string(), start: word.start, end: word.end }).filter(|word| !word.text.is_empty()).collect();
    let target = qnc_db_broker::ProjectDbTarget::for_project(locator.reader(), settings)?;
    qnc_script_db::set_take_heard(&target, take.block_id, take.take, words).map_err(|e| e.to_string())
}

fn log(line: &str) {
    qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line);
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_script_db::VoiceTake;

    fn take(number: u16, heard: Option<Vec<HeardWord>>) -> VoiceTake {
        VoiceTake { take: number, wav_uri: format!("T{number}.wav"), seconds: 2.0, tc: String::new(), text: String::new(), heard }
    }

    #[test]
    fn the_takes_without_their_words_are_wanted_the_last_recorded_first() {
        let voice = Voice { block_id: 7, name: "P".into(), clip_uri: "c".into(), chosen: 3, takes: vec![take(1, None), take(2, Some(Vec::new())), take(3, None)] };
        let found: Vec<u16> = wanted([&voice]).into_iter().map(|take| take.take).collect();
        assert_eq!(found, [3, 1], "a take in which nothing was heard is heard already");
    }
}

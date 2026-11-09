//! The reading of a whole story in one mono recording (user 2026-11-09): the journalist reads
//! every OFF in one go and marks each new reading with a press (Space, a mouse click, a tap on
//! the pad). The marks are the seconds of the recording where a press came, exact to the
//! sound, and a short beep answers on the computer's output. Nothing is written to a database;
//! the caller does.

use std::path::Path;

use qnc_still_locator::StillLocator;
use qnc_voice_record::Recorder;

use crate::Result;

/// Two presses closer than this are one (a bounce of the key or a double tap).
const LEAST_APART: f64 = 0.3;

/// A reading being recorded.
pub struct Session {
    recorder: Recorder,
    wav_uri: String,
    marks: Vec<f64>,
}

/// A finished recording of the story.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordedSession {
    pub wav_uri: String,
    pub seconds: f64,
    pub timecode: String,
    pub marks: Vec<f64>,
    pub silent: bool,
}

/// A press at `at`, unless it is on the one before.
pub(crate) fn add_mark(marks: &mut Vec<f64>, at: f64) -> bool {
    if marks.last().is_some_and(|last| at - last < LEAST_APART) {
        return false;
    }
    marks.push(at);
    true
}

impl Session {
    /// Opens the microphone: call it off the form thread.
    pub fn start(locator: &StillLocator, setting: &Path) -> Result<Self> {
        let settings = locator.settings();
        let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
        let (folder, base) = settings.voice_location(&project_dir).map_err(|e| e.message)?;
        let rate = settings.audio_sample_rate().map_err(|e| e.message)?;
        let (_, _, fps) = settings.video_picture().map_err(|e| e.message)?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let wav = format!("{}session_{}.wav", crate::record_name(&settings.project_name, ""), qnc_voice_record::file_stamp());
        let recorder = Recorder::start(folder.join(&wav), &project_dir, rate, fps, setting)?;
        Ok(Self { recorder, wav_uri: format!("{base}/{wav}"), marks: Vec::new() })
    }

    /// A press: a new reading starts here, the beep says it was taken.
    pub fn mark(&mut self) -> bool {
        let added = add_mark(&mut self.marks, self.recorder.seconds());
        if added {
            qnc_voice_record::beep();
        }
        added
    }

    pub fn marks(&self) -> usize {
        self.marks.len()
    }

    pub fn seconds(&self) -> f64 {
        self.recorder.seconds()
    }

    pub fn take_peak(&self) -> f32 {
        self.recorder.take_peak()
    }

    /// Stops the recording; its file stays in the voice folder of the project.
    pub fn finish(self) -> Result<RecordedSession> {
        let recording = self.recorder.stop()?;
        Ok(RecordedSession { wav_uri: self.wav_uri, seconds: recording.seconds, timecode: recording.timecode, marks: self.marks, silent: recording.silent })
    }

    pub fn cancel(self) -> Result<()> {
        self.recorder.cancel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_presses_too_close_are_one() {
        let mut marks = Vec::new();
        assert!(add_mark(&mut marks, 1.0));
        assert!(!add_mark(&mut marks, 1.2));
        assert!(add_mark(&mut marks, 1.31));
        assert_eq!(marks, vec![1.0, 1.31]);
    }
}

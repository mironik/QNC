//! One take of a script block (audit 2026-10-28: the take lived inside the script's piece). The
//! microphone of this computer records at the project's sample rate into the voice folder the
//! project names; stopped, the recording becomes a clip of the project at once (user
//! 2026-10-28): the title card of the block's text as its picture, its mark above it, the time
//! it was recorded as its timecode. The project's sound and picture are read as saved: a
//! missing or unusable value is a controlled error, never a guess. Nothing is written to a
//! database; the caller writes the take into its block.

use std::path::{Path, PathBuf};

use qnc_still_locator::StillLocator;
use qnc_voice_record::Recorder;

pub const MODULE_ID: &str = "qnc.module.voice-take";

pub type Result<T> = std::result::Result<T, String>;

/// A take that records.
pub struct Take {
    recorder: Recorder,
    folder: PathBuf,
    base: String,
    stem: String,
    project_dir: PathBuf,
    width: u32,
    height: u32,
    fps: f64,
}

/// A finished take, as its block keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    /// The clip of the take (QNC URI).
    pub uri: String,
    pub seconds: f64,
    /// The time of day it started.
    pub timecode: String,
}

impl Take {
    /// Starts the take of block `block` (it names the files); `setting` is the computer setting
    /// naming the microphone. Opens the microphone: call it off the form thread.
    pub fn start(locator: &StillLocator, block: i64, setting: &Path) -> Result<Self> {
        let settings = locator.settings();
        let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
        let (folder, base) = settings.voice_location(&project_dir).map_err(|e| e.message)?;
        let rate = settings.audio_sample_rate().map_err(|e| e.message)?;
        let (width, height, fps) = settings.video_picture().map_err(|e| e.message)?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let stem = format!("off_{block}_{}", qnc_voice_record::file_stamp());
        let recorder = Recorder::start(folder.join(format!("{stem}.wav")), &project_dir, rate, fps, setting)?;
        Ok(Self { recorder, folder, base, stem, project_dir, width, height, fps })
    }

    /// Seconds recorded so far.
    pub fn seconds(&self) -> f64 {
        self.recorder.seconds()
    }

    /// The loudest sample since the last call (0..1).
    pub fn take_peak(&self) -> f32 {
        self.recorder.take_peak()
    }

    /// Stops the take and makes its clip: `text` on the title card under `heading`. Writes a
    /// file: call it off the form thread.
    pub fn finish(self, text: &str, heading: &str) -> Result<Recorded> {
        let recording = self.recorder.stop()?;
        let out = self.folder.join(format!("{}.mxf", self.stem));
        let clip = qnc_voice_clip::VoiceClip { sound: &recording.path, text, heading, width: self.width, height: self.height, fps: self.fps, timecode: &recording.timecode, out: &out, project_dir: &self.project_dir };
        qnc_voice_clip::make(&clip)?;
        Ok(Recorded { uri: format!("{}/{}.mxf", self.base, self.stem), seconds: recording.seconds, timecode: recording.timecode })
    }
}

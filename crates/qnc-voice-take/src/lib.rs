//! The takes of a script block (audit 2026-10-28: the take lived inside the script's piece; user
//! 2026-10-30: every take of the same OFF is one more channel of one record). The microphone of
//! this computer records at the project's sample rate into the voice folder the project names;
//! stopped, the take is kept as its own WAV and the record's clip is made again with every take
//! as one mono channel (piece `qnc-voice-clip`): the title card of the block's text as its
//! picture, its mark above it, the time its first take was recorded as its timecode. A take
//! cancelled is thrown away and adds nothing.
//!
//! A record is named once, at its first take, `<project>_<mark>` (`Mironik_OFF2`): its clip is
//! `<name>.mxf` and its takes `<name>_T1.wav`, `<name>_T2.wav`...; the name stays when the
//! blocks are moved (the script keeps the link). The project's sound and picture are read as
//! saved: a missing or unusable value is a controlled error, never a guess. Nothing is written
//! to a database; the caller writes the take into its block.

use std::path::{Path, PathBuf};

use qnc_still_locator::StillLocator;
use qnc_voice_record::Recorder;

pub const MODULE_ID: &str = "qnc.module.voice-take";

pub type Result<T> = std::result::Result<T, String>;

/// The record a block has so far (its name and its takes in the order recorded).
#[derive(Debug, Clone, PartialEq)]
pub struct Earlier {
    pub name: String,
    /// Each take's recording (QNC URI of its WAV) and length, seconds.
    pub takes: Vec<(String, f64)>,
    /// The timecode of the first take (the clip's).
    pub first_tc: String,
}

/// A take that records.
pub struct Take {
    recorder: Recorder,
    folder: PathBuf,
    base: String,
    name: String,
    number: u16,
    earlier: Vec<(PathBuf, f64)>,
    first_tc: Option<String>,
    wav_uri: String,
    project_dir: PathBuf,
    width: u32,
    height: u32,
    fps: f64,
}

/// A finished take, as its block keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    /// The record's name and clip (QNC URI).
    pub name: String,
    pub clip_uri: String,
    /// The take: its number (and channel), its recording (QNC URI), length and the time of day
    /// it started.
    pub number: u16,
    pub wav_uri: String,
    pub seconds: f64,
    pub timecode: String,
}

/// The name of a record: the project's name and the block's mark (`OFF 2` is `OFF2`), as a
/// file may be named on every OS.
pub fn record_name(project: &str, mark: &str) -> String {
    let mark: String = mark.chars().filter(|c| !c.is_whitespace()).collect();
    qnc_work_settings::export_file_name(&format!("{}_{mark}", project.trim()))
}

impl Take {
    /// Starts the next take of a block whose mark is `mark`, after the takes it has
    /// (`earlier`); `setting` is the computer setting naming the microphone. Opens the
    /// microphone: call it off the form thread.
    pub fn start(locator: &StillLocator, mark: &str, earlier: Option<Earlier>, setting: &Path) -> Result<Self> {
        let settings = locator.settings();
        let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
        let (folder, base) = settings.voice_location(&project_dir).map_err(|e| e.message)?;
        let rate = settings.audio_sample_rate().map_err(|e| e.message)?;
        let (width, height, fps) = settings.video_picture().map_err(|e| e.message)?;
        let name = earlier.as_ref().map_or_else(|| record_name(&settings.project_name, mark), |earlier| earlier.name.clone());
        let taken = earlier.as_ref().map_or(0, |earlier| earlier.takes.len());
        if taken >= qnc_voice_clip::MAX_CHANNELS {
            return Err(qnc_ui_text::fill("voice.full", &[("most", &qnc_voice_clip::MAX_CHANNELS.to_string())]));
        }
        let earlier_files = earlier.as_ref().map_or(Ok(Vec::new()), |earlier| earlier.takes.iter().map(|(uri, seconds)| Ok((file_of(&folder, &base, uri)?, *seconds))).collect::<Result<Vec<_>>>())?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let number = taken as u16 + 1;
        let wav = take_file(&name, number);
        let recorder = Recorder::start(folder.join(&wav), &project_dir, rate, fps, setting)?;
        Ok(Self { recorder, folder, base: base.clone(), wav_uri: format!("{base}/{wav}"), name, number, earlier: earlier_files, first_tc: earlier.map(|earlier| earlier.first_tc), project_dir, width, height, fps })
    }

    /// The take's number (the channel it becomes).
    pub fn number(&self) -> u16 {
        self.number
    }

    /// Seconds recorded so far.
    pub fn seconds(&self) -> f64 {
        self.recorder.seconds()
    }

    /// The loudest sample since the last call (0..1).
    pub fn take_peak(&self) -> f32 {
        self.recorder.take_peak()
    }

    /// Stops the take and makes the record's clip again with it as its last channel: `text` on
    /// the title card under `heading`. Writes files: call it off the form thread.
    pub fn finish(self, text: &str, heading: &str) -> Result<Recorded> {
        let recording = self.recorder.stop()?;
        let mut sounds: Vec<PathBuf> = self.earlier.iter().map(|(file, _)| file.clone()).collect();
        sounds.push(recording.path.clone());
        let seconds = self.earlier.iter().map(|(_, seconds)| *seconds).fold(recording.seconds, f64::max);
        let timecode = self.first_tc.clone().unwrap_or_else(|| recording.timecode.clone());
        let out = self.folder.join(format!("{}.mxf", self.name));
        let clip = qnc_voice_clip::VoiceClip { sounds: &sounds, seconds, text, heading, width: self.width, height: self.height, fps: self.fps, timecode: &timecode, out: &out, project_dir: &self.project_dir };
        qnc_voice_clip::make(&clip)?;
        Ok(Recorded { clip_uri: format!("{}/{}.mxf", self.base, self.name), name: self.name, number: self.number, wav_uri: self.wav_uri, seconds: recording.seconds, timecode: recording.timecode })
    }

    /// Stops the take and throws it away (its file is left empty for the next take).
    pub fn cancel(self) -> Result<()> {
        self.recorder.cancel()
    }
}

/// The file behind `uri`, a clip or a take in the voice folder of the project on this computer
/// (to see that it is there). Reads no file.
pub fn voice_file(locator: &StillLocator, uri: &str) -> Result<PathBuf> {
    let settings = locator.settings();
    let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
    let (folder, base) = settings.voice_location(&project_dir).map_err(|e| e.message)?;
    file_of(&folder, &base, uri)
}

/// The file of take `number` of record `name`.
fn take_file(name: &str, number: u16) -> String {
    format!("{name}_T{number}.wav")
}

/// The file in the voice folder behind `uri` (a take of the record).
fn file_of(folder: &Path, base: &str, uri: &str) -> Result<PathBuf> {
    let relative = uri.strip_prefix(base).and_then(|rest| rest.strip_prefix('/')).filter(|rest| !rest.is_empty() && !rest.contains("..")).ok_or_else(|| format!("{uri}: {}", qnc_ui_text::text("voice.take_missing")))?;
    Ok(folder.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_record_is_named_after_the_project_and_the_mark_as_every_os_takes_it() {
        assert_eq!(record_name("Mironik teniski centar", "OFF 2"), "Mironik_teniski_centar_OFF2");
        assert_eq!(record_name("2026/10/30 Vijesti", "Take VO 1"), "2026_10_30_Vijesti_TakeVO1");
        assert_eq!(take_file("Mironik_OFF2", 3), "Mironik_OFF2_T3.wav");
    }

    #[test]
    fn a_take_is_found_only_in_the_voice_folder() {
        let folder = Path::new("p/products/voice");
        let base = "qnc://local/source/project-p1/file/products/voice";
        assert_eq!(file_of(folder, base, &format!("{base}/M_OFF1_T1.wav")).unwrap(), folder.join("M_OFF1_T1.wav"));
        assert!(file_of(folder, base, "qnc://local/source/x/file/a.wav").is_err());
        assert!(file_of(folder, base, &format!("{base}/../a.wav")).is_err());
    }
}

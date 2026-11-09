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

mod session;
pub use session::{RecordedSession, Session};

pub const MODULE_ID: &str = "qnc.module.voice-take";

pub type Result<T> = std::result::Result<T, String>;

/// The record a block has so far (its name and its takes in the order recorded).
#[derive(Debug, Clone, PartialEq)]
pub struct Earlier {
    pub name: String,
    /// How many readings it has.
    pub takes: u16,
}

/// A reading shorter than this is a press by mistake.
const SHORTEST_READING: f64 = 0.5;

/// A recording under way: Space, a click or a tap marks where a new reading starts (user
/// 2026-11-09: no channels; one recording, the presses divide it into readings).
pub struct Take {
    recorder: Recorder,
    name: String,
    number: u16,
    marks: Vec<f64>,
    maker: ReadingMaker,
}

/// Makes a reading's WAV and its clip in the voice folder of the project (from the settings as
/// saved); a recording of one OFF and of the whole story end the same way.
pub struct ReadingMaker {
    folder: PathBuf,
    base: String,
    /// The audio outputs of the project: the clip has a channel for each.
    outputs: usize,
    project_dir: PathBuf,
    width: u32,
    height: u32,
    fps: f64,
    fields: Option<bool>,
    rate: u32,
}

impl ReadingMaker {
    pub fn for_project(locator: &StillLocator) -> Result<Self> {
        let settings = locator.settings();
        let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
        let (folder, base) = settings.voice_location(&project_dir).map_err(|e| e.message)?;
        let rate = settings.audio_sample_rate().map_err(|e| e.message)?;
        let (width, height, fps) = settings.video_picture().map_err(|e| e.message)?;
        let fields = settings.video_fields().map_err(|e| e.message)?;
        let outputs = usize::from(settings.audio_channels().map_err(|e| e.message)?).max(1);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        Ok(Self { folder, base, outputs, project_dir, width, height, fps, fields, rate })
    }

    /// Reading `number` of record `name`: `samples` as its WAV and its mono clip, `text` on its
    /// title card. Writes files: call it off the form thread.
    #[allow(clippy::too_many_arguments)]
    pub fn make(&self, name: &str, number: u16, samples: &[f32], rate: u32, timecode: &str, text: &str, silent: bool) -> Result<Recorded> {
        let take_name = qnc_script_db::take_name(name, number);
        let wav = self.folder.join(format!("{take_name}.wav"));
        qnc_voice_record::write_take(&wav, samples, rate)?;
        let seconds = samples.len() as f64 / f64::from(rate);
        let out = self.folder.join(format!("{take_name}.mxf"));
        let sounds = vec![wav; self.outputs];
        let clip = qnc_voice_clip::VoiceClip { sounds: &sounds, seconds, text, heading: "", sequence: None, width: self.width, height: self.height, fps: self.fps, fields: self.fields, sample_rate: self.rate, timecode, out: &out, project_dir: &self.project_dir };
        qnc_voice_clip::make(&clip)?;
        let clip_uri = format!("{}/{take_name}.mxf", self.base);
        let record = qnc_voice_clip::describe(&clip, &clip_id(&take_name), &canonical(&clip_uri)?)?;
        Ok(Recorded { silent, clip_uri, name: name.to_string(), number, wav_uri: format!("{}/{take_name}.wav", self.base), seconds, timecode: timecode.to_string(), record })
    }
}

/// A finished take, as its block keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Recorded {
    /// The record's name and this reading's clip (QNC URI).
    pub name: String,
    pub clip_uri: String,
    /// The reading: its number, its recording (QNC URI), length and the time of day it started.
    pub number: u16,
    pub wav_uri: String,
    pub seconds: f64,
    pub timecode: String,
    /// The microphone gave no signal in this take.
    pub silent: bool,
    /// This reading's clip as the catalog keeps it (piece `qnc-voice-clip`).
    pub record: qnc_media_metadata::ClipMetadata,
}

/// Where each reading of a recording of `samples` samples is, cut at `marks` (seconds); a
/// reading too short to be one goes.
pub fn readings(samples: usize, rate: u32, marks: &[f64]) -> Vec<(usize, usize)> {
    let at = |seconds: f64| ((seconds * f64::from(rate)) as usize).min(samples);
    let cuts: Vec<usize> = std::iter::once(0).chain(marks.iter().map(|mark| at(*mark))).chain(std::iter::once(samples)).collect();
    let least = (SHORTEST_READING * f64::from(rate)) as usize;
    let kept: Vec<(usize, usize)> = cuts.windows(2).map(|pair| (pair[0], pair[1])).filter(|(from, to)| to.saturating_sub(*from) >= least).collect();
    if kept.is_empty() {
        vec![(0, samples)]
    } else {
        kept
    }
}

/// The name of a record: the project's name and the block's mark (`OFF 2` is `OFF2`), as a
/// file may be named on every OS.
pub fn record_name(project: &str, mark: &str) -> String {
    let mark: String = mark.chars().filter(|c| !c.is_whitespace()).collect();
    qnc_work_settings::export_file_name(&format!("{}_{mark}", project.trim()))
}

/// The catalog record of a voice record's clip as it stands (its takes' lengths, its first
/// take's timecode), from the project's settings as saved: what `finish` wrote, nothing probed.
/// A record made before the catalog knew voice clips is described the same way.
pub fn describe_record(settings: &qnc_work_settings::WorkSettings, name: &str, clip_uri: &str, takes: &[f64], first_tc: &str) -> Result<qnc_media_metadata::ClipMetadata> {
    let (width, height, fps) = settings.video_picture().map_err(|e| e.message)?;
    let fields = settings.video_fields().map_err(|e| e.message)?;
    let sample_rate = settings.audio_sample_rate().map_err(|e| e.message)?;
    // A reading is on every output of the project (the player asks for that many channels).
    let sounds = vec![PathBuf::new(); usize::from(settings.audio_channels().map_err(|e| e.message)?).max(1)];
    let seconds = takes.iter().copied().fold(0.0, f64::max);
    let clip = qnc_voice_clip::VoiceClip { sounds: &sounds, seconds, text: "", heading: "", sequence: None, width, height, fps, fields, sample_rate, timecode: first_tc, out: Path::new(""), project_dir: Path::new("") };
    qnc_voice_clip::describe(&clip, &clip_id(name), &canonical(clip_uri)?)
}

/// A file of the voice folder as the source reference of the catalog writes it (its path
/// percent encoded), the form the player reads; the script keeps the plain form.
pub fn canonical(uri: &str) -> Result<String> {
    let (source, path) = uri.split_once("/file/").ok_or_else(|| format!("{uri}: {}", qnc_ui_text::text("voice.take_missing")))?;
    qnc_source_contract::SourceReference::new(source, path).map(|reference| reference.uri()).map_err(|e| format!("{uri}: {e}"))
}

/// The catalog id of a record's clip: `voice-` and its name, with every character a clip id
/// may not have as `_` (one record, one clip, whatever the blocks do).
pub fn clip_id(name: &str) -> String {
    let id: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(200).collect();
    format!("voice-{id}")
}

impl Take {
    /// Starts the next take of a block whose mark is `mark`, after the takes it has
    /// (`earlier`); `setting` is the computer setting naming the microphone. Opens the
    /// microphone: call it off the form thread.
    pub fn start(locator: &StillLocator, mark: &str, earlier: Option<Earlier>, setting: &Path) -> Result<Self> {
        let settings = locator.settings();
        let maker = ReadingMaker::for_project(locator)?;
        let name = earlier.as_ref().map_or_else(|| record_name(&settings.project_name, mark), |earlier| earlier.name.clone());
        let number = earlier.as_ref().map_or(0, |earlier| earlier.takes) + 1;
        let raw = format!("{name}_recording_{number}.wav");
        let recorder = Recorder::start(maker.folder.join(&raw), &maker.project_dir, maker.rate, maker.fps, setting)?;
        Ok(Self { recorder, name, number, marks: Vec::new(), maker })
    }

    /// A press: a new reading starts here, the beep says it was taken.
    pub fn mark(&mut self) -> bool {
        let added = session::add_mark(&mut self.marks, self.recorder.seconds());
        if added {
            qnc_voice_record::beep();
        }
        added
    }

    /// The presses so far.
    pub fn marks(&self) -> usize {
        self.marks.len()
    }

    /// Seconds recorded so far.
    pub fn seconds(&self) -> f64 {
        self.recorder.seconds()
    }

    /// The loudest sample since the last call (0..1).
    pub fn take_peak(&self) -> f32 {
        self.recorder.take_peak()
    }

    /// Stops the recording and makes every reading its own mono clip: `text` on the title card
    /// under `heading`. Writes files: call it off the form thread.
    pub fn finish(self, text: &str, _heading: &str) -> Result<Vec<Recorded>> {
        let recording = self.recorder.stop()?;
        let (samples, rate) = qnc_voice_record::read_take(&recording.path)?;
        readings(samples.len(), rate, &self.marks)
            .into_iter()
            .enumerate()
            .map(|(index, (from, to))| self.maker.make(&self.name, self.number + index as u16, &samples[from..to], rate, &recording.timecode, text, recording.silent))
            .collect()
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
    }

    #[test]
    fn the_catalog_gets_the_clip_address_as_the_source_reference_writes_it() {
        let uri = canonical("qnc://local/source/project-p1/file/products/voice/P_OFF1.mxf").unwrap();
        assert_eq!(uri, "qnc://local/source/project-p1/file/products/voice/P%5FOFF1%2Emxf");
        assert_eq!(qnc_source_contract::SourceReference::from_uri(&uri).unwrap().uri(), uri);
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

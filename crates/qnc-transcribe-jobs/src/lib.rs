//! Speech to text as a plug-in program (contract `qnc.transcribe` 1; user 2026-10-26: the best
//! offline transcription, a block that can be replaced without building QNC). The application
//! writes the sound to transcribe as a 16 kHz mono WAV into an empty temporary folder and asks
//! the program the AI catalog names; the program answers the segments in parts, each with its
//! words and their times. The program writes nothing and decides nothing about the media; a
//! program that cannot run the backend the catalog chose answers an error, nothing falls back.

mod audio;

pub use audio::{read_wav, to_whisper_rate, write_wav, WHISPER_RATE};
use qnc_plugin_process::{Plugin, PluginSpec, Ready};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Mutex},
    time::Duration,
};

pub const MODULE_ID: &str = "qnc.module.transcribe-jobs";
pub const CONTRACT: &str = "qnc.transcribe";
pub const CONTRACT_VERSION: u32 = 1;
/// How long the program may stay silent within a job: loading the model and one window of
/// sound on a processor can take this long.
pub const TIMEOUT: Duration = Duration::from_secs(600);

/// Where the engine runs. The catalog chooses; the program never runs another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// The graphics card through Vulkan (Intel, AMD, NVIDIA).
    Vulkan,
    /// The processor.
    Cpu,
}

/// Transcribe `wav` (16 kHz mono 16-bit PCM) with `model`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transcribe {
    pub model: PathBuf,
    /// The voice activity model: only speech is transcribed (no text invented on silence).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vad_model: Option<PathBuf>,
    pub backend: Backend,
    pub wav: PathBuf,
    /// A language code (`hr`, `en`...) or `auto`.
    pub language: String,
    pub threads: u32,
}

/// One word and where it is in the sound (seconds from the start of the WAV).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Word {
    pub start: f64,
    pub end: f64,
    pub text: String,
    /// The engine's probability of the word.
    pub p: f32,
}

/// One segment of speech.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Done {
    pub language: String,
}

/// The whole answer of a job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub language: String,
    pub segments: Vec<Segment>,
    /// Which program made it (`plugin_id@plugin_version`).
    pub engine: String,
}

/// What a transcription program does; [`serve`] answers the application with it.
pub trait Engine {
    /// The backends this program can run.
    fn backends(&self) -> Vec<Backend>;
    /// Transcribes the job, handing every segment to `on_segment` as it is made; the language.
    fn transcribe(
        &mut self,
        job: &Transcribe,
        cancel: &AtomicBool,
        on_segment: &mut dyn FnMut(Segment) -> Result<(), String>,
    ) -> Result<String, String>;
}

/// The program side: says it is `plugin_id` of `plugin_version` and answers each job.
pub fn serve(engine: &mut dyn Engine, plugin_id: &str, plugin_version: &str, about_engine: &str) -> Result<(), String> {
    let ready = Ready {
        contract: CONTRACT.into(),
        version: CONTRACT_VERSION,
        plugin_id: plugin_id.into(),
        plugin_version: plugin_version.into(),
        about: serde_json::json!({ "engine": about_engine, "backends": engine.backends() }),
    };
    qnc_plugin_process::serve_parts(ready, |op, body, cancel, send| match op {
        "transcribe" => {
            let job: Transcribe = parse(body)?;
            let language = engine.transcribe(&job, cancel, &mut |segment| send(to_value(segment)?))?;
            to_value(Done { language })
        }
        other => Err(format!("unknown transcription job {other}")),
    })
}

/// The application side: one transcription program, started for a job and kept for the next.
/// Jobs run one at a time on it: the model is large and holds the graphics card.
pub struct Transcriber {
    plugin: Mutex<Plugin>,
}

impl std::fmt::Debug for Transcriber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transcriber").finish_non_exhaustive()
    }
}

impl Transcriber {
    /// The program named `program`, beside the application.
    pub fn beside_application(program: &str) -> Result<Self, String> {
        Ok(Self::new(qnc_plugin_process::beside_application(program)?))
    }

    pub fn new(executable: PathBuf) -> Self {
        let spec = PluginSpec { executable, contract: CONTRACT.into(), version: CONTRACT_VERSION, timeout: TIMEOUT };
        Self { plugin: Mutex::new(Plugin::new(spec)) }
    }

    /// Transcribes `job`, handing each segment to `on_segment` as it comes.
    pub fn transcribe(
        &self,
        job: &Transcribe,
        cancel: &AtomicBool,
        on_segment: &mut dyn FnMut(&Segment),
    ) -> Result<Transcript, String> {
        let plugin = self.plugin.lock().unwrap_or_else(|e| e.into_inner());
        let ready = plugin.ready()?;
        let backends: Vec<Backend> = ready
            .about
            .get("backends")
            .cloned()
            .map(parse)
            .transpose()?
            .unwrap_or_default();
        if !backends.contains(&job.backend) {
            return Err(format!(
                "the transcription program {} does not run the {:?} backend the catalog chose",
                ready.plugin_id, job.backend
            ));
        }
        let mut segments = Vec::new();
        let done: Done = parse(plugin.call_parts("transcribe", to_value(job)?, cancel, &mut |part| {
            let segment: Segment = parse(part)?;
            on_segment(&segment);
            segments.push(segment);
            Ok(())
        })?)?;
        Ok(Transcript {
            language: done.language,
            segments,
            engine: format!("{}@{}", ready.plugin_id, ready.plugin_version),
        })
    }
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("outside the transcription contract: {e}"))
}

fn to_value(value: impl Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;

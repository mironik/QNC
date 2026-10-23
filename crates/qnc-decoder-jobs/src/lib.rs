//! The decoder jobs besides playback as a plug-in program (contract `qnc.decoder-jobs` 1;
//! user 2026-10-23: "blocks like a DLL, so that a change in one block cannot break another").
//! Playback already reaches a decoder program through `qnc_packets_v1`; these are the other
//! jobs a decoder does for QNC: the filmstrip pictures, the pictures around a trim, the key
//! pictures, the sound of a wave, and pictures and sound already read from a card. A decoder
//! registered with a jobs plug-in does all of them through its program, so replacing the
//! decoder replaces them together and nothing keeps calling the old one.
//!
//! A job names the medium the decoder reads and, for pictures, the empty temporary folder of
//! the application it writes `000.jpg`, `001.jpg`... into; sound comes back in parts. The
//! plug-in writes nowhere else and decides nothing about the media: what is asked for comes
//! from the stored record. A plug-in that fails gives a controlled error, nothing falls back.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use qnc_plugin_process::{Plugin, PluginSpec, Ready};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    path::PathBuf,
    sync::{atomic::AtomicBool, Mutex},
    time::Duration,
};

pub const MODULE_ID: &str = "qnc.module.decoder-jobs";
pub const CONTRACT: &str = "qnc.decoder-jobs";
pub const CONTRACT_VERSION: u32 = 1;
/// How long a plug-in may stay silent within a job.
pub const TIMEOUT: Duration = Duration::from_secs(120);
/// Processes kept for jobs that run at the same time.
const KEPT: usize = 8;
/// Samples gathered into one part of a sound answer.
const PART_SAMPLES: usize = 1 << 16;

/// How a picture of the filmstrip is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PictureMode {
    /// The picture at the time, decoded from there.
    Random,
    /// The key picture nearest the time.
    Keyframe,
    /// The exact picture at the time.
    Exact,
}

/// Pictures at `seconds` of `media`, as `000.jpg`... of `thumb` size in `out_dir`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frames {
    /// The decoder executable the catalog registers (host configuration, never a media identity).
    pub decoder: PathBuf,
    pub media: PathBuf,
    pub seconds: Vec<f64>,
    /// The stored frame rate of the medium.
    pub frame_rate: (u32, u32),
    pub mode: PictureMode,
    pub thumb: [u32; 2],
    pub out_dir: PathBuf,
}

/// `count` consecutive pictures from the exact picture at `start_sec`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    pub decoder: PathBuf,
    pub media: PathBuf,
    pub start_sec: f64,
    pub count: u32,
    pub thumb: [u32; 2],
    pub out_dir: PathBuf,
}

/// The presentation times of the key pictures from about `start_sec` for `duration_sec`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyPictures {
    pub decoder: PathBuf,
    pub media: PathBuf,
    pub start_sec: f64,
    pub duration_sec: f64,
}

/// The audio `streams` of `media` merged in order into interleaved f32 of `channels`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioStreams {
    pub decoder: PathBuf,
    pub media: PathBuf,
    /// The stored container.
    pub container: String,
    pub streams: Vec<u32>,
    pub channels: u32,
}

/// Sound already read (`demuxer` of its stored codec) decoded to interleaved f32 of `channels`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioBytes {
    pub decoder: PathBuf,
    pub demuxer: String,
    /// The bytes, base64.
    pub bytes: String,
    pub channels: u32,
}

/// Encoded pictures already read (one file each) decoded to `000.jpg`... in `out_dir`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodePictures {
    pub decoder: PathBuf,
    pub demuxer: String,
    pub pictures: Vec<PathBuf>,
    pub thumb: [u32; 2],
    pub out_dir: PathBuf,
}

/// A part of a sound answer: interleaved f32 little-endian samples, base64.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Samples {
    pub samples: String,
}

/// Hands sound samples on as they are decoded.
pub type OnSamples<'a> = dyn FnMut(&[f32]) -> Result<(), String> + 'a;

/// What a decoder plug-in program does; [`serve`] answers the application with it.
pub trait DecoderJobs {
    fn frames(&self, job: &Frames, cancel: &AtomicBool) -> Result<(), String>;
    fn run(&self, job: &Run, cancel: &AtomicBool) -> Result<(), String>;
    fn key_pictures(&self, job: &KeyPictures, cancel: &AtomicBool) -> Result<Vec<f64>, String>;
    fn audio_streams(&self, job: &AudioStreams, cancel: &AtomicBool, on_samples: &mut OnSamples) -> Result<(), String>;
    fn audio_bytes(&self, demuxer: &str, bytes: Vec<u8>, channels: u32, decoder: &std::path::Path, cancel: &AtomicBool, on_samples: &mut OnSamples) -> Result<(), String>;
    fn decode_pictures(&self, job: &DecodePictures, cancel: &AtomicBool) -> Result<(), String>;
}

/// The program side: says it is `plugin_id` of `plugin_version` and answers each job with `jobs`.
pub fn serve(jobs: &dyn DecoderJobs, plugin_id: &str, plugin_version: &str) -> Result<(), String> {
    let ready = Ready {
        contract: CONTRACT.into(),
        version: CONTRACT_VERSION,
        plugin_id: plugin_id.into(),
        plugin_version: plugin_version.into(),
        about: Value::Null,
    };
    qnc_plugin_process::serve_parts(ready, |op, body, cancel, send| {
        let mut sound = SoundParts { send, gathered: Vec::new() };
        match op {
            "frames" => jobs.frames(&parse(body)?, cancel).map(|()| Value::Null),
            "run" => jobs.run(&parse(body)?, cancel).map(|()| Value::Null),
            "key_pictures" => jobs.key_pictures(&parse(body)?, cancel).and_then(to_value),
            "audio_streams" => {
                let job: AudioStreams = parse(body)?;
                jobs.audio_streams(&job, cancel, &mut |samples| sound.add(samples))?;
                sound.flush().map(|()| Value::Null)
            }
            "audio_bytes" => {
                let job: AudioBytes = parse(body)?;
                let bytes = STANDARD.decode(&job.bytes).map_err(|e| format!("sound bytes are not base64: {e}"))?;
                jobs.audio_bytes(&job.demuxer, bytes, job.channels, &job.decoder, cancel, &mut |samples| sound.add(samples))?;
                sound.flush().map(|()| Value::Null)
            }
            "decode_pictures" => jobs.decode_pictures(&parse(body)?, cancel).map(|()| Value::Null),
            other => Err(format!("unknown decoder job {other}")),
        }
    })
}

struct SoundParts<'a, 'b> {
    send: &'a mut qnc_plugin_process::PartSender<'b>,
    gathered: Vec<f32>,
}

impl SoundParts<'_, '_> {
    fn add(&mut self, samples: &[f32]) -> Result<(), String> {
        self.gathered.extend_from_slice(samples);
        if self.gathered.len() >= PART_SAMPLES {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        if self.gathered.is_empty() {
            return Ok(());
        }
        let bytes: Vec<u8> = self.gathered.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        self.gathered.clear();
        (self.send)(to_value(Samples { samples: STANDARD.encode(bytes) })?)
    }
}

/// The application side: a decoder's jobs program, started for a job and kept for the next.
pub struct JobsClient {
    spec: PluginSpec,
    idle: Mutex<Vec<Plugin>>,
}

impl std::fmt::Debug for JobsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JobsClient").field("program", &self.spec.executable).finish()
    }
}

impl JobsClient {
    /// The jobs program named `program`, beside the application.
    pub fn beside_application(program: &str) -> Result<Self, String> {
        Ok(Self::new(qnc_plugin_process::beside_application(program)?))
    }

    pub fn new(executable: PathBuf) -> Self {
        let spec = PluginSpec { executable, contract: CONTRACT.into(), version: CONTRACT_VERSION, timeout: TIMEOUT };
        Self { spec, idle: Mutex::new(Vec::new()) }
    }

    pub fn frames(&self, job: &Frames, cancel: &AtomicBool) -> Result<(), String> {
        self.call("frames", to_value(job)?, cancel, &mut |_| Err(no_part())).map(drop)
    }

    pub fn run(&self, job: &Run, cancel: &AtomicBool) -> Result<(), String> {
        self.call("run", to_value(job)?, cancel, &mut |_| Err(no_part())).map(drop)
    }

    pub fn key_pictures(&self, job: &KeyPictures, cancel: &AtomicBool) -> Result<Vec<f64>, String> {
        parse(self.call("key_pictures", to_value(job)?, cancel, &mut |_| Err(no_part()))?)
    }

    pub fn audio_streams(&self, job: &AudioStreams, cancel: &AtomicBool, on_samples: &mut OnSamples) -> Result<(), String> {
        self.call("audio_streams", to_value(job)?, cancel, &mut |part| samples(part, on_samples)).map(drop)
    }

    pub fn audio_bytes(&self, decoder: PathBuf, demuxer: &str, bytes: &[u8], channels: u32, cancel: &AtomicBool, on_samples: &mut OnSamples) -> Result<(), String> {
        let job = AudioBytes { decoder, demuxer: demuxer.into(), bytes: STANDARD.encode(bytes), channels };
        self.call("audio_bytes", to_value(job)?, cancel, &mut |part| samples(part, on_samples)).map(drop)
    }

    pub fn decode_pictures(&self, job: &DecodePictures, cancel: &AtomicBool) -> Result<(), String> {
        self.call("decode_pictures", to_value(job)?, cancel, &mut |_| Err(no_part())).map(drop)
    }

    /// One job on a kept process, or on a new one when every kept one is busy.
    fn call(&self, op: &str, body: Value, cancel: &AtomicBool, on_part: &mut dyn FnMut(Value) -> Result<(), String>) -> Result<Value, String> {
        let plugin = self.idle.lock().unwrap_or_else(|e| e.into_inner()).pop().unwrap_or_else(|| Plugin::new(self.spec.clone()));
        let answer = plugin.call_parts(op, body, cancel, on_part);
        let mut idle = self.idle.lock().unwrap_or_else(|e| e.into_inner());
        if idle.len() < KEPT {
            idle.push(plugin);
        }
        answer
    }
}

fn samples(part: Value, on_samples: &mut OnSamples) -> Result<(), String> {
    let part: Samples = parse(part)?;
    let bytes = STANDARD.decode(part.samples).map_err(|e| format!("decoder sound is not base64: {e}"))?;
    if bytes.len() % 4 != 0 {
        return Err("decoder sound is not whole f32 samples".into());
    }
    let samples: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    on_samples(&samples)
}

fn no_part() -> String {
    "the decoder sent a part where its job has none".into()
}

fn parse<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("outside the decoder jobs contract: {e}"))
}

fn to_value(value: impl Serialize) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;

//! Recording the voice-over (user 2026-10-28, Media Assist Audio: the journalist records the
//! OFF on the laptop). The microphone of this computer (its default input, or the one named in
//! the computer setting `data/voice-input.json` `{"device": "<name>"}`) is recorded, its first
//! channel, at the project's sample rate, into a mono 24-bit WAV file inside the project folder.
//! A microphone that does not work at that rate is a clear error, never a silent resample. The
//! time of day the recording started is its timecode, as a camera does.
//!
//! The file is written under its final name from the start (the project folder does not let a
//! file be renamed). It knows no form, script or database: the caller names the file and keeps
//! where it is.

use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub const MODULE_ID: &str = "qnc.module.voice-record";

/// The computer setting naming the microphone (inside the QNC `data` folder).
pub const INPUT_SETTING: &str = "voice-input.json";

pub type Result<T> = std::result::Result<T, String>;

/// A finished recording.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub path: PathBuf,
    pub seconds: f64,
    /// The time of day it started, as `HH:MM:SS:FF` at the project's frame rate.
    pub timecode: String,
}

/// A recording that runs; stop it to get the file.
pub struct Recorder {
    stop: Arc<AtomicBool>,
    peak: Arc<AtomicU32>,
    samples: Arc<AtomicU32>,
    rate: u32,
    thread: Option<JoinHandle<Result<Recording>>>,
}

impl Recorder {
    /// Starts recording into `path` (which must be inside `project_dir` and not exist yet) at
    /// `sample_rate`; `fps` gives the timecode its frames. `setting` is the computer setting
    /// naming the microphone (`voice-input.json` beside the other settings of this computer).
    pub fn start(path: PathBuf, project_dir: &Path, sample_rate: u32, fps: f64, setting: &Path) -> Result<Self> {
        inside(project_dir, &path)?;
        let device_name = chosen_device(setting)?;
        let stop = Arc::new(AtomicBool::new(false));
        let peak = Arc::new(AtomicU32::new(0));
        let samples = Arc::new(AtomicU32::new(0));
        let (ready_send, ready) = mpsc::channel();
        let (stop_t, peak_t, samples_t) = (stop.clone(), peak.clone(), samples.clone());
        let thread = std::thread::Builder::new()
            .name("qnc-voice-record".into())
            .spawn(move || record(path, device_name, sample_rate, fps, (stop_t, peak_t, samples_t), ready_send))
            .map_err(|e| e.to_string())?;
        // The stream is opened in its thread (it does not leave it); its answer says it runs.
        match ready.recv_timeout(Duration::from_secs(5)) {
            Ok(Ok(())) => Ok(Self { stop, peak, samples, rate: sample_rate, thread: Some(thread) }),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            Err(_) => {
                stop.store(true, Ordering::SeqCst);
                Err(qnc_ui_text::text("voice.no_answer").into())
            }
        }
    }

    /// The loudest sample since the last call (0..1), for a level meter.
    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }

    /// Seconds recorded so far.
    pub fn seconds(&self) -> f64 {
        self.samples.load(Ordering::Relaxed) as f64 / self.rate.max(1) as f64
    }

    /// Stops and finishes the file.
    pub fn stop(mut self) -> Result<Recording> {
        self.stop.store(true, Ordering::SeqCst);
        self.thread.take().map_or_else(|| Err(qnc_ui_text::text("voice.no_answer").into()), |thread| thread.join().unwrap_or_else(|_| Err(qnc_ui_text::text("voice.no_answer").into())))
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// The microphone named in the computer setting, if any.
fn chosen_device(file: &Path) -> Result<Option<String>> {
    let Ok(text) = std::fs::read_to_string(file) else { return Ok(None) };
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", file.display()))?;
    Ok(value.get("device").and_then(serde_json::Value::as_str).filter(|name| !name.is_empty()).map(str::to_string))
}

fn inside(project_dir: &Path, path: &Path) -> Result<()> {
    let plain = path.components().all(|part| !matches!(part, std::path::Component::ParentDir));
    if plain && path.starts_with(project_dir) {
        Ok(())
    } else {
        Err(qnc_ui_text::text("import.writing_allowed_only_into_project_folder").into())
    }
}

/// `HH:MM:SS:FF` of the local time of day at `fps`.
pub fn time_of_day_timecode(time: chrono::NaiveTime, fps: f64) -> String {
    use chrono::Timelike;
    let frames = ((time.nanosecond() as f64 / 1e9) * fps).floor() as u64;
    format!("{:02}:{:02}:{:02}:{:02}", time.hour(), time.minute(), time.second(), frames.min(fps.ceil() as u64 - 1))
}

type Shared = (Arc<AtomicBool>, Arc<AtomicU32>, Arc<AtomicU32>);

fn record(path: PathBuf, device_name: Option<String>, sample_rate: u32, fps: f64, (stop, peak, counted): Shared, ready: mpsc::Sender<Result<()>>) -> Result<Recording> {
    let fail = |error: String| {
        let _ = ready.send(Err(error.clone()));
        Err(error)
    };
    let host = cpal::default_host();
    let device = match &device_name {
        Some(name) => host.input_devices().ok().and_then(|mut devices| devices.find(|d| d.description().ok().is_some_and(|about| about.name() == name.as_str()))),
        None => host.default_input_device(),
    };
    let Some(device) = device else {
        return fail(match device_name {
            Some(name) => qnc_ui_text::fill("voice.no_named_device", &[("name", &name)]),
            None => qnc_ui_text::text("voice.no_device").into(),
        });
    };
    // A configuration at the project's rate, f32 or i16; never another rate.
    let config = device.supported_input_configs().ok().and_then(|mut configs| {
        configs.find_map(|range| {
            let format = range.sample_format();
            (matches!(format, cpal::SampleFormat::F32 | cpal::SampleFormat::I16)).then(|| range.try_with_sample_rate(sample_rate)).flatten()
        })
    });
    let Some(config) = config else {
        return fail(qnc_ui_text::fill("voice.rate", &[("rate", &sample_rate.to_string())]));
    };
    let channels = config.channels().max(1) as usize;
    let format = config.sample_format();
    let stream_config = config.config();
    let file = match File::create_new(&path) {
        Ok(file) => file,
        Err(error) => return fail(format!("{}: {error}", path.display())),
    };
    let mut wav = BufWriter::new(file);
    if let Err(error) = write_header(&mut wav, sample_rate, 0) {
        return fail(error);
    }
    let (send, samples) = mpsc::channel::<Vec<f32>>();
    let keep = move |data: &[f32]| {
        // The first channel (a microphone is on it).
        let mono: Vec<f32> = data.chunks(channels).map(|frame| frame[0]).collect();
        let _ = send.send(mono);
    };
    let error_fn = |error| {
        let _ = error;
    };
    let stream = match format {
        cpal::SampleFormat::F32 => {
            let keep = keep.clone();
            device.build_input_stream(&stream_config, move |data: &[f32], _: &cpal::InputCallbackInfo| keep(data), error_fn, None)
        }
        _ => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| keep(&data.iter().map(|s| *s as f32 / 32768.0).collect::<Vec<_>>()),
            error_fn,
            None,
        ),
    };
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => return fail(qnc_ui_text::fill("voice.stream", &[("error", &error.to_string())])),
    };
    if let Err(error) = stream.play() {
        return fail(qnc_ui_text::fill("voice.stream", &[("error", &error.to_string())]));
    }
    let started = chrono::Local::now().time();
    let _ = ready.send(Ok(()));
    let mut written: u64 = 0;
    let mut write_block = |block: Vec<f32>, written: &mut u64| -> Result<()> {
        let mut loudest = f32::from_bits(peak.load(Ordering::Relaxed));
        for sample in block {
            let clamped = sample.clamp(-1.0, 1.0);
            loudest = loudest.max(clamped.abs());
            let value = (clamped * 8_388_607.0).round() as i32;
            wav.write_all(&value.to_le_bytes()[..3]).map_err(|e| e.to_string())?;
            *written += 1;
        }
        peak.store(loudest.to_bits(), Ordering::Relaxed);
        counted.store((*written).min(u32::MAX as u64) as u32, Ordering::Relaxed);
        Ok(())
    };
    while !stop.load(Ordering::SeqCst) {
        if let Ok(block) = samples.recv_timeout(Duration::from_millis(50)) {
            write_block(block, &mut written)?;
        }
    }
    drop(stream);
    while let Ok(block) = samples.try_recv() {
        write_block(block, &mut written)?;
    }
    wav.flush().map_err(|e| e.to_string())?;
    let mut file = wav.into_inner().map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    write_header(&mut file, sample_rate, written)?;
    file.sync_all().map_err(|e| e.to_string())?;
    Ok(Recording { path, seconds: written as f64 / sample_rate as f64, timecode: time_of_day_timecode(started, fps) })
}

/// The header of a mono 24-bit PCM WAV file of `samples` samples.
fn write_header(out: &mut impl Write, sample_rate: u32, samples: u64) -> Result<()> {
    let data = (samples * 3).min(u32::MAX as u64 - 36) as u32;
    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&1u16.to_le_bytes()); // PCM
    header.extend_from_slice(&1u16.to_le_bytes()); // mono
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&(sample_rate * 3).to_le_bytes());
    header.extend_from_slice(&3u16.to_le_bytes());
    header.extend_from_slice(&24u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data.to_le_bytes());
    out.write_all(&header).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_timecode_is_the_time_of_day_at_the_projects_rate() {
        let time = chrono::NaiveTime::from_hms_milli_opt(18, 1, 2, 500).unwrap();
        assert_eq!(time_of_day_timecode(time, 50.0), "18:01:02:25");
        assert_eq!(time_of_day_timecode(time, 25.0), "18:01:02:12");
    }

    #[test]
    fn a_wav_header_says_mono_24_bit_and_its_length() {
        let mut out = Vec::new();
        write_header(&mut out, 48_000, 48_000).unwrap();
        assert_eq!(out.len(), 44);
        assert_eq!(&out[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(out[40..44].try_into().unwrap()), 144_000);
        assert_eq!(u16::from_le_bytes(out[34..36].try_into().unwrap()), 24);
    }

    #[test]
    fn a_file_outside_the_project_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("p");
        assert!(inside(&project, &project.join("products/voice/a.wav")).is_ok());
        assert!(inside(&project, &dir.path().join("x.wav")).is_err());
        assert!(inside(&project, &project.join("../x.wav")).is_err());
    }
}

/// The local date and time for a file name (`YYYYMMDD_HHMMSS`).
pub fn file_stamp() -> String {
    chrono::Local::now().format("%Y%m%d_%H%M%S").to_string()
}

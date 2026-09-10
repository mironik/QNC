use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Format {
    pub sample_rate_hz: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: String,
    pub format: Format,
    /// Private device-edge binding from deployment, not a media or filesystem identity.
    pub device_id: Option<String>,
    pub capacity_frames: u32,
    pub ready_frames: u32,
}

impl Config {
    pub(crate) fn validate(&self) -> Result<usize> {
        let f = self.format;
        if self.version != crate::VERSION
            || !(8_000..=384_000).contains(&f.sample_rate_hz)
            || !(1..=64).contains(&f.channels)
            || self.ready_frames == 0
            || self.ready_frames > self.capacity_frames
            || self.capacity_frames > f.sample_rate_hz.saturating_mul(2)
            || self
                .device_id
                .as_ref()
                .is_some_and(|id| id.is_empty() || id.len() > 4096)
        {
            return Err(Error::new(Code::Contract, "invalid output configuration"));
        }
        let samples = self.capacity_frames as usize * f.channels as usize;
        if samples > 4 * 1024 * 1024 {
            return Err(Error::new(
                Code::Contract,
                "output queue exceeds memory limit",
            ));
        }
        Ok(samples)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Code {
    Contract,
    Unsupported,
    Device,
    NotReady,
    StaleGeneration,
    Full,
    Underrun,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    pub code: Code,
    pub message: String,
}
impl Error {
    pub(crate) fn new(code: Code, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "audio output {:?}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Paused,
    Preparing,
    Ready,
    Playing,
    Drained,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Telemetry {
    pub generation: u64,
    pub status: Status,
    /// Frames handed to the driver, NOT a claim of acoustic presentation.
    pub submitted_frames: u64,
    pub queued_frames: u64,
    pub start_to_first_callback_ns: Option<u64>,
    /// Driver-reported delay between callback and playback for that first buffer.
    pub first_driver_delay_ns: Option<u64>,
}

/// Observation of a driver buffer, not acoustic feedback or a playback clock.
#[derive(Debug, Clone, Copy)]
pub struct DriverTiming {
    pub generation: u64,
    pub first_sample_frame: u64,
    pub sample_rate_hz: u32,
    pub playback_unix_ns: u128,
}

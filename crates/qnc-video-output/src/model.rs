use serde::{Deserialize, Serialize};

pub const VERSION: &str = "0.1.0";
pub const MAX_POOL_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_OUTPUT_SLOTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    Rgba8Srgb,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputConfig {
    pub version: String,
    pub session_id: String,
    pub generation: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub slots: usize,
    pub pool_budget_bytes: u64,
}

impl OutputConfig {
    pub fn validate(&self) -> Result<(), OutputError> {
        if self.version != VERSION {
            return Err(OutputError::Version);
        }
        if !valid_id(&self.session_id) || self.generation == 0 {
            return Err(OutputError::Identity);
        }
        let size = pixel_bytes(self.width, self.height)?;
        if !(1..=MAX_OUTPUT_SLOTS).contains(&self.slots)
            || self.pool_budget_bytes > MAX_POOL_BYTES
            || size
                .checked_mul(self.slots as u64)
                .ok_or(OutputError::Budget)?
                > self.pool_budget_bytes
        {
            return Err(OutputError::Budget);
        }
        Ok(())
    }
}

/// Tightly packed, top-down, opaque RGBA8 sRGB; no media path or GPU handle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameHeader {
    pub version: String,
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub source_id: String,
    pub frame_number: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
}

impl FrameHeader {
    pub(crate) fn validate_geometry(&self, config: &OutputConfig) -> Result<(), OutputError> {
        if self.version != VERSION {
            return Err(OutputError::Version);
        }
        if self.session_id != config.session_id
            || self.generation != config.generation
            || !valid_id(&self.source_id)
        {
            return Err(OutputError::Identity);
        }
        if self.width != config.width
            || self.height != config.height
            || self.pixel_format != config.pixel_format
        {
            return Err(OutputError::Pixels);
        }
        Ok(())
    }

    pub(crate) fn validate(&self, config: &OutputConfig, bytes: usize) -> Result<(), OutputError> {
        self.validate_geometry(config)?;
        if u64::try_from(bytes).ok() != Some(pixel_bytes(self.width, self.height)?) {
            return Err(OutputError::Pixels);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmissionTarget {
    Surface,
    Offscreen,
}

/// Submission is not a display scanout acknowledgment.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission {
    pub version: String,
    pub id: u64,
    pub frame: FrameHeader,
    pub target: SubmissionTarget,
}

/// GPU completion is not a display scanout acknowledgment either.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GpuCompletion {
    pub submission: Submission,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutputError {
    Version,
    Identity,
    Dimensions,
    Budget,
    Pixels,
    Sequence,
    Full,
    StaleToken,
    NotReady,
    Busy,
    Suspended,
    UnsupportedSurface,
    Surface(String),
    Device(String),
}

impl std::fmt::Display for OutputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "video output: {self:?}")
    }
}

impl std::error::Error for OutputError {}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 512 && !id.chars().any(char::is_control)
}

pub(crate) fn pixel_bytes(width: u32, height: u32) -> Result<u64, OutputError> {
    if width == 0 || height == 0 {
        return Err(OutputError::Dimensions);
    }
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4))
        .ok_or(OutputError::Dimensions)
}

pub(crate) fn viewport(source: (u32, u32), target: (u32, u32)) -> (f32, f32, f32, f32) {
    let scale = (target.0 as f32 / source.0 as f32).min(target.1 as f32 / source.1 as f32);
    let width = source.0 as f32 * scale;
    let height = source.1 as f32 * scale;
    (
        (target.0 as f32 - width) * 0.5,
        (target.1 as f32 - height) * 0.5,
        width,
        height,
    )
}

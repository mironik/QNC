use qnc_media_metadata::{Fact, ScanMode, Signal, VideoMetadata};
use serde::{Deserialize, Serialize};

pub const VERSION: &str = "0.1.0";
pub(crate) const MAX_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelLayout {
    Yuv420p,
    Yuv422p,
    Yuv444p,
    Yuv420p10le,
    Yuv422p10le,
    Yuv444p10le,
}

impl PixelLayout {
    pub fn name(self) -> &'static str {
        match self {
            Self::Yuv420p => "yuv420p",
            Self::Yuv422p => "yuv422p",
            Self::Yuv444p => "yuv444p",
            Self::Yuv420p10le => "yuv420p10le",
            Self::Yuv422p10le => "yuv422p10le",
            Self::Yuv444p10le => "yuv444p10le",
        }
    }
    pub fn ten_bit(self) -> bool {
        matches!(
            self,
            Self::Yuv420p10le | Self::Yuv422p10le | Self::Yuv444p10le
        )
    }
    pub fn chroma_size(self, width: u32, height: u32) -> (u32, u32) {
        match self {
            Self::Yuv420p | Self::Yuv420p10le => (width.div_ceil(2), height.div_ceil(2)),
            Self::Yuv422p | Self::Yuv422p10le => (width.div_ceil(2), height),
            _ => (width, height),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Range {
    Limited,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transfer {
    Bt709,
    Srgb,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConversionSpec {
    pub version: String,
    pub width: u32,
    pub height: u32,
    pub layout: PixelLayout,
    pub primaries: String,
    pub matrix: String,
    pub scan_mode: ScanMode,
    pub range: Range,
    pub transfer: Transfer,
}

impl ConversionSpec {
    /// The saved evidence is interpreted here; absent/unspecified is not a default.
    pub fn from_saved(video: &VideoMetadata) -> Result<Self, ConversionError> {
        let layout = match value(&video.pixel_format, "pixel_format")?.as_str() {
            "yuv420p" => PixelLayout::Yuv420p,
            "yuv422p" => PixelLayout::Yuv422p,
            "yuv444p" => PixelLayout::Yuv444p,
            "yuv420p10le" => PixelLayout::Yuv420p10le,
            "yuv422p10le" => PixelLayout::Yuv422p10le,
            "yuv444p10le" => PixelLayout::Yuv444p10le,
            _ => return Err(ConversionError::Unsupported("pixel_format")),
        };
        let range = match signal(&video.color.range, "color.range")?.as_str() {
            "tv" | "limited" => Range::Limited,
            "pc" | "full" => Range::Full,
            _ => return Err(ConversionError::Unsupported("color.range")),
        };
        let transfer = match signal(&video.color.transfer, "color.transfer")?.as_str() {
            "bt709" => Transfer::Bt709,
            "iec61966-2-1" | "srgb" => Transfer::Srgb,
            _ => return Err(ConversionError::Unsupported("color.transfer")),
        };
        let spec = Self {
            version: VERSION.into(),
            width: *value(&video.width, "width")?,
            height: *value(&video.height, "height")?,
            layout,
            primaries: signal(&video.color.primaries, "color.primaries")?.clone(),
            matrix: signal(&video.color.matrix, "color.matrix")?.clone(),
            scan_mode: *value(&video.scan_mode, "scan_mode")?,
            range,
            transfer,
        };
        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<(), ConversionError> {
        if self.version != VERSION {
            return Err(ConversionError::Version);
        }
        if self.primaries != "bt709" {
            return Err(ConversionError::Unsupported("color.primaries"));
        }
        if self.matrix != "bt709" {
            return Err(ConversionError::Unsupported("color.matrix"));
        }
        if self.scan_mode != ScanMode::Progressive {
            return Err(ConversionError::Unsupported("scan_mode"));
        }
        self.sizes().map(|_| ())
    }

    pub fn input_bytes(&self) -> Result<usize, ConversionError> {
        Ok(self.sizes()?.0)
    }

    pub fn validate_payload(&self, input: &[u8]) -> Result<(), ConversionError> {
        self.validate()?;
        if input.len() != self.input_bytes()? {
            return Err(ConversionError::Payload);
        }
        if self.layout.ten_bit()
            && input
                .chunks_exact(2)
                .any(|v| u16::from_le_bytes([v[0], v[1]]) > 1023)
        {
            return Err(ConversionError::BitDepth);
        }
        Ok(())
    }
    pub fn output_bytes(&self) -> Result<usize, ConversionError> {
        Ok(self.sizes()?.1)
    }
    pub fn scratch_bytes(&self) -> Result<usize, ConversionError> {
        let (input, output) = self.sizes()?;
        let bytes = if self.layout.ten_bit() {
            input + output * 2 + 1024
        } else {
            1024
        };
        if bytes > MAX_BYTES {
            return Err(ConversionError::Budget);
        }
        Ok(bytes)
    }

    fn sizes(&self) -> Result<(usize, usize), ConversionError> {
        if self.width == 0 || self.height == 0 || self.width > 16_384 || self.height > 16_384 {
            return Err(ConversionError::Dimensions);
        }
        let (cw, ch) = self.layout.chroma_size(self.width, self.height);
        let pixels = u64::from(self.width) * u64::from(self.height);
        let samples = pixels + 2 * u64::from(cw) * u64::from(ch);
        let input = samples * if self.layout.ten_bit() { 2 } else { 1 };
        let output = pixels * 4;
        if input > MAX_BYTES as u64 || output > MAX_BYTES as u64 {
            return Err(ConversionError::Budget);
        }
        Ok((input as usize, output as usize))
    }
}

fn value<'a, T>(fact: &'a Option<Fact<T>>, field: &'static str) -> Result<&'a T, ConversionError> {
    fact.as_ref()
        .map(|f| &f.value)
        .ok_or(ConversionError::Missing(field))
}
fn signal<'a, T>(
    fact: &'a Option<Fact<Signal<T>>>,
    field: &'static str,
) -> Result<&'a T, ConversionError> {
    match value(fact, field)? {
        Signal::Known(value) => Ok(value),
        Signal::Unspecified => Err(ConversionError::Missing(field)),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConversionError {
    Version,
    Missing(&'static str),
    Unsupported(&'static str),
    Dimensions,
    Budget,
    Payload,
    BitDepth,
    Library(String),
}
impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pixel conversion: {self:?}")
    }
}
impl std::error::Error for ConversionError {}

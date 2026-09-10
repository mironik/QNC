use qnc_media_metadata::{MediaRepresentation, Rational};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodeRequest {
    pub version: String,
    pub media: MediaRepresentation,
    pub stream_index: u32,
    /// Relative input timestamp, NOT a promised source frame index. None starts at the beginning.
    pub start: Option<Rational>,
}

#[derive(Debug, Clone)]
pub struct DecoderConfig {
    pub adapter: Arc<dyn crate::DecoderAdapter>,
    pub queued_packets: usize,
    pub memory_budget_bytes: usize,
    pub read_timeout: Duration,
}
impl DecoderConfig {
    pub fn new(adapter: impl crate::DecoderAdapter + 'static) -> Self {
        Self {
            adapter: Arc::new(adapter),
            queued_packets: 8,
            memory_budget_bytes: 512 * 1024 * 1024,
            read_timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecodedFormat {
    Video {
        width: u32,
        height: u32,
        pixel_format: String,
    },
    Audio {
        sample_rate_hz: u32,
        channels: u32,
        sample_format: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedPacket {
    pub version: String,
    pub media_uri: String,
    pub stream_index: u32,
    /// Decode-session output ordinal; reset after opening a new decode session/seek.
    pub ordinal: u64,
    pub pts: i64,
    pub time_base: Rational,
    pub format: DecodedFormat,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Contract,
    Unsupported,
    Process,
    Stream,
    Timeout,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodeError {
    pub kind: ErrorKind,
    pub message: String,
}
impl DecodeError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "decode {:?}: {}", self.kind, self.message)
    }
}
impl std::error::Error for DecodeError {}
pub type Result<T> = std::result::Result<T, DecodeError>;

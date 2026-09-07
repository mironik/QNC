use std::collections::BTreeMap;

use qnc_frame_timebase::FrameTimebase;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipMetadata {
    pub contract_id: String,
    pub contract_version: String,
    pub clip_id: String,
    pub evidence: Vec<Evidence>,
    pub original: MediaRepresentation,
    pub proxy: Option<MediaRepresentation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub id: String,
    pub kind: EvidenceKind,
    pub document_uri: String,
    pub media_uri: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    CameraMetadata,
    Ffprobe,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fact<T> {
    pub value: T,
    pub evidence_id: String,
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Signal<T> {
    Known(T),
    Unspecified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rational {
    pub numerator: i64,
    pub denominator: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaRepresentation {
    pub media_uri: String,
    pub container: Option<Fact<String>>,
    pub duration_seconds: Option<Fact<Rational>>,
    pub streams_complete: Option<Fact<bool>>,
    pub streams: Vec<MediaStream>,
    pub tags: BTreeMap<String, Fact<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaStream {
    pub index: Option<Fact<u32>>,
    pub codec: Option<Fact<Signal<String>>>,
    pub profile: Option<Fact<String>>,
    pub time_base: Option<Fact<Rational>>,
    pub start_pts: Option<Fact<i64>>,
    pub duration_ts: Option<Fact<i64>>,
    pub details: StreamDetails,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "metadata",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamDetails {
    Video(Box<VideoMetadata>),
    Audio(Box<AudioMetadata>),
    Other { stream_type: Fact<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoMetadata {
    pub width: Option<Fact<u32>>,
    pub height: Option<Fact<u32>>,
    pub frame_rate: Option<Fact<FrameTimebase>>,
    pub frame_rate_mode: Option<Fact<FrameRateMode>>,
    pub frame_count: Option<Fact<FrameCount>>,
    pub scan_mode: Option<Fact<ScanMode>>,
    pub pixel_format: Option<Fact<String>>,
    pub sample_aspect_ratio: Option<Fact<Rational>>,
    pub rotation_degrees: Option<Fact<Signal<i32>>>,
    pub color: ColorMetadata,
}

impl VideoMetadata {
    /// An estimated count must never silently become a frame-accurate boundary.
    pub fn exact_frame_count(&self) -> Option<u64> {
        match self.frame_count.as_ref()?.value {
            FrameCount::Exact(count) if count > 0 && count <= i64::MAX as u64 => Some(count),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "accuracy",
    content = "frames",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum FrameCount {
    Exact(u64),
    Estimated(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameRateMode {
    Constant,
    Variable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMode {
    Progressive,
    InterlacedTopFieldFirst,
    InterlacedBottomFieldFirst,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorMetadata {
    pub primaries: Option<Fact<Signal<String>>>,
    pub transfer: Option<Fact<Signal<String>>>,
    pub matrix: Option<Fact<Signal<String>>>,
    pub range: Option<Fact<Signal<String>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioMetadata {
    pub sample_rate_hz: Option<Fact<u32>>,
    pub channels: Option<Fact<u32>>,
    pub sample_format: Option<Fact<Signal<String>>>,
    pub channel_layout: Option<Fact<Signal<String>>>,
    pub bits_per_sample: Option<Fact<u16>>,
}

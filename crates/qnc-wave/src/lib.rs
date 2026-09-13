//! Shared waveform model.
//!
//! This crate owns no UI, database, scanner, probe, playback clock or form
//! state. It builds deterministic audio peak plans from already stored clip
//! metadata and validates peak artifacts for passive timeline painting.

use qnc_media_metadata::{AudioMetadata, MediaRepresentation, Rational, StreamDetails};
use qnc_media_records::{Phase, Snapshot};
use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.wave";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const WAVE_RENDER_VERSION: i64 = 1;
pub const WAVE_PEAK_BUCKETS: usize = 1200;
pub const MAX_WAVE_PEAK_BUCKETS: usize = 9600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveLane {
    A1,
    A2,
    A3,
    A4,
}

impl WaveLane {
    fn from_zero_based(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::A1),
            1 => Some(Self::A2),
            2 => Some(Self::A3),
            3 => Some(Self::A4),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WaveSourceKind {
    Original,
    Proxy,
}

impl WaveSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Proxy => "proxy",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveChannel {
    pub lane: WaveLane,
    pub stream_index: u32,
    pub channel_index: u32,
    pub source_channels: u32,
    pub sample_rate_hz: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WavePlan {
    pub clip_id: String,
    pub source_kind: WaveSourceKind,
    pub source_uri: String,
    pub artifact_uri: String,
    pub duration_seconds: f64,
    pub duration_sample_frames: u64,
    pub source_sample_rate_hz: u32,
    pub peak_count: usize,
    pub channels: Vec<WaveChannel>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveformPeaks {
    pub clip_id: String,
    pub a1_peaks: Vec<f32>,
    pub a2_peaks: Vec<f32>,
    pub a3_peaks: Vec<f32>,
    pub a4_peaks: Vec<f32>,
}

impl WaveformPeaks {
    pub fn is_empty(&self) -> bool {
        self.a1_peaks.is_empty()
            && self.a2_peaks.is_empty()
            && self.a3_peaks.is_empty()
            && self.a4_peaks.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaveArtifactRecord {
    pub clip_id: String,
    pub status: String,
    pub artifact_uri: String,
    pub source_uri: String,
    pub source_sample_rate_hz: u32,
    pub peak_count: usize,
    pub a1_peaks: Vec<f32>,
    #[serde(default)]
    pub a2_peaks: Vec<f32>,
    #[serde(default)]
    pub a3_peaks: Vec<f32>,
    #[serde(default)]
    pub a4_peaks: Vec<f32>,
    pub warning: Option<String>,
    pub render_version: i64,
}

impl WaveArtifactRecord {
    pub fn peaks(&self) -> Option<WaveformPeaks> {
        (self.status == "ready").then(|| WaveformPeaks {
            clip_id: self.clip_id.clone(),
            a1_peaks: self.a1_peaks.clone(),
            a2_peaks: self.a2_peaks.clone(),
            a3_peaks: self.a3_peaks.clone(),
            a4_peaks: self.a4_peaks.clone(),
        })
    }
}

pub fn plan_from_snapshot(snapshot: &Snapshot, wave_base_uri: &str) -> Result<WavePlan, String> {
    plan_from_snapshot_with_artifact_name(snapshot, wave_base_uri, None)
}

pub fn plan_from_snapshot_with_artifact_name(
    snapshot: &Snapshot,
    wave_base_uri: &str,
    artifact_name: Option<&str>,
) -> Result<WavePlan, String> {
    plan_from_snapshot_inner(
        snapshot,
        wave_base_uri,
        artifact_name,
        WaveSourceKind::Original,
    )
}

pub fn plan_from_snapshot_with_project_audio_channels(
    snapshot: &Snapshot,
    wave_base_uri: &str,
    artifact_name: Option<&str>,
    project_audio_channels: u16,
) -> Result<WavePlan, String> {
    let proxy_has_audio = snapshot
        .metadata
        .proxy
        .as_ref()
        .map(|proxy| audio_channel_count(proxy))
        .transpose()?
        .is_some_and(|channels| channels > 0);
    let source_kind = if project_audio_channels == 2 && proxy_has_audio {
        WaveSourceKind::Proxy
    } else {
        WaveSourceKind::Original
    };
    plan_from_snapshot_inner(snapshot, wave_base_uri, artifact_name, source_kind)
}

pub fn media_for_plan<'a>(
    snapshot: &'a Snapshot,
    plan: &WavePlan,
) -> Result<&'a MediaRepresentation, String> {
    let media = match plan.source_kind {
        WaveSourceKind::Original => &snapshot.metadata.original,
        WaveSourceKind::Proxy => snapshot
            .metadata
            .proxy
            .as_ref()
            .ok_or_else(|| "Wave plan trazi proxy koji nije zapisan.".to_string())?,
    };
    if media.media_uri != plan.source_uri {
        return Err("Wave plan source ne odgovara spremljenom media zapisu.".into());
    }
    Ok(media)
}

fn plan_from_snapshot_inner(
    snapshot: &Snapshot,
    wave_base_uri: &str,
    artifact_name: Option<&str>,
    source_kind: WaveSourceKind,
) -> Result<WavePlan, String> {
    snapshot.validate().map_err(|error| error.to_string())?;
    if snapshot.phase != Phase::Final {
        return Err("Wave treba finalni spremljeni media zapis.".into());
    }
    let media =
        match source_kind {
            WaveSourceKind::Original => &snapshot.metadata.original,
            WaveSourceKind::Proxy => snapshot.metadata.proxy.as_ref().ok_or_else(|| {
                "Wave treba spremljeni proxy za projekt sa dva kanala.".to_string()
            })?,
        };
    let duration = media
        .duration_seconds
        .as_ref()
        .ok_or_else(|| "Wave treba spremljeno trajanje izvornog klipa.".to_string())?
        .value;
    let channels = audio_channels(media, source_kind)?;
    let source_sample_rate_hz = channels
        .first()
        .ok_or_else(|| "Wave treba najmanje jedan izvorni audio kanal.".to_string())?
        .sample_rate_hz;
    if channels
        .iter()
        .any(|channel| channel.sample_rate_hz != source_sample_rate_hz)
    {
        return Err(
            "Wave ne smije miješati izvorne audio kanale razlicitih sample rateova.".into(),
        );
    }
    let duration_seconds = rational_to_f64(duration)?;
    let duration_sample_frames = duration_to_samples(duration, source_sample_rate_hz)?;
    let root = wave_base_uri.trim_end_matches('/');
    qnc_media_records::validate_resource_uri(root).map_err(|_| "invalid wave root uri")?;
    let safe_clip = artifact_name
        .map(safe_name)
        .filter(|name| name != "clip")
        .unwrap_or_else(|| safe_name(&snapshot.metadata.clip_id));
    let artifact_uri = format!("{root}/{safe_clip}");
    qnc_media_records::validate_resource_uri(&artifact_uri)
        .map_err(|_| "invalid wave artifact uri")?;
    Ok(WavePlan {
        clip_id: snapshot.metadata.clip_id.clone(),
        source_kind,
        source_uri: media.media_uri.clone(),
        artifact_uri,
        duration_seconds,
        duration_sample_frames,
        source_sample_rate_hz,
        peak_count: WAVE_PEAK_BUCKETS,
        channels,
    })
}

pub fn artifact_record_from_peaks(
    plan: &WavePlan,
    a1_peaks: Vec<f32>,
    a2_peaks: Vec<f32>,
    warning: Option<String>,
) -> Result<WaveArtifactRecord, String> {
    artifact_record_from_peaks4(plan, a1_peaks, a2_peaks, Vec::new(), Vec::new(), warning)
}

pub fn artifact_record_from_lane_peaks(
    plan: &WavePlan,
    lane_peaks: Vec<(WaveLane, Vec<f32>)>,
    warning: Option<String>,
) -> Result<WaveArtifactRecord, String> {
    let mut a1_peaks = Vec::new();
    let mut a2_peaks = Vec::new();
    let mut a3_peaks = Vec::new();
    let mut a4_peaks = Vec::new();
    for (lane, peaks) in lane_peaks {
        let slot = match lane {
            WaveLane::A1 => &mut a1_peaks,
            WaveLane::A2 => &mut a2_peaks,
            WaveLane::A3 => &mut a3_peaks,
            WaveLane::A4 => &mut a4_peaks,
        };
        if !slot.is_empty() {
            return Err("Wave ima duplicirani lane zapis.".into());
        }
        *slot = peaks;
    }
    artifact_record_from_peaks4(plan, a1_peaks, a2_peaks, a3_peaks, a4_peaks, warning)
}

pub fn artifact_record_from_peaks4(
    plan: &WavePlan,
    a1_peaks: Vec<f32>,
    a2_peaks: Vec<f32>,
    a3_peaks: Vec<f32>,
    a4_peaks: Vec<f32>,
    warning: Option<String>,
) -> Result<WaveArtifactRecord, String> {
    let record = WaveArtifactRecord {
        clip_id: plan.clip_id.clone(),
        status: "ready".into(),
        artifact_uri: plan.artifact_uri.clone(),
        source_uri: plan.source_uri.clone(),
        source_sample_rate_hz: plan.source_sample_rate_hz,
        peak_count: plan.peak_count,
        a1_peaks,
        a2_peaks,
        a3_peaks,
        a4_peaks,
        warning,
        render_version: WAVE_RENDER_VERSION,
    };
    validate_artifact(&record)?;
    Ok(record)
}

pub fn validate_artifact(artifact: &WaveArtifactRecord) -> Result<(), String> {
    qnc_media_records::valid_id(&artifact.clip_id).map_err(|error| error.to_string())?;
    qnc_media_records::validate_resource_uri(&artifact.artifact_uri)
        .map_err(|_| "invalid wave artifact uri")?;
    qnc_media_records::validate_resource_uri(&artifact.source_uri)
        .map_err(|_| "invalid wave source uri")?;
    if !matches!(
        artifact.status.as_str(),
        "missing" | "building" | "ready" | "error"
    ) {
        return Err("Neispravan wave status.".into());
    }
    if artifact.render_version != WAVE_RENDER_VERSION {
        return Err("Nepodrzana wave render verzija.".into());
    }
    if !(8_000..=768_000).contains(&artifact.source_sample_rate_hz) {
        return Err("Neispravan wave sample rate.".into());
    }
    if artifact.peak_count > MAX_WAVE_PEAK_BUCKETS {
        return Err("Prevelik broj wave peakova.".into());
    }
    if artifact.status == "ready" {
        if artifact.peak_count == 0 || artifact.a1_peaks.len() != artifact.peak_count {
            return Err("Wave A1 nije spreman.".into());
        }
        for (lane, peaks) in [
            ("A2", &artifact.a2_peaks),
            ("A3", &artifact.a3_peaks),
            ("A4", &artifact.a4_peaks),
        ] {
            if !peaks.is_empty() && peaks.len() != artifact.peak_count {
                return Err(format!("Wave {lane} nema isti broj peakova."));
            }
        }
    }
    for peak in artifact
        .a1_peaks
        .iter()
        .chain(&artifact.a2_peaks)
        .chain(&artifact.a3_peaks)
        .chain(&artifact.a4_peaks)
    {
        if !peak.is_finite() || !(0.0..=1.0).contains(peak) {
            return Err("Wave peak je izvan valjanog raspona.".into());
        }
    }
    if artifact
        .warning
        .as_ref()
        .is_some_and(|text| text.len() > 4096)
    {
        return Err("Wave upozorenje je predugo.".into());
    }
    Ok(())
}

#[derive(Debug)]
pub struct StreamPeakCollector {
    source_channels: u32,
    selected: Vec<(WaveLane, u32, ChannelPeakCollector)>,
    seen_sample_frames: u64,
}

impl StreamPeakCollector {
    pub fn new(plan: &WavePlan, stream_index: u32) -> Result<Self, String> {
        let selected = plan
            .channels
            .iter()
            .filter(|channel| channel.stream_index == stream_index)
            .map(|channel| {
                if channel.channel_index >= channel.source_channels {
                    return Err("Wave kanal je izvan izvornog streama.".to_string());
                }
                Ok((
                    channel.lane,
                    channel.channel_index,
                    ChannelPeakCollector::new(plan.duration_sample_frames, plan.peak_count),
                ))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let source_channels = plan
            .channels
            .iter()
            .find(|channel| channel.stream_index == stream_index)
            .map(|channel| channel.source_channels)
            .ok_or_else(|| "Wave stream nije u planu.".to_string())?;
        if selected.is_empty() {
            return Err("Wave stream nema odabrane kanale.".into());
        }
        Ok(Self {
            source_channels,
            selected,
            seen_sample_frames: 0,
        })
    }

    pub fn push_f32le_interleaved(&mut self, bytes: &[u8]) -> Result<(), String> {
        let first = self.seen_sample_frames;
        self.push_f32le_interleaved_at(first, bytes)?;
        self.seen_sample_frames = self
            .seen_sample_frames
            .saturating_add(sample_frame_count(bytes.len(), self.source_channels)?);
        Ok(())
    }

    pub fn push_f32le_interleaved_at(
        &mut self,
        first_sample_frame: u64,
        bytes: &[u8],
    ) -> Result<(), String> {
        let stride = self.source_channels as usize * 4;
        if stride == 0 || !bytes.len().is_multiple_of(stride) {
            return Err("Wave PCM paket nije poravnat na izvorne kanale.".into());
        }
        for (offset_frame, frame) in bytes.chunks_exact(stride).enumerate() {
            let sample_frame = first_sample_frame.saturating_add(offset_frame as u64);
            for (.., channel_index, collector) in &mut self.selected {
                let offset = *channel_index as usize * 4;
                let sample = f32::from_le_bytes(
                    frame[offset..offset + 4]
                        .try_into()
                        .map_err(|_| "Wave PCM uzorak nije potpun.".to_string())?,
                );
                collector.push_at(sample_frame, sample)?;
            }
        }
        Ok(())
    }

    pub fn finish(self) -> Vec<(WaveLane, Vec<f32>)> {
        self.selected
            .into_iter()
            .map(|(lane, _, collector)| (lane, collector.finish()))
            .collect()
    }
}

pub fn relative_sample_position(
    pts: i64,
    tb: Rational,
    origin: (i64, Rational),
    sample_rate_hz: u32,
) -> Result<u64, String> {
    relative_position(pts, tb, origin, sample_rate_hz.into(), 1)
}

fn relative_position(
    pts: i64,
    tb: Rational,
    origin: (i64, Rational),
    num: i64,
    den: i64,
) -> Result<u64, String> {
    let mul = |a: i128, b: i128| {
        a.checked_mul(b)
            .ok_or_else(|| "Wave timestamp overflow.".to_string())
    };
    if tb.numerator <= 0
        || tb.denominator <= 0
        || origin.1.numerator <= 0
        || origin.1.denominator <= 0
        || num <= 0
        || den <= 0
    {
        return Err("Wave timestamp scale nije valjan.".into());
    }
    let a = mul(
        mul(pts.into(), tb.numerator.into())?,
        origin.1.denominator.into(),
    )?;
    let b = mul(
        mul(origin.0.into(), origin.1.numerator.into())?,
        tb.denominator.into(),
    )?;
    let n = mul(
        a.checked_sub(b)
            .ok_or_else(|| "Wave timestamp overflow.".to_string())?,
        num.into(),
    )?;
    let d = mul(
        mul(tb.denominator.into(), origin.1.denominator.into())?,
        den.into(),
    )?;
    if n < 0 || n % d != 0 {
        return Err("Wave timestamp nije na spremljenoj izvornoj mrezi.".into());
    }
    u64::try_from(n / d).map_err(|error| error.to_string())
}

fn sample_frame_count(byte_len: usize, source_channels: u32) -> Result<u64, String> {
    let stride = source_channels as usize * 4;
    if stride == 0 || !byte_len.is_multiple_of(stride) {
        return Err("Wave PCM paket nije poravnat na izvorne kanale.".into());
    }
    Ok((byte_len / stride) as u64)
}

#[derive(Debug)]
struct ChannelPeakCollector {
    expected_sample_frames: u64,
    peaks: Vec<f32>,
}

impl ChannelPeakCollector {
    fn new(expected_sample_frames: u64, peak_count: usize) -> Self {
        Self {
            expected_sample_frames,
            peaks: vec![0.0; peak_count],
        }
    }

    fn push_at(&mut self, sample_frame: u64, sample: f32) -> Result<(), String> {
        if !sample.is_finite() || self.peaks.is_empty() {
            return Err("Wave PCM uzorak nije valjan.".into());
        }
        let bucket = if self.expected_sample_frames == 0 {
            0
        } else {
            let bucket = sample_frame as u128 * self.peaks.len() as u128
                / self.expected_sample_frames as u128;
            bucket.min((self.peaks.len() - 1) as u128) as usize
        };
        self.peaks[bucket] = self.peaks[bucket].max(sample.abs().clamp(0.0, 1.0));
        Ok(())
    }

    fn finish(self) -> Vec<f32> {
        self.peaks
    }
}

fn audio_channel_count(media: &MediaRepresentation) -> Result<u32, String> {
    let mut count = 0u32;
    for stream in &media.streams {
        let StreamDetails::Audio(audio) = &stream.details else {
            continue;
        };
        let (_, source_channels) = audio_format(audio)?;
        count = count
            .checked_add(source_channels)
            .ok_or_else(|| "Wave audio channel count overflow.".to_string())?;
    }
    Ok(count)
}

fn audio_channels(
    media: &MediaRepresentation,
    _source_kind: WaveSourceKind,
) -> Result<Vec<WaveChannel>, String> {
    let mut streams = media.streams.iter().collect::<Vec<_>>();
    streams.sort_by_key(|stream| stream.index.as_ref().map(|index| index.value));
    let mut channels = Vec::new();
    for stream in streams {
        let StreamDetails::Audio(audio) = &stream.details else {
            continue;
        };
        let index = stream
            .index
            .as_ref()
            .ok_or_else(|| "Wave audio stream nema index.".to_string())?
            .value;
        let (sample_rate_hz, source_channels) = audio_format(audio)?;
        for channel_index in 0..source_channels {
            let Some(lane) = WaveLane::from_zero_based(channels.len()) else {
                return Ok(channels);
            };
            channels.push(WaveChannel {
                lane,
                stream_index: index,
                channel_index,
                source_channels,
                sample_rate_hz,
            });
        }
    }
    Ok(channels)
}

fn audio_format(audio: &AudioMetadata) -> Result<(u32, u32), String> {
    let sample_rate_hz = audio
        .sample_rate_hz
        .as_ref()
        .ok_or_else(|| "Wave audio stream nema sample rate.".to_string())?
        .value;
    let channels = audio
        .channels
        .as_ref()
        .ok_or_else(|| "Wave audio stream nema broj kanala.".to_string())?
        .value;
    if channels == 0 || !(8_000..=768_000).contains(&sample_rate_hz) {
        return Err("Wave audio stream ima nepodrzan format.".into());
    }
    Ok((sample_rate_hz, channels))
}

fn duration_to_samples(duration: Rational, sample_rate_hz: u32) -> Result<u64, String> {
    if duration.numerator <= 0 || duration.denominator <= 0 {
        return Err("Wave trajanje nije valjano.".into());
    }
    let numerator = i128::from(duration.numerator) * i128::from(sample_rate_hz);
    let denominator = i128::from(duration.denominator);
    let samples = (numerator + denominator - 1) / denominator;
    u64::try_from(samples.max(1)).map_err(|error| error.to_string())
}

fn rational_to_f64(value: Rational) -> Result<f64, String> {
    if value.numerator <= 0 || value.denominator <= 0 {
        return Err("Wave trajanje nije valjano.".into());
    }
    Ok(value.numerator as f64 / value.denominator as f64)
}

fn safe_name(value: &str) -> String {
    let mut out = value
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    if out.len() > 120 {
        out.truncate(120);
    }
    if out.is_empty() || matches!(out.as_str(), "." | "..") {
        "clip".into()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_media_metadata::{
        ClipMetadata, ColorMetadata, Evidence, EvidenceKind, Fact, FrameCount, FrameRateMode,
        MediaStream, ScanMode, Signal, VideoMetadata,
    };
    use qnc_media_records::{Binding, Completeness};

    fn fact<T>(value: T) -> Option<Fact<T>> {
        fact_for("camera", value)
    }

    fn fact_for<T>(evidence_id: &str, value: T) -> Option<Fact<T>> {
        Some(Fact {
            value,
            evidence_id: evidence_id.into(),
            locator: "test".into(),
        })
    }

    fn audio_stream(index: u32, channels: u32) -> MediaStream {
        MediaStream {
            index: fact(index),
            codec: fact(Signal::Known("pcm_s24le".into())),
            profile: None,
            time_base: fact(Rational {
                numerator: 1,
                denominator: 48_000,
            }),
            start_pts: fact(0),
            duration_ts: fact(480_000),
            details: StreamDetails::Audio(Box::new(AudioMetadata {
                sample_rate_hz: fact(48_000),
                channels: fact(channels),
                sample_format: fact(Signal::Known("s32".into())),
                channel_layout: fact(Signal::Unspecified),
                bits_per_sample: fact(24),
            })),
        }
    }

    fn video_stream() -> MediaStream {
        MediaStream {
            index: fact(0),
            codec: fact(Signal::Known("mpeg2video".into())),
            profile: None,
            time_base: fact(Rational {
                numerator: 1,
                denominator: 50,
            }),
            start_pts: fact(0),
            duration_ts: fact(500),
            details: StreamDetails::Video(Box::new(VideoMetadata {
                width: fact(1920),
                height: fact(1080),
                frame_rate: fact(qnc_media_metadata::FrameTimebase::new(50, 1).unwrap()),
                frame_rate_mode: fact(FrameRateMode::Constant),
                frame_count: fact(FrameCount::Exact(500)),
                scan_mode: fact(ScanMode::Progressive),
                pixel_format: fact("yuv422p".into()),
                sample_aspect_ratio: fact(Rational {
                    numerator: 1,
                    denominator: 1,
                }),
                rotation_degrees: fact(Signal::Known(0)),
                color: ColorMetadata {
                    primaries: fact(Signal::Known("bt709".into())),
                    transfer: fact(Signal::Known("bt709".into())),
                    matrix: fact(Signal::Known("bt709".into())),
                    range: fact(Signal::Known("tv".into())),
                },
            })),
        }
    }

    fn proxy_audio_stream(index: u32, channels: u32) -> MediaStream {
        MediaStream {
            index: fact_for("proxy", index),
            codec: fact_for("proxy", Signal::Known("aac".into())),
            profile: None,
            time_base: fact_for(
                "proxy",
                Rational {
                    numerator: 1,
                    denominator: 48_000,
                },
            ),
            start_pts: fact_for("proxy", 0),
            duration_ts: fact_for("proxy", 480_000),
            details: StreamDetails::Audio(Box::new(AudioMetadata {
                sample_rate_hz: fact_for("proxy", 48_000),
                channels: fact_for("proxy", channels),
                sample_format: fact_for("proxy", Signal::Known("fltp".into())),
                channel_layout: fact_for("proxy", Signal::Unspecified),
                bits_per_sample: fact_for("proxy", 16),
            })),
        }
    }

    fn proxy_video_stream() -> MediaStream {
        MediaStream {
            index: fact_for("proxy", 0),
            codec: fact_for("proxy", Signal::Known("h264".into())),
            profile: None,
            time_base: fact_for(
                "proxy",
                Rational {
                    numerator: 1,
                    denominator: 50,
                },
            ),
            start_pts: fact_for("proxy", 0),
            duration_ts: fact_for("proxy", 500),
            details: StreamDetails::Video(Box::new(VideoMetadata {
                width: fact_for("proxy", 1280),
                height: fact_for("proxy", 720),
                frame_rate: fact_for(
                    "proxy",
                    qnc_media_metadata::FrameTimebase::new(50, 1).unwrap(),
                ),
                frame_rate_mode: fact_for("proxy", FrameRateMode::Constant),
                frame_count: fact_for("proxy", FrameCount::Exact(500)),
                scan_mode: fact_for("proxy", ScanMode::Progressive),
                pixel_format: fact_for("proxy", "yuv420p".into()),
                sample_aspect_ratio: fact_for(
                    "proxy",
                    Rational {
                        numerator: 1,
                        denominator: 1,
                    },
                ),
                rotation_degrees: fact_for("proxy", Signal::Known(0)),
                color: ColorMetadata {
                    primaries: fact_for("proxy", Signal::Known("bt709".into())),
                    transfer: fact_for("proxy", Signal::Known("bt709".into())),
                    matrix: fact_for("proxy", Signal::Known("bt709".into())),
                    range: fact_for("proxy", Signal::Known("tv".into())),
                },
            })),
        }
    }

    fn snapshot(streams: Vec<MediaStream>) -> Snapshot {
        let original_uri = "qnc://local/source/card/file/PRIVATE/CLIP/C0001.MXF";
        let metadata = ClipMetadata {
            contract_id: qnc_media_metadata::CONTRACT_ID.into(),
            contract_version: qnc_media_metadata::CONTRACT_VERSION.into(),
            clip_id: "clip-c0001".into(),
            evidence: vec![Evidence {
                id: "camera".into(),
                kind: EvidenceKind::CameraMetadata,
                document_uri: "qnc://local/source/card/file/PRIVATE/CLIP/C0001.XML".into(),
                media_uri: original_uri.into(),
            }],
            original: MediaRepresentation {
                media_uri: original_uri.into(),
                container: fact("mxf".into()),
                duration_seconds: fact(Rational {
                    numerator: 10,
                    denominator: 1,
                }),
                streams_complete: fact(true),
                streams,
                tags: Default::default(),
            },
            proxy: None,
        };
        let report = qnc_media_metadata::inspect(&metadata);
        assert!(report.is_complete(), "{report:?}");
        Snapshot {
            binding: Binding {
                source_index_uri: "qnc://local/db/source_index".into(),
                source_record_id: "source-record-1".into(),
                original_uri: original_uri.into(),
                proxy_uri: None,
            },
            revision: 1,
            phase: Phase::Final,
            completeness: Completeness::Complete,
            metadata,
            report,
            recorded_at_unix_ms: 1,
        }
    }

    fn snapshot_with_proxy(original_audio_channels: u32, proxy_audio_channels: u32) -> Snapshot {
        let proxy_uri = "qnc://local/source/card/file/PRIVATE/SUB/C0001.MP4";
        let mut snapshot = snapshot(vec![
            video_stream(),
            audio_stream(1, original_audio_channels),
        ]);
        snapshot.metadata.evidence.push(Evidence {
            id: "proxy".into(),
            kind: EvidenceKind::CameraMetadata,
            document_uri: "qnc://local/source/card/file/PRIVATE/SUB/C0001.XML".into(),
            media_uri: proxy_uri.into(),
        });
        snapshot.metadata.proxy = Some(MediaRepresentation {
            media_uri: proxy_uri.into(),
            container: fact_for("proxy", "mov".into()),
            duration_seconds: fact_for(
                "proxy",
                Rational {
                    numerator: 10,
                    denominator: 1,
                },
            ),
            streams_complete: fact_for("proxy", true),
            streams: vec![
                proxy_video_stream(),
                proxy_audio_stream(1, proxy_audio_channels),
            ],
            tags: Default::default(),
        });
        snapshot.binding.proxy_uri = Some(proxy_uri.into());
        snapshot.report = qnc_media_metadata::inspect(&snapshot.metadata);
        assert!(snapshot.report.is_complete(), "{:?}", snapshot.report);
        snapshot.completeness = Completeness::Complete;
        snapshot
    }

    #[test]
    fn plan_uses_saved_source_channels_as_mono_lanes() {
        let plan = plan_from_snapshot(
            &snapshot(vec![video_stream(), audio_stream(1, 1), audio_stream(2, 1)]),
            "qnc://local/db/ingest_content/p1/wave",
        )
        .unwrap();

        assert_eq!(plan.source_sample_rate_hz, 48_000);
        assert_eq!(plan.duration_sample_frames, 480_000);
        assert_eq!(
            plan.channels,
            vec![
                WaveChannel {
                    lane: WaveLane::A1,
                    stream_index: 1,
                    channel_index: 0,
                    source_channels: 1,
                    sample_rate_hz: 48_000,
                },
                WaveChannel {
                    lane: WaveLane::A2,
                    stream_index: 2,
                    channel_index: 0,
                    source_channels: 1,
                    sample_rate_hz: 48_000,
                },
            ]
        );
    }

    #[test]
    fn multichannel_stream_maps_to_available_mono_source_lanes() {
        let plan = plan_from_snapshot(
            &snapshot(vec![video_stream(), audio_stream(1, 4)]),
            "qnc://local/db/ingest_content/p1/wave",
        )
        .unwrap();

        assert_eq!(plan.channels.len(), 4);
        assert_eq!(plan.channels[0].lane, WaveLane::A1);
        assert_eq!(plan.channels[0].channel_index, 0);
        assert_eq!(plan.channels[1].lane, WaveLane::A2);
        assert_eq!(plan.channels[1].channel_index, 1);
        assert_eq!(plan.channels[2].lane, WaveLane::A3);
        assert_eq!(plan.channels[2].channel_index, 2);
        assert_eq!(plan.channels[3].lane, WaveLane::A4);
        assert_eq!(plan.channels[3].channel_index, 3);
    }

    #[test]
    fn two_channel_project_uses_proxy_wave_when_proxy_exists() {
        let snapshot = snapshot_with_proxy(4, 1);

        let plan = plan_from_snapshot_with_project_audio_channels(
            &snapshot,
            "qnc://local/db/ingest_content/p1/wave",
            Some("C0001.MXF"),
            2,
        )
        .unwrap();

        assert_eq!(plan.source_kind, WaveSourceKind::Proxy);
        assert_eq!(
            plan.source_uri,
            "qnc://local/source/card/file/PRIVATE/SUB/C0001.MP4"
        );
        assert_eq!(plan.channels.len(), 1);
        assert_eq!(plan.channels[0].lane, WaveLane::A1);
        assert_eq!(
            media_for_plan(&snapshot, &plan).unwrap().media_uri,
            plan.source_uri
        );
    }

    #[test]
    fn more_than_two_channel_project_uses_original_wave_even_when_proxy_exists() {
        let snapshot = snapshot_with_proxy(4, 2);

        let plan = plan_from_snapshot_with_project_audio_channels(
            &snapshot,
            "qnc://local/db/ingest_content/p1/wave",
            Some("C0001.MXF"),
            4,
        )
        .unwrap();

        assert_eq!(plan.source_kind, WaveSourceKind::Original);
        assert_eq!(
            plan.source_uri,
            "qnc://local/source/card/file/PRIVATE/CLIP/C0001.MXF"
        );
        assert_eq!(plan.channels.len(), 4);
    }

    #[test]
    fn two_channel_project_without_proxy_uses_original_wave() {
        let snapshot = snapshot(vec![video_stream(), audio_stream(1, 2)]);

        let plan = plan_from_snapshot_with_project_audio_channels(
            &snapshot,
            "qnc://local/db/ingest_content/p1/wave",
            Some("C0001.MXF"),
            2,
        )
        .unwrap();

        assert_eq!(plan.source_kind, WaveSourceKind::Original);
    }

    #[test]
    fn collector_builds_normalized_peak_buckets_without_resampling() {
        let mut plan = plan_from_snapshot(
            &snapshot(vec![video_stream(), audio_stream(1, 2)]),
            "qnc://local/db/ingest_content/p1/wave",
        )
        .unwrap();
        plan.peak_count = 3;
        plan.duration_sample_frames = 6;
        let mut collector = StreamPeakCollector::new(&plan, 1).unwrap();
        let mut bytes = Vec::new();
        for (a, b) in [
            (0.1f32, 0.2f32),
            (-0.3, 0.4),
            (0.5, -0.6),
            (0.7, 0.1),
            (1.5, -0.8),
            (0.2, 0.9),
        ] {
            bytes.extend(a.to_le_bytes());
            bytes.extend(b.to_le_bytes());
        }

        collector.push_f32le_interleaved(&bytes).unwrap();
        let result = collector.finish();

        assert_eq!(result[0], (WaveLane::A1, vec![0.3, 0.7, 1.0]));
        assert_eq!(result[1], (WaveLane::A2, vec![0.4, 0.6, 0.9]));
    }

    #[test]
    fn ready_artifact_requires_a1_and_matching_optional_a2() {
        let plan = plan_from_snapshot(
            &snapshot(vec![video_stream(), audio_stream(1, 2)]),
            "qnc://local/db/ingest_content/p1/wave",
        )
        .unwrap();
        let record = artifact_record_from_peaks(
            &plan,
            vec![0.0; WAVE_PEAK_BUCKETS],
            vec![0.5; WAVE_PEAK_BUCKETS],
            None,
        )
        .unwrap();

        assert!(validate_artifact(&record).is_ok());
        let peaks = record.peaks().unwrap();
        assert_eq!(peaks.a1_peaks.len(), WAVE_PEAK_BUCKETS);
        assert_eq!(peaks.a2_peaks.len(), WAVE_PEAK_BUCKETS);
    }
}

//! Shared filmstrip model.
//!
//! This crate owns no UI, database, scanner, probe, playback clock, or form
//! state. It builds deterministic frame plans from already stored clip
//! metadata and loads already generated JPEG/PNG assets for passive painting.

use qnc_frame_timebase::FrameTimebase;
use qnc_image_assets::RgbaImage;
use qnc_media_metadata::{MediaRepresentation, Rational, StreamDetails, VideoMetadata};
use qnc_media_records::Snapshot;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const MODULE_ID: &str = "qnc.module.filmstrip";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FILMSTRIP_FRAME_COUNT: usize = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilmstripSourceKind {
    Original,
    Proxy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilmstripExtractionMode {
    RandomSeek,
    KeyframeSeek,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripFrameRequest {
    pub index: usize,
    pub seek_sec: f64,
    pub artifact_uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripPlan {
    pub clip_id: String,
    pub source_uri: String,
    pub source_kind: FilmstripSourceKind,
    pub duration_sec: f64,
    pub source_duration_frames: u64,
    pub source_timebase: FrameTimebase,
    pub artifact_root_uri: String,
    pub frames: Vec<FilmstripFrameRequest>,
    pub extraction_mode: FilmstripExtractionMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilmstripFrameAsset {
    pub index: usize,
    pub seek_sec: f64,
    pub uri: String,
    pub image: RgbaImage,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilmstripBackground {
    pub clip_id: String,
    pub frames: Vec<FilmstripFrameAsset>,
}

impl FilmstripBackground {
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripFrameRecord {
    pub index: usize,
    pub seek_sec: String,
    pub artifact_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripArtifactRecord {
    pub clip_id: String,
    pub status: String,
    pub duration_sec: String,
    pub frame_count: usize,
    pub artifact_uri: String,
    pub frames: Vec<FilmstripFrameRecord>,
}

pub fn plan_from_snapshot(
    snapshot: &Snapshot,
    filmstrip_root_uri: &str,
) -> Result<FilmstripPlan, String> {
    plan_from_snapshot_with_artifact_name(snapshot, filmstrip_root_uri, None)
}

pub fn plan_from_snapshot_with_artifact_name(
    snapshot: &Snapshot,
    filmstrip_root_uri: &str,
    artifact_name: Option<&str>,
) -> Result<FilmstripPlan, String> {
    snapshot.validate().map_err(|error| error.to_string())?;
    let metadata = &snapshot.metadata;
    let original_timing = representation_timing(&metadata.original)
        .ok_or_else(|| "filmstrip source frame metadata missing".to_string())?;
    let (source_kind, source) = if let Some(proxy) = &metadata.proxy {
        (FilmstripSourceKind::Proxy, proxy)
    } else {
        (FilmstripSourceKind::Original, &metadata.original)
    };
    let timing = representation_timing(source).unwrap_or(original_timing);
    let duration_sec = representation_duration_sec(source)
        .or_else(|| representation_duration_sec(&metadata.original))
        .unwrap_or_else(|| duration_from_frames(timing.frames, timing.timebase));
    let root = filmstrip_root_uri.trim_end_matches('/');
    qnc_media_records::validate_resource_uri(root).map_err(|_| "invalid filmstrip root uri")?;
    let safe_clip = artifact_name
        .map(safe_name)
        .filter(|name| name != "clip")
        .unwrap_or_else(|| safe_name(&metadata.clip_id));
    let artifact_root_uri = format!("{root}/{safe_clip}");
    qnc_media_records::validate_resource_uri(&artifact_root_uri)
        .map_err(|_| "invalid filmstrip artifact uri")?;

    let frames = (0..FILMSTRIP_FRAME_COUNT)
        .map(|index| {
            let source_frame =
                ((index as u128 * timing.frames as u128) / FILMSTRIP_FRAME_COUNT as u128) as u64;
            let seek_sec = duration_from_frames(source_frame, timing.timebase);
            let artifact_uri = format!("{artifact_root_uri}/{}", frame_file_name(index, seek_sec));
            qnc_media_records::validate_resource_uri(&artifact_uri)
                .map_err(|_| "invalid filmstrip frame uri")?;
            Ok(FilmstripFrameRequest {
                index,
                seek_sec,
                artifact_uri,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(FilmstripPlan {
        clip_id: metadata.clip_id.clone(),
        source_uri: source.media_uri.clone(),
        source_kind,
        duration_sec,
        source_duration_frames: timing.frames,
        source_timebase: timing.timebase,
        artifact_root_uri,
        frames,
        extraction_mode: extraction_mode(timing.frames, timing.timebase)?,
    })
}

pub fn extraction_mode(
    source_duration_frames: u64,
    source_timebase: FrameTimebase,
) -> Result<FilmstripExtractionMode, String> {
    if source_duration_frames == 0 {
        return Err("filmstrip source duration frames missing".into());
    }
    if source_timebase.fps_num <= 0 || source_timebase.fps_den <= 0 {
        return Err("filmstrip source timebase missing".into());
    }
    Ok(FilmstripExtractionMode::KeyframeSeek)
}

pub fn artifact_record_from_plan(plan: &FilmstripPlan, status: &str) -> FilmstripArtifactRecord {
    FilmstripArtifactRecord {
        clip_id: plan.clip_id.clone(),
        status: status.into(),
        duration_sec: format_seconds(plan.duration_sec),
        frame_count: plan.frames.len(),
        artifact_uri: plan.artifact_root_uri.clone(),
        frames: plan
            .frames
            .iter()
            .map(|frame| FilmstripFrameRecord {
                index: frame.index,
                seek_sec: format_seconds(frame.seek_sec),
                artifact_uri: frame.artifact_uri.clone(),
            })
            .collect(),
    }
}

pub fn safe_name(value: &str) -> String {
    let mut out: String = value
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.len() > 120 {
        out.truncate(120);
    }
    if out.is_empty() || matches!(out.as_str(), "." | "..") {
        "clip".into()
    } else {
        out
    }
}

pub fn frame_file_name(index: usize, seek_sec: f64) -> String {
    format!(
        "{index:03}_{}.jpg",
        format_seconds(seek_sec).replace('.', "_")
    )
}

pub fn format_seconds(value: f64) -> String {
    format!("{:.2}", value.max(0.0))
}

#[derive(Debug, Clone)]
pub struct LocalFilmstripArtifacts {
    root_uri: String,
    root_dir: PathBuf,
}

impl LocalFilmstripArtifacts {
    pub fn new(root_uri: impl Into<String>, root_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let root_uri = root_uri.into();
        qnc_media_records::validate_resource_uri(root_uri.trim_end_matches('/'))
            .map_err(|_| "invalid filmstrip root uri")?;
        Ok(Self {
            root_uri: root_uri.trim_end_matches('/').into(),
            root_dir: root_dir.into(),
        })
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn frame_path(&self, artifact_uri: &str) -> Result<PathBuf, String> {
        qnc_media_records::validate_resource_uri(artifact_uri)
            .map_err(|_| "invalid filmstrip frame uri")?;
        let relative = artifact_uri
            .strip_prefix(&format!("{}/", self.root_uri))
            .ok_or("filmstrip frame uri is outside artifact root")?;
        let mut path = self.root_dir.clone();
        for part in relative.split('/') {
            if part.is_empty()
                || matches!(part, "." | "..")
                || part.contains(['\\', ':', '?', '#'])
                || part.chars().any(char::is_control)
            {
                return Err("invalid filmstrip frame path".into());
            }
            path.push(part);
        }
        Ok(path)
    }

    pub fn output_paths(&self, plan: &FilmstripPlan) -> Result<Vec<PathBuf>, String> {
        plan.frames
            .iter()
            .map(|frame| self.frame_path(&frame.artifact_uri))
            .collect()
    }

    pub fn ready(&self, plan: &FilmstripPlan) -> bool {
        plan.frames.iter().all(|frame| {
            self.frame_path(&frame.artifact_uri)
                .ok()
                .is_some_and(|path| file_ready(&path))
        })
    }

    pub fn load_background(&self, plan: &FilmstripPlan) -> Result<FilmstripBackground, String> {
        let mut frames = Vec::with_capacity(plan.frames.len());
        for frame in &plan.frames {
            let path = self.frame_path(&frame.artifact_uri)?;
            let bytes =
                fs::read(&path).map_err(|error| format!("filmstrip frame read failed: {error}"))?;
            let image = qnc_image_assets::decode_thumbnail(&bytes)?;
            frames.push(FilmstripFrameAsset {
                index: frame.index,
                seek_sec: frame.seek_sec,
                uri: frame.artifact_uri.clone(),
                image,
            });
        }
        Ok(FilmstripBackground {
            clip_id: plan.clip_id.clone(),
            frames,
        })
    }
}

pub fn file_ready(path: &Path) -> bool {
    path.is_file() && path.metadata().map(|m| m.len()).unwrap_or(0) > 0
}

#[derive(Clone, Copy)]
struct Timing {
    frames: u64,
    timebase: FrameTimebase,
}

fn representation_timing(media: &MediaRepresentation) -> Option<Timing> {
    let video = first_video(media)?;
    Some(Timing {
        frames: video.exact_frame_count()?,
        timebase: video.frame_rate.as_ref()?.value,
    })
}

fn first_video(media: &MediaRepresentation) -> Option<&VideoMetadata> {
    media
        .streams
        .iter()
        .find_map(|stream| match &stream.details {
            StreamDetails::Video(video) => Some(video.as_ref()),
            _ => None,
        })
}

fn representation_duration_sec(media: &MediaRepresentation) -> Option<f64> {
    media
        .duration_seconds
        .as_ref()
        .and_then(|duration| rational_to_f64(duration.value))
}

fn rational_to_f64(value: Rational) -> Option<f64> {
    (value.denominator > 0).then(|| value.numerator as f64 / value.denominator as f64)
}

fn duration_from_frames(frames: u64, timebase: FrameTimebase) -> f64 {
    frames as f64 * timebase.fps_den as f64 / timebase.fps_num as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_media_metadata::{
        ClipMetadata, ColorMetadata, Evidence, EvidenceKind, Fact, FrameCount, FrameRateMode,
        Rational, ScanMode, Signal, VideoMetadata,
    };
    use qnc_media_records::{Binding, Phase, Snapshot};

    fn fact<T>(value: T, evidence_id: &str) -> Fact<T> {
        Fact {
            value,
            evidence_id: evidence_id.into(),
            locator: "test".into(),
        }
    }

    fn video(frames: u64, evidence_id: &str) -> StreamDetails {
        StreamDetails::Video(Box::new(VideoMetadata {
            width: Some(fact(1920, evidence_id)),
            height: Some(fact(1080, evidence_id)),
            frame_rate: Some(fact(FrameTimebase::new(50, 1).unwrap(), evidence_id)),
            frame_rate_mode: Some(fact(FrameRateMode::Constant, evidence_id)),
            frame_count: Some(fact(FrameCount::Exact(frames), evidence_id)),
            scan_mode: Some(fact(ScanMode::Progressive, evidence_id)),
            pixel_format: Some(fact("yuv422p".into(), evidence_id)),
            sample_aspect_ratio: Some(fact(
                Rational {
                    numerator: 1,
                    denominator: 1,
                },
                evidence_id,
            )),
            rotation_degrees: Some(fact(Signal::Known(0), evidence_id)),
            color: ColorMetadata {
                primaries: Some(fact(Signal::Known("bt709".into()), evidence_id)),
                transfer: Some(fact(Signal::Known("bt709".into()), evidence_id)),
                matrix: Some(fact(Signal::Known("bt709".into()), evidence_id)),
                range: Some(fact(Signal::Known("tv".into()), evidence_id)),
            },
        }))
    }

    fn media(uri: &str, frames: u64, evidence_id: &str) -> MediaRepresentation {
        MediaRepresentation {
            media_uri: uri.into(),
            container: Some(fact("mxf".into(), evidence_id)),
            duration_seconds: Some(fact(
                Rational {
                    numerator: frames as i64,
                    denominator: 50,
                },
                evidence_id,
            )),
            streams_complete: Some(fact(true, evidence_id)),
            streams: vec![qnc_media_metadata::MediaStream {
                index: Some(fact(0, evidence_id)),
                codec: Some(fact(Signal::Known("mpeg2video".into()), evidence_id)),
                profile: None,
                time_base: Some(fact(
                    Rational {
                        numerator: 1,
                        denominator: 50,
                    },
                    evidence_id,
                )),
                start_pts: Some(fact(0, evidence_id)),
                duration_ts: Some(fact(frames as i64, evidence_id)),
                details: video(frames, evidence_id),
            }],
            tags: Default::default(),
        }
    }

    fn snapshot(proxy: Option<MediaRepresentation>, frames: u64) -> Snapshot {
        let mut evidence = vec![Evidence {
            id: "e1".into(),
            kind: EvidenceKind::CameraMetadata,
            document_uri: "qnc://local/source/card/file/PRIVATE/CLIP/C0001.XML".into(),
            media_uri: "qnc://local/source/card/file/PRIVATE/CLIP/C0001.MXF".into(),
        }];
        if let Some(proxy) = &proxy {
            evidence.push(Evidence {
                id: "e2".into(),
                kind: EvidenceKind::CameraMetadata,
                document_uri: "qnc://local/source/card/file/PRIVATE/PROXY/C0001.XML".into(),
                media_uri: proxy.media_uri.clone(),
            });
        }
        let metadata = ClipMetadata {
            contract_id: qnc_media_metadata::CONTRACT_ID.into(),
            contract_version: qnc_media_metadata::CONTRACT_VERSION.into(),
            clip_id: "mironik_2002".into(),
            evidence,
            original: media(
                "qnc://local/source/card/file/PRIVATE/CLIP/C0001.MXF",
                frames,
                "e1",
            ),
            proxy,
        };
        let report = qnc_media_metadata::inspect(&metadata);
        let completeness = qnc_media_records::completeness(&report);
        let proxy_uri = metadata.proxy.as_ref().map(|proxy| proxy.media_uri.clone());
        Snapshot {
            binding: Binding {
                source_index_uri: "qnc://local/db/source_index".into(),
                source_record_id: "source_record_1".into(),
                original_uri: "qnc://local/source/card/file/PRIVATE/CLIP/C0001.MXF".into(),
                proxy_uri,
            },
            phase: Phase::Final,
            completeness,
            metadata,
            report,
            revision: 1,
            recorded_at_unix_ms: 1,
        }
    }

    #[test]
    fn creates_thirteen_even_segment_start_frames() {
        let plan = plan_from_snapshot(&snapshot(None, 7_000), "qnc://local/project/proj/filmstrip")
            .unwrap();
        assert_eq!(plan.frames.len(), FILMSTRIP_FRAME_COUNT);
        assert_eq!(plan.frames[0].seek_sec, 0.0);
        assert_eq!(plan.frames[1].seek_sec, 10.76);
        assert_eq!(plan.frames[12].seek_sec, 129.22);
        assert_eq!(plan.extraction_mode, FilmstripExtractionMode::KeyframeSeek);
        assert_eq!(
            plan.frames[0].artifact_uri,
            "qnc://local/project/proj/filmstrip/mironik_2002/000_0_00.jpg"
        );
    }

    #[test]
    fn short_clip_uses_keyframe_seek_mode() {
        let plan =
            plan_from_snapshot(&snapshot(None, 500), "qnc://local/project/proj/filmstrip").unwrap();
        assert_eq!(plan.extraction_mode, FilmstripExtractionMode::KeyframeSeek);
        assert_eq!(plan.frames[1].seek_sec, 0.76);
    }

    #[test]
    fn artifact_directory_can_use_database_clip_name() {
        let plan = plan_from_snapshot_with_artifact_name(
            &snapshot(None, 500),
            "qnc://local/project/proj/filmstrip",
            Some("Mironik 2002.MXF"),
        )
        .unwrap();
        assert_eq!(
            plan.artifact_root_uri,
            "qnc://local/project/proj/filmstrip/Mironik_2002.MXF"
        );
        assert_eq!(
            plan.frames[0].artifact_uri,
            "qnc://local/project/proj/filmstrip/Mironik_2002.MXF/000_0_00.jpg"
        );
        assert_eq!(plan.clip_id, "mironik_2002");
    }

    #[test]
    fn proxy_media_is_selected_when_present() {
        let proxy = media(
            "qnc://local/source/card/file/PRIVATE/PROXY/C0001.MP4",
            500,
            "e2",
        );
        let plan = plan_from_snapshot(
            &snapshot(Some(proxy), 500),
            "qnc://local/project/proj/filmstrip",
        )
        .unwrap();
        assert_eq!(plan.source_kind, FilmstripSourceKind::Proxy);
        assert_eq!(
            plan.source_uri,
            "qnc://local/source/card/file/PRIVATE/PROXY/C0001.MP4"
        );
    }
}

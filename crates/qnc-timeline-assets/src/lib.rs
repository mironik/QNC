//! Public read-only timeline asset loader.
//!
//! This module reads already published filmstrip and wave artifacts for passive
//! timeline painting. It does not generate media, probe, scan, own playback, own
//! application workflow, or write any database.

pub use qnc_filmstrip::FilmstripBackground;
use qnc_filmstrip::{FilmstripArtifactRecord, FilmstripFrameAsset, FILMSTRIP_FRAME_COUNT};
use qnc_wave::{WaveArtifactRecord, WaveformPeaks};
use std::{collections::BTreeMap, fmt, sync::Arc};

pub const MODULE_ID: &str = "qnc.module.timeline-assets";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub trait TimelineArtifactRead: Send + Sync {
    fn read_filmstrip(&self, clip_id: &str) -> Result<Option<FilmstripArtifactRecord>, String>;
    fn read_wave(&self, clip_id: &str) -> Result<Option<WaveArtifactRecord>, String>;
    fn read_image_bytes(&self, artifact_uri: &str) -> Result<Vec<u8>, String>;
}

#[derive(Clone)]
pub struct TimelineAssetContext {
    pub project_id: String,
    pub reader: Arc<dyn TimelineArtifactRead>,
}

impl TimelineAssetContext {
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
}

impl fmt::Debug for TimelineAssetContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TimelineAssetContext")
            .field("project_id", &self.project_id)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceTimelineAssets {
    pub clip_id: String,
    pub filmstrip_background: Option<FilmstripBackground>,
    pub wave: Option<WaveformPeaks>,
}

impl SourceTimelineAssets {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn empty_for(clip_id: impl Into<String>) -> Self {
        Self {
            clip_id: clip_id.into(),
            ..Default::default()
        }
    }

    pub fn filmstrip_background(&self) -> Option<&FilmstripBackground> {
        self.filmstrip_background
            .as_ref()
            .filter(|background| !background.is_empty())
    }

    pub fn a1_peaks(&self) -> &[f32] {
        self.wave
            .as_ref()
            .map(|wave| wave.a1_peaks.as_slice())
            .unwrap_or(&[])
    }

    pub fn a2_peaks(&self) -> &[f32] {
        self.wave
            .as_ref()
            .map(|wave| wave.a2_peaks.as_slice())
            .unwrap_or(&[])
    }

    pub fn a3_peaks(&self) -> &[f32] {
        self.wave
            .as_ref()
            .map(|wave| wave.a3_peaks.as_slice())
            .unwrap_or(&[])
    }

    pub fn a4_peaks(&self) -> &[f32] {
        self.wave
            .as_ref()
            .map(|wave| wave.a4_peaks.as_slice())
            .unwrap_or(&[])
    }
}

#[derive(Debug, Default)]
pub struct TimelineAssetReader {
    context: Option<TimelineAssetContext>,
    cache: BTreeMap<String, SourceTimelineAssets>,
}

impl TimelineAssetReader {
    pub fn configure(&mut self, context: TimelineAssetContext) {
        let changed = self
            .context
            .as_ref()
            .is_some_and(|old| old.project_id() != context.project_id());
        if changed {
            self.cache.clear();
        }
        self.context = Some(context);
    }

    pub fn reset(&mut self) {
        self.context = None;
        self.cache.clear();
    }

    pub fn remove_clips(&mut self, clip_ids: &[String]) {
        let Some(project_id) = self.context.as_ref().map(TimelineAssetContext::project_id) else {
            return;
        };
        for clip_id in clip_ids {
            self.cache.remove(&cache_key(project_id, clip_id));
        }
    }

    pub fn load_clip(&mut self, clip_id: &str) -> Result<SourceTimelineAssets, String> {
        qnc_media_records::valid_id(clip_id).map_err(|error| error.to_string())?;
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| "Timeline asset context nije postavljen.".to_string())?;
        let key = cache_key(context.project_id(), clip_id);
        if let Some(assets) = self.cache.get(&key) {
            return Ok(assets.clone());
        }
        let assets = read_assets(context, clip_id)?;
        if assets.filmstrip_background.is_some() || assets.wave.is_some() {
            self.cache.insert(key, assets.clone());
        }
        Ok(assets)
    }

    pub fn refresh_clip(&mut self, clip_id: &str) -> Result<SourceTimelineAssets, String> {
        let Some(project_id) = self.context.as_ref().map(TimelineAssetContext::project_id) else {
            return Err("Timeline asset context nije postavljen.".into());
        };
        self.cache.remove(&cache_key(project_id, clip_id));
        self.load_clip(clip_id)
    }
}

fn read_assets(
    context: &TimelineAssetContext,
    clip_id: &str,
) -> Result<SourceTimelineAssets, String> {
    let filmstrip_record = context.reader.read_filmstrip(clip_id)?;
    let wave_record = context.reader.read_wave(clip_id)?;
    Ok(SourceTimelineAssets {
        clip_id: clip_id.to_string(),
        filmstrip_background: match filmstrip_record {
            Some(record) => load_filmstrip_background(context, &record)?,
            None => None,
        },
        wave: wave_record.and_then(|record| ready_wave(&record)),
    })
}

fn load_filmstrip_background(
    context: &TimelineAssetContext,
    record: &FilmstripArtifactRecord,
) -> Result<Option<FilmstripBackground>, String> {
    if record.status != "ready" || record.frames.is_empty() {
        return Ok(None);
    }
    if record.frame_count != record.frames.len() || record.frames.len() > FILMSTRIP_FRAME_COUNT {
        return Err("Neispravan filmstrip zapis.".into());
    }
    let mut frames = Vec::with_capacity(record.frames.len());
    for frame in &record.frames {
        let bytes = context.reader.read_image_bytes(&frame.artifact_uri)?;
        let image = qnc_image_assets::decode_thumbnail(&bytes)?;
        frames.push(FilmstripFrameAsset {
            index: frame.index,
            seek_sec: frame
                .seek_sec
                .parse::<f64>()
                .map_err(|_| "Neispravan filmstrip vremenski polozaj.".to_string())?,
            uri: frame.artifact_uri.clone(),
            image,
        });
    }
    frames.sort_by_key(|frame| frame.index);
    Ok(Some(FilmstripBackground {
        clip_id: record.clip_id.clone(),
        frames,
    }))
}

fn ready_wave(record: &WaveArtifactRecord) -> Option<WaveformPeaks> {
    qnc_wave::validate_artifact(record).ok()?;
    (record.render_version == qnc_wave::WAVE_RENDER_VERSION)
        .then(|| record.peaks())
        .flatten()
}

fn cache_key(project_id: &str, clip_id: &str) -> String {
    format!("{project_id}::{clip_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_assets_return_empty_passive_inputs() {
        let assets = SourceTimelineAssets::empty_for("clip-1");

        assert_eq!(assets.clip_id, "clip-1");
        assert!(assets.filmstrip_background().is_none());
        assert!(assets.a1_peaks().is_empty());
        assert!(assets.a2_peaks().is_empty());
        assert!(assets.a3_peaks().is_empty());
        assert!(assets.a4_peaks().is_empty());
    }

    #[test]
    fn ready_wave_record_becomes_passive_peaks() {
        let record = WaveArtifactRecord {
            clip_id: "clip-1".into(),
            status: "ready".into(),
            artifact_uri: "qnc://local/db/ingest_content/p1/wave/clip-1".into(),
            source_uri: "qnc://local/source/card/clip-1/original".into(),
            source_sample_rate_hz: 48_000,
            peak_count: 2,
            a1_peaks: vec![0.25, 0.5],
            a2_peaks: vec![0.1, 0.2],
            a3_peaks: vec![0.05, 0.15],
            a4_peaks: Vec::new(),
            warning: None,
            render_version: qnc_wave::WAVE_RENDER_VERSION,
        };

        let peaks = ready_wave(&record).expect("ready wave");

        assert_eq!(peaks.a1_peaks, [0.25, 0.5]);
        assert_eq!(peaks.a2_peaks, [0.1, 0.2]);
        assert_eq!(peaks.a3_peaks, [0.05, 0.15]);
        assert!(peaks.a4_peaks.is_empty());
    }

    #[test]
    fn empty_artifact_lookup_is_not_cached() {
        #[derive(Default)]
        struct Reader {
            calls: std::sync::Mutex<usize>,
        }

        impl TimelineArtifactRead for Reader {
            fn read_filmstrip(
                &self,
                _clip_id: &str,
            ) -> Result<Option<FilmstripArtifactRecord>, String> {
                *self.calls.lock().unwrap() += 1;
                Ok(None)
            }

            fn read_wave(&self, _clip_id: &str) -> Result<Option<WaveArtifactRecord>, String> {
                Ok(None)
            }

            fn read_image_bytes(&self, _artifact_uri: &str) -> Result<Vec<u8>, String> {
                Ok(Vec::new())
            }
        }

        let reader = Arc::new(Reader::default());
        let mut assets = TimelineAssetReader::default();
        assets.configure(TimelineAssetContext {
            project_id: "project-1".into(),
            reader: reader.clone(),
        });

        assert!(assets
            .load_clip("clip-1")
            .unwrap()
            .filmstrip_background
            .is_none());
        assert!(assets
            .load_clip("clip-1")
            .unwrap()
            .filmstrip_background
            .is_none());

        assert_eq!(*reader.calls.lock().unwrap(), 2);
    }
}

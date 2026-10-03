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
    /// Reads running on their own thread (cache keys), and their finished results:
    /// the form's thread never waits on the database or decodes the filmstrip.
    asked: std::collections::BTreeSet<String>,
    loaded: Arc<std::sync::Mutex<Vec<(String, Result<SourceTimelineAssets, String>)>>>,
}

impl TimelineAssetReader {
    pub fn configure(&mut self, context: TimelineAssetContext) {
        let changed = self
            .context
            .as_ref()
            .is_some_and(|old| old.project_id() != context.project_id());
        if changed {
            self.cache.clear();
            self.asked.clear();
            self.loaded = Arc::default(); // reads of the old project land nowhere
        }
        self.context = Some(context);
    }

    /// The project the reader is set up for.
    pub fn project_id(&self) -> Option<&str> {
        self.context.as_ref().map(TimelineAssetContext::project_id)
    }

    pub fn reset(&mut self) {
        self.context = None;
        self.cache.clear();
        self.asked.clear();
        self.loaded = Arc::default(); // reads of the old project land nowhere
    }

    /// The assets of `clip_id` read on a thread of their own; `take_loaded` gives them.
    /// Returns what the cache already has (shown meanwhile); `fresh` reads again even
    /// then (artifacts may have appeared since).
    pub fn request(&mut self, clip_id: &str, fresh: bool) -> Option<SourceTimelineAssets> {
        qnc_media_records::valid_id(clip_id).ok()?;
        let context = self.context.clone()?;
        let key = cache_key(context.project_id(), clip_id);
        let cached = self.cache.get(&key).cloned();
        if (cached.is_some() && !fresh) || self.asked.contains(&key) {
            return cached;
        }
        self.asked.insert(key.clone());
        let mailbox = self.loaded.clone();
        let clip = clip_id.to_string();
        let started = std::thread::Builder::new().name("qnc-timeline-assets".into()).spawn(move || {
            let read = read_assets(&context, &clip);
            if let Ok(mut loaded) = mailbox.lock() {
                loaded.push((key, read));
            }
        });
        if started.is_err() {
            self.asked.remove(&cache_key(self.context.as_ref()?.project_id(), clip_id));
        }
        cached
    }

    /// Reads finished since the last call: the assets, or an empty set for that clip
    /// when the read failed (and the error).
    pub fn take_loaded(&mut self) -> Vec<(SourceTimelineAssets, Option<String>)> {
        let finished = self.loaded.lock().map(|mut loaded| std::mem::take(&mut *loaded)).unwrap_or_default();
        let mut taken = Vec::new();
        for (key, read) in finished {
            self.asked.remove(&key);
            let clip_id = key.rsplit("::").next().unwrap_or_default().to_string();
            match read {
                Ok(assets) => {
                    if assets.filmstrip_background.is_some() || assets.wave.is_some() {
                        self.cache.insert(key, assets.clone());
                    }
                    taken.push((assets, None));
                }
                Err(error) => taken.push((SourceTimelineAssets::empty_for(clip_id), Some(error))),
            }
        }
        taken
    }

    /// Whether a read is still running.
    pub fn loading(&self) -> bool {
        !self.asked.is_empty()
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

/// The poster of a clip across the filmstrip row, in every slot, when the project
/// makes no filmstrip (`artifacts.filmstrip = off`, user rule 2026-09-30). It is not a
/// generated filmstrip and never stands in for one: a project that makes filmstrips
/// (`made`) keeps its row empty until the real frames are there.
pub fn with_poster_filmstrip(
    mut assets: SourceTimelineAssets,
    made: bool,
    poster: Option<&qnc_image_assets::RgbaImage>,
) -> SourceTimelineAssets {
    if made || assets.filmstrip_background.is_some() || assets.clip_id.is_empty() {
        return assets;
    }
    let Some(poster) = poster else {
        return assets;
    };
    let uri = format!("poster:{}", assets.clip_id);
    assets.filmstrip_background = Some(FilmstripBackground {
        clip_id: assets.clip_id.clone(),
        frames: (0..FILMSTRIP_FRAME_COUNT)
            .map(|index| FilmstripFrameAsset {
                index,
                seek_sec: 0.0,
                uri: uri.clone(),
                image: poster.clone(),
            })
            .collect(),
    });
    assets
}

/// The same for the clip on screen, its poster found among `posters` (clip id, loaded
/// poster) and `made` read from the project settings (`None`: not loaded, treated as made).
pub fn with_clip_poster<'a>(
    assets: SourceTimelineAssets,
    settings: Option<&qnc_work_settings::WorkSettings>,
    posters: impl IntoIterator<Item = (&'a str, Option<&'a qnc_image_assets::RgbaImage>)>,
) -> SourceTimelineAssets {
    let made = settings.is_none_or(qnc_work_settings::WorkSettings::filmstrip_made);
    let clip_id = assets.clip_id.clone();
    let poster = posters.into_iter().find(|(id, _)| *id == clip_id).and_then(|(_, poster)| poster);
    with_poster_filmstrip(assets, made, poster)
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
    fn a_project_without_filmstrips_shows_the_poster_across_the_row() {
        let poster = qnc_image_assets::RgbaImage {
            size: [2, 1],
            pixels: vec![255; 8],
            content_key: 7,
        };
        let off = with_poster_filmstrip(SourceTimelineAssets::empty_for("clip-1"), false, Some(&poster));
        let row = off.filmstrip_background().expect("the poster fills the row");
        assert_eq!(row.frames.len(), FILMSTRIP_FRAME_COUNT);
        assert!(row.frames.iter().all(|frame| frame.image == poster));
        let made = with_poster_filmstrip(SourceTimelineAssets::empty_for("clip-1"), true, Some(&poster));
        assert!(made.filmstrip_background().is_none(), "a real filmstrip is never replaced");
        let without = with_poster_filmstrip(SourceTimelineAssets::empty_for("clip-1"), false, None);
        assert!(without.filmstrip_background().is_none());
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

        // Read on its own thread: the request returns at once, the result comes later,
        // and a second request while one runs reads nothing more.
        assert!(assets.request("clip-2", true).is_none());
        assert!(assets.request("clip-2", true).is_none());
        let start = std::time::Instant::now();
        let taken = loop {
            let taken = assets.take_loaded();
            if !taken.is_empty() {
                break taken;
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(5), "no read");
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].0.clip_id, "clip-2");
        assert!(!assets.loading());
        assert_eq!(*reader.calls.lock().unwrap(), 3);
    }
}

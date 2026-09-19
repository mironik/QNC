//! Timeline artifacts.
//!
//! One public component for every form that shows a source timeline: it runs the
//! filmstrip and wave workers, reads what they published and hands the artifacts of
//! the focused clip to the timeline. It knows no application and no database owner:
//! the caller supplies the content readers and writers (traits of the workers and of
//! `qnc-timeline-assets`) and the source bindings, so the same component serves Ingest,
//! Media Assist, Story or any future form, on a local disk, LAN or intranet.

use qnc_filmstrip_worker::{
    FilmstripContentRead, FilmstripContentWriteFactory, FilmstripContext, FilmstripSourceBinding,
    TimelineFilmstripService,
};
use qnc_timeline_assets::{
    SourceTimelineAssets, TimelineArtifactRead, TimelineAssetContext, TimelineAssetReader,
};
use qnc_wave_worker::{
    TimelineWaveService, WaveContentRead, WaveContentWriteFactory, WaveContext, WaveSourceBinding,
};
use std::{path::PathBuf, sync::Arc};

pub const MODULE_ID: &str = "qnc.module.timeline-artifacts";
pub const VERSION: &str = "0.1.0";

/// Everything it needs for one project.
pub struct ArtifactsContext {
    pub project_id: String,
    pub filmstrip_root_uri: String,
    /// Where the filmstrip frames of this machine live.
    pub filmstrip_dir: PathBuf,
    pub wave_root_uri: String,
    pub project_audio_channels: u16,
    pub timeline_reader: Arc<dyn TimelineArtifactRead>,
    pub filmstrip_reader: Arc<dyn FilmstripContentRead>,
    pub filmstrip_writer: Arc<dyn FilmstripContentWriteFactory>,
    pub filmstrip_sources: Vec<FilmstripSourceBinding>,
    pub wave_reader: Arc<dyn WaveContentRead>,
    pub wave_writer: Arc<dyn WaveContentWriteFactory>,
    pub wave_sources: Vec<WaveSourceBinding>,
}

/// Result of one poll: whether anything changed, the first error of the focused clip,
/// and the refreshed artifacts of the focused clip when they changed.
#[derive(Debug, Default)]
pub struct Polled {
    pub changed: bool,
    pub error: Option<String>,
    pub assets: Option<SourceTimelineAssets>,
}

#[derive(Default)]
pub struct Artifacts {
    filmstrip: TimelineFilmstripService,
    wave: TimelineWaveService,
    assets: TimelineAssetReader,
    sync_deferred: bool,
}

impl Artifacts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget everything of the previous project.
    pub fn reset(&mut self) {
        self.filmstrip.reset();
        self.wave.reset();
        self.assets.reset();
        self.sync_deferred = false;
    }

    pub fn configure(&mut self, context: ArtifactsContext) {
        self.assets.configure(TimelineAssetContext {
            project_id: context.project_id.clone(),
            reader: context.timeline_reader,
        });
        self.filmstrip.configure(FilmstripContext {
            project_id: context.project_id.clone(),
            filmstrip_root_uri: context.filmstrip_root_uri,
            filmstrip_dir: context.filmstrip_dir,
            content_reader: context.filmstrip_reader,
            content_writer: context.filmstrip_writer,
            source_bindings: context.filmstrip_sources,
        });
        self.wave.configure(WaveContext {
            project_id: context.project_id,
            wave_root_uri: context.wave_root_uri,
            project_audio_channels: context.project_audio_channels,
            content_reader: context.wave_reader,
            content_writer: context.wave_writer,
            source_bindings: context.wave_sources,
        });
    }

    /// Makes the workers look for clips without artifacts. While something must keep
    /// priority (playback) the sync is deferred and `sync_deferred` says so.
    pub fn sync(&mut self, defer: bool) -> Result<(), String> {
        if defer {
            self.sync_deferred = true;
            self.set_playback_priority(true);
            return Ok(());
        }
        self.sync_deferred = false;
        let filmstrip = self.filmstrip.sync_content_db();
        let wave = self.wave.sync_content_db();
        filmstrip.and(wave)
    }

    pub fn sync_deferred(&self) -> bool {
        self.sync_deferred
    }

    /// The artifacts of one clip as they are now.
    pub fn focus(&mut self, clip_id: &str) -> SourceTimelineAssets {
        self.assets
            .load_clip(clip_id)
            .unwrap_or_else(|_| SourceTimelineAssets::empty_for(clip_id))
    }

    pub fn remove_clips(&mut self, clip_ids: &[String]) {
        self.filmstrip.remove_clips(clip_ids);
        self.wave.remove_clips(clip_ids);
        self.assets.remove_clips(clip_ids);
    }

    pub fn poll(&mut self, active_clip_id: Option<&str>) -> Polled {
        let filmstrip = self.filmstrip.poll(active_clip_id);
        let wave = self.wave.poll(active_clip_id);
        let mut polled = Polled {
            changed: filmstrip.changed || wave.changed,
            error: filmstrip.active_error.or(wave.active_error),
            assets: None,
        };
        if polled.changed {
            if let Some(clip_id) = active_clip_id {
                polled.assets = self.assets.refresh_clip(clip_id).ok();
            }
        }
        polled
    }

    pub fn set_playback_priority(&mut self, active: bool) {
        self.filmstrip.set_playback_priority(active);
        self.wave.set_playback_priority(active);
    }

    pub fn has_pending_work(&self) -> bool {
        self.filmstrip.has_pending_work() || self.wave.has_pending_work()
    }
}

impl std::fmt::Debug for Artifacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Artifacts")
            .field("pending", &self.has_pending_work())
            .field("sync_deferred", &self.sync_deferred)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_host_has_nothing_pending_and_focus_of_an_unknown_clip_is_empty() {
        let mut artifacts = Artifacts::new();
        assert!(!artifacts.has_pending_work());
        assert!(!artifacts.sync_deferred());
        let assets = artifacts.focus("clip-1");
        assert_eq!(assets.clip_id, "clip-1");
        assert!(assets.filmstrip_background().is_none());
    }

    #[test]
    fn a_deferred_sync_is_remembered_until_it_can_run() {
        let mut artifacts = Artifacts::new();
        artifacts.sync(true).unwrap();
        assert!(artifacts.sync_deferred());
        artifacts.sync(false).ok();
        assert!(!artifacts.sync_deferred());
    }

    #[test]
    fn polling_without_a_project_changes_nothing() {
        let mut artifacts = Artifacts::new();
        let polled = artifacts.poll(Some("clip-1"));
        assert!(!polled.changed);
        assert!(polled.assets.is_none());
        artifacts.reset();
        artifacts.remove_clips(&["clip-1".into()]);
    }
}

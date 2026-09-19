//! The application only decides *when*; the timeline artifact host does the work
//! and `qnc-ingest-artifacts` connects it to the Ingest content transport.

use super::*;

impl IngestApplication {
    pub(super) fn reset_timeline_artifacts(&mut self) {
        self.artifacts.reset();
        self.view.timeline_assets = qnc_timeline_assets::SourceTimelineAssets::empty();
    }

    pub(super) fn refresh_timeline_artifact_context(&mut self) {
        let (Some(reader), Some(plan), Some(target), Some(config)) = (
            self.settings_reader.as_ref(),
            self.work_plan(),
            self.catalog_target.clone(),
            self.selection_config.as_ref(),
        ) else {
            return;
        };
        match qnc_ingest_artifacts::artifacts_context(reader, plan, target, config) {
            Ok(context) => self.artifacts.configure(context),
            Err(error) => self.view.message = error,
        }
    }

    pub(super) fn sync_timeline_artifact_content_db(&mut self) {
        self.refresh_timeline_artifact_context();
        let defer = self.playback_guard_active();
        if let Err(error) = self.artifacts.sync(defer) {
            self.view.message = error;
        }
    }

    pub(super) fn focus_timeline_assets(&mut self, clip_id: &str) {
        self.refresh_timeline_artifact_context();
        self.view.timeline_assets = self.artifacts.focus(clip_id);
    }

    pub(super) fn remove_timeline_artifact_clips(&mut self, clip_ids: &[String]) {
        self.artifacts.remove_clips(clip_ids);
        if self
            .view
            .preview_clip_id
            .as_ref()
            .is_some_and(|id| clip_ids.contains(id))
        {
            self.view.timeline_assets = qnc_timeline_assets::SourceTimelineAssets::empty();
        }
    }

    pub(super) fn poll_timeline_artifacts(&mut self) -> bool {
        let polled = self.artifacts.poll(self.view.preview_clip_id.as_deref());
        if let Some(error) = polled.error {
            self.view.message = error;
        }
        if let Some(assets) = polled.assets {
            self.view.timeline_assets = assets;
        }
        polled.changed
    }

    pub(super) fn set_timeline_artifact_playback_priority(&mut self, active: bool) {
        self.artifacts.set_playback_priority(active);
    }

    pub(super) fn update_timeline_artifact_playback_priority(&mut self) {
        self.set_timeline_artifact_playback_priority(self.playback_guard_active());
    }
}

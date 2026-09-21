//! The application only decides *when*; the neutral project content artifact
//! adapter feeds the timeline artifact host from the active project database.

use super::*;

impl IngestApplication {
    pub(super) fn reset_timeline_artifacts(&mut self) {
        self.artifacts.reset();
        self.view.timeline_assets = qnc_timeline_assets::SourceTimelineAssets::empty();
    }

    pub(super) fn refresh_timeline_artifact_context(&mut self) -> Result<(), String> {
        let (Some(reader), Some(plan), Some(target)) = (
            self.settings_reader.as_ref(),
            self.work_plan().cloned(),
            self.artifact_target.clone(),
        ) else {
            return Ok(());
        };
        self.artifacts
            .configure(reader, &plan.settings, target)
    }

    pub(super) fn sync_timeline_artifact_content_db(&mut self) {
        let (Some(reader), Some(plan), Some(target)) = (
            self.settings_reader.as_ref(),
            self.work_plan().cloned(),
            self.artifact_target.clone(),
        ) else {
            if self.playback_guard_active() {
                self.artifacts.defer_sync();
            }
            return;
        };
        let defer = self.playback_guard_active();
        if let Err(error) = self.artifacts.sync(reader, &plan.settings, target, defer) {
            self.view.message = error;
        }
    }

    pub(super) fn focus_timeline_assets(&mut self, clip_id: &str) {
        let (Some(reader), Some(plan), Some(target)) = (
            self.settings_reader.as_ref(),
            self.work_plan().cloned(),
            self.artifact_target.clone(),
        ) else {
            self.view.timeline_assets =
                qnc_timeline_assets::SourceTimelineAssets::empty_for(clip_id);
            return;
        };
        match self.artifacts.focus(
            reader,
            &plan.settings,
            target,
            clip_id,
        ) {
            Ok(assets) => self.view.timeline_assets = assets,
            Err(error) => self.view.message = error,
        }
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
        if polled.changed && self.preview.refresh_assets() {
            self.sync_playback_view();
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

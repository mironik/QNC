//! Filmstrip and wave are made by one generator, the background worker (its lease in
//! the project database keeps it single); the application only wakes it. The artifacts of
//! the clip on screen are read by the universal source preview, as in every form.

use super::*;

impl IngestApplication {
    pub(super) fn reset_timeline_artifacts(&mut self) {
        self.artifacts.reset();
        self.view.timeline_assets = qnc_timeline_assets::SourceTimelineAssets::empty();
    }

    /// The artifacts of the active project are the background worker's: it is woken,
    /// never duplicated in this process (two generators on one card halve the pace).
    pub(super) fn wake_artifact_worker(&mut self) {
        if self.root.is_some() {
            if let Err(error) = self.begin_import() {
                self.view.message = error;
            }
        }
    }

    pub(super) fn remove_timeline_artifact_clips(&mut self, clip_ids: &[String]) {
        self.artifacts.remove_clips(clip_ids);
        self.preview.source_preview_mut().forget_assets(clip_ids);
        if self
            .view
            .preview_clip_id
            .as_ref()
            .is_some_and(|id| clip_ids.contains(id))
        {
            self.view.timeline_assets = qnc_timeline_assets::SourceTimelineAssets::empty();
        }
    }

    pub(super) fn set_timeline_artifact_playback_priority(&mut self, active: bool) {
        self.artifacts.set_playback_priority(active);
    }

    pub(super) fn update_timeline_artifact_playback_priority(&mut self) {
        self.set_timeline_artifact_playback_priority(self.playback_guard_active());
    }
}

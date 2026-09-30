//! Filmstrip and wave are made by one generator, the background worker (its lease in
//! the project database keeps it single); the application only wakes it and reads the
//! artifacts of the clip on screen from the active project database.

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

    pub(super) fn focus_timeline_assets(&mut self, clip_id: &str) {
        let (Some(reader), Some(plan)) = (self.settings_reader.as_ref(), self.work_plan().cloned())
        else {
            self.view.timeline_assets =
                qnc_timeline_assets::SourceTimelineAssets::empty_for(clip_id);
            return;
        };
        let posters = self.view.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.thumb_image.as_deref()));
        match self.artifacts.focus(reader, &plan.settings, clip_id) {
            Ok(found) => {
                self.view.timeline_assets = qnc_timeline_assets::with_clip_poster(found, Some(&plan.settings), posters)
            }
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

    /// The clip on screen is read again while the worker still makes its artifacts.
    pub(super) fn poll_timeline_artifacts(&mut self) -> bool {
        if !self.artifacts.refresh_due(&self.view.timeline_assets) {
            return false;
        }
        let clip_id = self.view.timeline_assets.clip_id.clone();
        self.focus_timeline_assets(&clip_id);
        true
    }

    pub(super) fn set_timeline_artifact_playback_priority(&mut self, active: bool) {
        self.artifacts.set_playback_priority(active);
    }

    pub(super) fn update_timeline_artifact_playback_priority(&mut self) {
        self.set_timeline_artifact_playback_priority(self.playback_guard_active());
    }
}

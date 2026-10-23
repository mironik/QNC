use super::*;

impl IngestApplication {
    pub fn request_missing_thumbnails(&mut self) {
        self.start_thumbnail_load(qnc_media_thumbnail::missing(&self.view.clips));
    }

    pub(crate) fn start_thumbnail_load(&mut self, clips: Vec<(String, String)>) {
        if self.playback_guard_active() {
            return;
        }
        if clips.is_empty() {
            return;
        }
        let sources = self.selection_config.as_ref().map(|c| c.source_readers()).unwrap_or_default();
        let project = self.project.project_folder();
        if sources.is_empty() && project.is_none() {
            return;
        }
        self.posters.configure(&sources, project);
        self.posters
            .request(clips, self.view.preview_clip_id.as_deref());
    }

    pub(crate) fn cancel_thumbnail_load(&mut self) {
        self.posters.cancel();
    }

    pub(crate) fn poll_thumbnails(&mut self) -> bool {
        // A background process makes posters for clips whose source has none: the list
        // is read again (its signature only) while some clip still waits for one.
        if self.posters.missing_due(self.view.clips.iter().any(|clip| clip.thumb_uri.is_none())) {
            let _ = self.refresh_active_project();
        }
        self.posters.poll_into(&mut self.view.clips).0
    }
}

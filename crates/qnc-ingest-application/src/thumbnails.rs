use super::*;

impl IngestApplication {
    pub fn request_missing_thumbnails(&mut self) {
        let clips = self
            .view
            .clips
            .iter()
            .filter(|clip| clip.thumb_image.is_none())
            .filter_map(|clip| Some((clip.clip_id.clone(), clip.thumb_uri.clone()?)))
            .collect::<Vec<_>>();
        self.start_thumbnail_load(clips);
    }

    pub(crate) fn start_thumbnail_load(&mut self, clips: Vec<(String, String)>) {
        if self.playback_guard_active() {
            return;
        }
        if clips.is_empty() {
            return;
        }
        let sources = self
            .selection_config
            .as_ref()
            .map(|config| {
                config
                    .sources
                    .iter()
                    .filter_map(|source| source.reader().ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let project = self
            .settings_reader
            .as_ref()
            .zip(self.work_plan.as_ref())
            .and_then(|(reader, plan)| catalog::project_folder(reader, plan));
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
        let mut changed = false;
        for poster in self.posters.poll() {
            if let Some(clip) = self
                .view
                .clips
                .iter_mut()
                .find(|clip| clip.clip_id == poster.clip_id)
            {
                clip.thumb_image = Some(poster.image);
                clip.thumb_status = ThumbStatus::Ready;
                changed = true;
            }
        }
        changed
    }
}

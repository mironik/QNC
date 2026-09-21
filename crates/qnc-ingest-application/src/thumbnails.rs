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
        self.cancel_thumbnail_load();
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
        let requests = clips
            .into_iter()
            .map(|(clip_id, uri)| qnc_media_thumbnail::ThumbnailRequest {
                item_id: clip_id,
                uri,
            })
            .collect::<Vec<_>>();
        if let Err(error) = self
            .thumbnail_loader
            .start_with_project(sources, project, requests)
        {
            self.view.message = error;
        }
    }

    pub(crate) fn cancel_thumbnail_load(&mut self) {
        self.thumbnail_loader.cancel();
    }

    pub(crate) fn poll_thumbnails(&mut self) -> bool {
        self.thumbnail_loader.poll_into(&mut self.view.clips, 16)
    }
}

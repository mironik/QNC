use super::*;

impl IngestApplication {
    pub(crate) fn start_thumbnail_load(&mut self, clips: Vec<(String, String)>) {
        self.cancel_thumbnail_load();
        if self.playback_guard_active() {
            return;
        }
        if clips.is_empty() {
            return;
        }
        let Some(config) = self.selection_config.clone() else {
            return;
        };
        let sources = config
            .sources
            .iter()
            .filter_map(|source| source.reader().ok())
            .collect::<Vec<_>>();
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
        if let Err(error) = self.thumbnail_loader.start_with_project(sources, project, requests) {
            self.view.message = error;
        }
    }

    pub(crate) fn cancel_thumbnail_load(&mut self) {
        self.thumbnail_loader.cancel();
    }

    pub(crate) fn poll_thumbnails(&mut self) -> bool {
        let mut changed = false;
        for event in self.thumbnail_loader.poll(16) {
            match event {
                qnc_media_thumbnail::ThumbnailEvent::Ready {
                    item_id,
                    uri,
                    image,
                } => {
                    if let Some(clip) = self.view.clips.iter_mut().find(|clip| {
                        clip.clip_id == item_id && clip.thumb_uri.as_deref() == Some(uri.as_str())
                    }) {
                        clip.thumb_image = Some(image);
                        clip.thumb_status = ThumbStatus::Ready;
                        changed = true;
                    }
                }
                qnc_media_thumbnail::ThumbnailEvent::Finished => break,
            }
        }
        changed
    }
}

use super::*;

impl IngestApplication {
    pub fn poll(&mut self) -> bool {
        let mut changed = self.poll_settings();
        if self.preview.poll() {
            self.sync_playback_view();
            let message = self.preview.take_message();
            if !message.is_empty() {
                self.view.message = message;
            }
            changed = true;
        }
        self.apply_playback_guard();
        // A player works: the copy of the background application waits (told through the database).
        self.runtime
            .update(self.catalog_target.as_ref(), self.playback_guard_active());
        if !self.playback_guard_active() {
            changed |= self.poll_thumbnails();
        }
        self.apply_playback_guard();
        if !self.playback_guard_active() && self.artifacts.sync_deferred() {
            self.sync_timeline_artifact_content_db();
            changed = true;
        }
        changed |= self.poll_timeline_artifacts();
        if let Some(result) = self.selection_writer.poll() {
            match result {
                Ok(_) => match self.begin_import() {
                    Ok(()) => {
                        self.sync_timeline_artifact_content_db();
                        self.view.command_busy = false;
                        self.view.message = "Uvoz je predan pozadinskoj aplikaciji.".into();
                        self.request_navigation_after_import();
                    }
                    Err(error) => {
                        self.view.command_busy = false;
                        self.view.message = error;
                    }
                },
                Err(error) => {
                    self.view.command_busy = false;
                    self.view.message = error;
                }
            }
            changed = true;
        }
        if let Some(result) = self.browse.poll() {
            self.apply_source_browser_result(result);
            changed = true;
        }
        for event in self.selection_session.poll(64) {
            changed = true;
            let outcome = qnc_ingest_clip_list::apply_select_event(
                &mut self.view.clips,
                self.view.preview_clip_id.as_deref(),
                &mut self.selection_events,
                event,
            );
            if let Some(count) = outcome.warning_count {
                self.view.select_warning_count = count;
            }
            if let Some(message) = outcome.message {
                self.view.message = message;
            }
            if !outcome.removed_ids.is_empty() {
                self.remove_timeline_artifact_clips(&outcome.removed_ids);
            }
            if outcome.preview_removed {
                self.view.preview_clip_id = None;
                self.stop_player();
            }
            if outcome.finished {
                self.view.command_busy = false;
                if outcome.finished_ok {
                    self.sync_timeline_artifact_content_db();
                }
            }
        }
        changed
    }
}

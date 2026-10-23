use super::*;

impl IngestApplication {
    pub fn poll(&mut self, pointed: Option<&str>) -> bool {
        let mut changed = self.poll_settings();
        if self.preview.poll(pointed) {
            self.sync_playback_view();
            let message = self.preview.take_message();
            if !message.is_empty() {
                self.view.message = message;
            }
            changed = true;
        }
        self.apply_playback_guard();
        if !self.playback_guard_active() {
            changed |= self.poll_thumbnails();
        }
        if let Some(result) = self.selection_writer.poll() {
            match result {
                Ok(_) => match self.begin_import() {
                    Ok(()) => {
                        self.view.command_busy = false;
                        self.view.message = qnc_ui_text::text("ingest.msg.import_handed_over").into();
                        self.request_navigation_after_import();
                    }
                    Err(error) => {
                        self.view.command_busy = false;
                        self.view.message = error;
                    }
                },
                Err(error) => {
                    (self.view.command_busy, self.view.message) = (false, error);
                    self.view.import_choice = self.selection_writer.choice().cloned(); // clips in the way: the user chooses
                }
            }
            changed = true;
        }
        if let Some(result) = self.browse.poll() {
            self.apply_source_browser_result(result);
            changed = true;
        }
        let polled = self.select.poll(&mut self.view.clips, self.view.preview_clip_id.as_deref());
        changed |= polled.changed;
        self.view.select_warning_count = polled.warning_count.unwrap_or(self.view.select_warning_count);
        self.view.message = polled.message.unwrap_or(std::mem::take(&mut self.view.message));
        if !polled.removed_ids.is_empty() {
            self.remove_timeline_artifact_clips(&polled.removed_ids);
        }
        if polled.preview_removed {
            self.view.preview_clip_id = None;
            self.stop_player();
        }
        if let Some(ok) = polled.finished {
            self.view.command_busy = false;
            if ok {
                // The background completes the card records playback still lacks (v5)
                // and makes the filmstrip and wave: one generator.
                self.view.message = self.begin_import().err().unwrap_or_default();
            }
        }
        changed
    }
}

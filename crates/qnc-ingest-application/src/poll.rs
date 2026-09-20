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
            let import = std::mem::take(&mut self.import_after_selection);
            match result {
                Ok(_) if import => match self.begin_import() {
                    Ok(()) => {
                        self.view.command_busy = false;
                        self.view.message = "Uvoz je predan pozadinskoj aplikaciji.".into();
                        self.request_navigation_after_import();
                    }
                    Err(error) => {
                        self.view.command_busy = false;
                        self.view.message = error;
                    }
                },
                Ok(_) => {}
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
            match event {
                selection::Event::Status(message) => self.view.message = message,
                selection::Event::Warning(message) => {
                    self.selection_warnings += 1;
                    self.view.select_warning_count = self.selection_warnings;
                    self.selection_last_warning = Some(message.clone());
                    self.view.message = message;
                }
                selection::Event::Clip(clip) => {
                    let mut clip = ClipView::from(clip);
                    if let Some(existing) = self
                        .view
                        .clips
                        .iter_mut()
                        .find(|c| c.clip_id == clip.clip_id)
                    {
                        clip.selected = existing.selected;
                        *existing = clip;
                    } else {
                        let index = self.view.clips.partition_point(|c| c.name < clip.name);
                        self.view.clips.insert(index, clip);
                    }
                }
                selection::Event::Existing(ids) => {
                    for clip in &mut self.view.clips {
                        if ids.contains(&clip.clip_id) {
                            clip.previously_seen = true;
                        }
                    }
                }
                selection::Event::Saved { revisions, error } => {
                    for clip in &mut self.view.clips {
                        if revisions.iter().any(|(id, revision)| {
                            id == &clip.clip_id && *revision == clip.metadata_revision
                        }) {
                            clip.save_state = if error.is_some() {
                                SaveState::Failed
                            } else {
                                SaveState::Saved
                            };
                        }
                    }
                    if let Some(error) = error {
                        self.selection_warnings += 1;
                        self.view.select_warning_count = self.selection_warnings;
                        self.selection_last_warning = Some(error.clone());
                        self.view.message = error;
                    }
                }
                selection::Event::Removed(ids) => {
                    self.view.clips.retain(|c| !ids.contains(&c.clip_id));
                    self.remove_timeline_artifact_clips(&ids);
                    if self
                        .view
                        .preview_clip_id
                        .as_ref()
                        .is_some_and(|id| ids.contains(id))
                    {
                        self.view.preview_clip_id = None;
                        self.stop_player();
                    }
                }
                selection::Event::Finished(result) => {
                    self.view.command_busy = false;
                    let finished_ok = result.is_ok();
                    self.view.message = match result {
                        Ok(summary) if self.selection_warnings == 0 => format!(
                            "Select: {} postojećih; {} obrađenih; {} uklonjenih ({:.1} s).",
                            summary.unchanged,
                            summary.processed,
                            summary.removed,
                            summary.elapsed_ms as f64 / 1000.0
                        ),
                        Ok(_) => format!(
                            "Select: {} klipova; {} upozorenja. {}",
                            self.view.clips.len(),
                            self.selection_warnings,
                            self.selection_last_warning.as_deref().unwrap_or_default()
                        ),
                        Err(error) => {
                            self.view.select_warning_count += 1;
                            error
                        }
                    };
                    if finished_ok {
                        self.sync_timeline_artifact_content_db();
                    }
                }
            }
        }
        changed
    }
}

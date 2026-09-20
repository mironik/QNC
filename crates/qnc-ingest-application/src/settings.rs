use super::*;

impl IngestApplication {
    pub(crate) fn settings_failed(&mut self, error: String) {
        self.stop_player();
        self.cancel_thumbnail_load();
        self.work_plan = None;
        self.catalog_target = None;
        self.catalog_stats = None;
        self.view.clips.clear();
        self.view.timeline = Default::default();
        self.reset_timeline_artifacts();
        self.view.clip_filter = ClipFilter::All;
        self.view.preview_clip_id = None;
        self.pending_source = None;
        self.catalog_loader.cancel();
        self.view.work_settings_loading = false;
        self.view.work_settings_ready = false;
        self.view.work_settings_error = Some(error);
        self.view.ai_mining = false;
    }

    pub fn refresh_active_project(&mut self) -> IngestDispatchResult {
        self.load_work_settings_inner(None, true)
    }

    pub(crate) fn load_work_settings(&mut self, pending_source: Option<String>) -> IngestDispatchResult {
        self.load_work_settings_inner(pending_source, false)
    }

    pub(crate) fn load_work_settings_inner(
        &mut self,
        pending_source: Option<String>,
        retain_loaded_workspace: bool,
    ) -> IngestDispatchResult {
        if self.catalog_loader.is_busy()
            || self.selection_writer.is_busy()
            || self.selection_session.has_pending_work()
        {
            if retain_loaded_workspace && pending_source.is_none() && self.catalog_loader.is_busy() {
                return IngestDispatchResult::accepted(None, true);
            }
            return IngestDispatchResult::rejected("Citanje radnih postavki je u tijeku.");
        }
        let Some(reader) = self.settings_reader.clone() else {
            self.settings_failed("Nema konfiguriranog citaca radnih postavki.".into());
            return IngestDispatchResult::rejected("Nema konfiguriranog citaca radnih postavki.");
        };
        let keep_current_view =
            retain_loaded_workspace && pending_source.is_none() && self.work_plan.is_some();
        let retained_workspace = keep_current_view
            .then(|| {
                self.work_plan
                    .as_ref()
                    .map(|p| p.settings.workspace_db_uri.clone())
            })
            .flatten();
        let retained_stats = keep_current_view
            .then(|| self.catalog_stats.clone())
            .flatten();
        self.pending_source = pending_source;
        if !keep_current_view {
            self.stop_player();
            self.cancel_thumbnail_load();
            self.view.work_settings_loading = true;
            self.view.work_settings_ready = false;
        }
        self.view.work_settings_error = None;
        if let Err(error) = self.catalog_loader.start(reader, retained_workspace, retained_stats) {
            self.settings_failed(error);
        }
        IngestDispatchResult::accepted(None, true)
    }

    pub(crate) fn poll_settings(&mut self) -> bool {
        let Some(result) = self.catalog_loader.poll() else {
            return false;
        };
        self.view.work_settings_loading = false;
        match result {
            Ok(loaded) => {
                let plan = loaded.plan;
                let catalog_was_loaded = loaded.clips.is_some();
                // A new project must never inherit the preceding project's selection/preview.
                if self.work_plan.as_ref().map(|p| &p.settings.project_id)
                    != Some(&plan.settings.project_id)
                {
                    self.cancel_thumbnail_load();
                    self.view.clips.clear();
                    self.view.clip_filter = ClipFilter::All;
                    self.view.preview_clip_id = None;
                    self.reset_timeline_artifacts();
                    self.stop_player();
                    self.view.selected_source_uri = None;
                    self.view.selected_source_name.clear();
                    self.view.selected_source_serial_number.clear();
                    self.view.selected_source_volume_name.clear();
                    self.catalog_stats = None;
                }
                self.view.ai_mining = plan.settings.ai_enabled();
                self.view.archive_original_available = false;
                self.view.archive_original = false;
                self.view.work_settings_ready = true;
                self.catalog_target = Some(loaded.target);
                self.catalog_stats = Some(loaded.stats);
                if let Some(clips) = loaded.clips {
                    let marked: std::collections::HashMap<&str, bool> = self
                        .view
                        .clips
                        .iter()
                        .map(|clip| (clip.clip_id.as_str(), clip.selected))
                        .collect();
                    let clips = clips
                        .into_iter()
                        .map(ClipView::from)
                        .map(|mut clip| {
                            if let Some(selected) = marked.get(clip.clip_id.as_str()) {
                                clip.selected = *selected;
                            }
                            clip
                        })
                        .collect::<Vec<_>>();
                    let thumbnails = clips
                        .iter()
                        .filter_map(|clip| {
                            clip.thumb_uri
                                .as_ref()
                                .map(|uri| (clip.clip_id.clone(), uri.clone()))
                        })
                        .collect::<Vec<_>>();
                    self.view.clips = clips;
                    self.start_thumbnail_load(thumbnails);
                }
                if self.pending_source.is_none() && catalog_was_loaded {
                    if let Some(source) = loaded.source {
                        self.view.selected_source_uri = Some(source.uri);
                        self.view.selected_source_name = source.name;
                        self.view.selected_source_serial_number = source.serial_number;
                        self.view.selected_source_volume_name = source.volume_name;
                    } else {
                        self.view.selected_source_uri = None;
                        self.view.selected_source_name.clear();
                        self.view.selected_source_serial_number.clear();
                        self.view.selected_source_volume_name.clear();
                    }
                }
                self.work_plan = Some(plan);
                self.refresh_timeline_artifact_context();
                if catalog_was_loaded {
                    self.sync_timeline_artifact_content_db();
                }
                if let Some(uri) = self.pending_source.take() {
                    if self.playback_guard_active() {
                        self.view.message = playback_guard_message().to_string();
                    } else {
                        self.confirm_source_selection(uri);
                    }
                }
            }
            Err(error) => self.settings_failed(error),
        }
        true
    }
}

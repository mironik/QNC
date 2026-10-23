use super::*;

impl IngestApplication {
    pub(crate) fn settings_failed(&mut self, error: String) {
        self.project.fail(error);
        self.clear_project_view();
        self.show_project_status();
    }

    /// Nothing of a closed or another project stays on view.
    fn clear_project_view(&mut self) {
        self.stop_player();
        self.posters.reset(); // the clips go: a reopened project loads its posters again
        self.view.clips.clear();
        self.view.timeline = Default::default();
        self.reset_timeline_artifacts();
        self.view.clip_filter = ClipFilter::All;
        self.view.preview_clip_id = None;
        self.pending_source = None;
        self.view.ai_mining = false;
    }

    fn show_project_status(&mut self) {
        let status = self.project.status();
        (self.view.work_settings_loading, self.view.work_settings_ready) = (status.loading, status.ready);
        self.view.work_settings_error = status.error.clone();
    }

    pub fn refresh_active_project(&mut self) -> IngestDispatchResult {
        self.load_work_settings_inner(None, true)
    }

    /// Shown: the active project is checked again. Hidden: the player is closed so it
    /// neither plays nor reads media behind another surface.
    pub fn set_active(&mut self, active: bool) -> IngestDispatchResult {
        if !active {
            self.stop_player();
            return IngestDispatchResult::accepted(None, false);
        }
        if self.project.is_current() != Some(true) {
            return self.refresh_active_project();
        }
        self.request_missing_thumbnails();
        IngestDispatchResult::accepted(None, true)
    }

    pub(crate) fn load_work_settings(&mut self, pending_source: Option<String>) -> IngestDispatchResult {
        self.load_work_settings_inner(pending_source, false)
    }

    pub(crate) fn load_work_settings_inner(&mut self, pending_source: Option<String>, retain_loaded_workspace: bool) -> IngestDispatchResult {
        let others_busy = self.import.is_busy() || self.select.is_busy();
        let error_before = self.project.status().error.clone();
        let started = self.project.start(retain_loaded_workspace && pending_source.is_none(), others_busy);
        if let qnc_ingest_project::Start::Started { keep_view: false } = started {
            self.stop_player();
            self.cancel_thumbnail_load();
        }
        if matches!(started, qnc_ingest_project::Start::Started { .. }) {
            self.pending_source = pending_source;
        }
        if self.project.status().error.is_some() && self.project.status().error != error_before {
            self.clear_project_view(); // no active project reader, or the load did not start
        }
        self.show_project_status();
        match started {
            qnc_ingest_project::Start::Refused(error) => IngestDispatchResult::rejected(error),
            _ => IngestDispatchResult::accepted(None, true),
        }
    }

    pub(crate) fn poll_settings(&mut self) -> bool {
        let Some(result) = self.project.poll() else {
            return false;
        };
        let loaded = match result {
            Ok(loaded) => loaded,
            Err(error) => {
                self.settings_failed(error);
                return true;
            }
        };
        // A new project must never inherit the preceding project's selection/preview.
        if loaded.project_changed {
            self.clear_project_view();
            self.view.selected_source_uri = None;
            self.view.browser.chosen = Default::default();
        }
        self.show_project_status();
        let ai = self.project.plan().is_some_and(|plan| plan.settings.ai_enabled());
        (self.view.ai_mining, self.view.archive_original_available, self.view.archive_original) = (ai, false, false);
        let catalog_was_loaded = loaded.clips.is_some();
        if let Some(clips) = loaded.clips {
            let previous = self.view.clips.clone();
            self.view.clips = catalog::apply_catalog_rows(&self.view.clips, clips).clips;
            qnc_media_thumbnail::keep_loaded(&mut self.view.clips, &previous);
            self.start_thumbnail_load(qnc_media_thumbnail::missing(&self.view.clips));
        }
        if self.pending_source.is_none() && catalog_was_loaded {
            let source = loaded.source;
            self.view.selected_source_uri = source.as_ref().map(|source| source.uri.clone());
            self.view.browser.chosen = source.map(|source| qnc_source_browse::SourceMetadata { name: source.name, serial_number: source.serial_number, volume_name: source.volume_name }).unwrap_or_default();
        }
        self.wake_artifact_worker();
        if let Some(uri) = self.pending_source.take() {
            if self.playback_guard_active() {
                self.view.message = playback_guard_message().to_string();
            } else {
                self.confirm_source_selection(uri);
            }
        }
        true
    }
}

use crate::*;

impl IngestApplication {
    /// The user confirmed the source: read the project settings, then Select.
    pub(crate) fn on_dir_confirm(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        // From a key there is no payload: the folder the browser shows.
        let uri = match payload {
            IngestPayload::LocationUri(uri) => uri,
            _ => match self.view.browser.current_uri.clone().filter(|_| !self.view.browser.roots) {
                Some(uri) => uri,
                None => return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.missing_location_uri")),
            },
        };
        if self.browse.selected(&uri).is_none() {
            let error = self
                .selection_config_error
                .clone()
                .unwrap_or_else(|| qnc_ui_text::text("ingest.msg.pick_disk_or_folder").into());
            self.view.message = error.clone();
            return IngestDispatchResult::rejected(error);
        }
        self.load_work_settings(Some(uri))
    }

    pub(crate) fn confirm_source_selection(&mut self, uri: String) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if self.work_plan().is_none() {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.settings_unavailable"));
        }
        if qnc_contracts::parse_qnc_uri(&uri).is_err() {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.source_not_qnc_uri"));
        }
        let (browser, private) = (&self.view.browser, self.browse.selected_private_local_path(&uri));
        let record = qnc_ingest_select_run::source_record(&uri, source_kind_id(self.view.source_kind), &browser.chosen, &browser.path_label, private);

        if let Some(store) = self.store.as_mut() {
            match store.record_source_selection(&record) {
                Ok(session) => {
                    self.view.selected_source_uri = Some(uri.clone());
                    self.view.message = qnc_ui_text::fill("ingest.msg.source_selected_at", &[("time", &session.selected_at_utc)]);
                    self.start_selection(&uri)
                }
                Err(error) => {
                    self.view.message = error.clone();
                    IngestDispatchResult::rejected(error)
                }
            }
        } else {
            self.view.selected_source_uri = Some(uri.clone());
            self.view.message = qnc_ui_text::text("ingest.msg.source_selected").to_string();
            self.start_selection(&uri)
        }
    }

    pub(crate) fn start_selection(&mut self, uri: &str) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        let Some(selected) = self.browse.selected(uri) else {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.source_gone"));
        };
        let Some(config) = self.selection_config.clone() else { return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.no_select_config")) };
        let Some(target) = self.project.select_target().cloned() else { return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.catalog_unavailable")) };
        match self.select.start(config, selected, target) {
            Ok(()) => {
                self.view.select_warning_count = 0;
                self.view.command_busy = true;
                self.view.message = qnc_ui_text::text("ingest.msg.select_started").into();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.message = error.to_string();
                IngestDispatchResult::rejected(error.to_string())
            }
        }
    }
}

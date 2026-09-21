use crate::*;

impl IngestApplication {
    /// The user confirmed the source: read the project settings, then Select.
    pub(crate) fn on_dir_confirm(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        let IngestPayload::LocationUri(uri) = payload else {
            return IngestDispatchResult::rejected("Nedostaje QNC lokacijski URI.");
        };
        if self.browse.selected(&uri).is_none() {
            let error = self
                .selection_config_error
                .clone()
                .unwrap_or_else(|| "Odaberi disk ili mapu u browseru.".into());
            self.view.message = error.clone();
            return IngestDispatchResult::rejected(error);
        }
        self.load_work_settings(Some(uri))
    }

    pub(crate) fn confirm_source_selection(&mut self, uri: String) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if self.work_plan.is_none() {
            return IngestDispatchResult::rejected("Radne postavke nisu dostupne.");
        }
        if qnc_contracts::parse_qnc_uri(&uri).is_err() {
            return IngestDispatchResult::rejected("Odabir izvora nije QNC URI.");
        }
        let display_name = if !self.view.selected_source_name.trim().is_empty() {
            self.view.selected_source_name.clone()
        } else if !self.view.browser_path_label.trim().is_empty() {
            self.view.browser_path_label.clone()
        } else {
            uri.clone()
        };
        let record = SourceSelectionRecord {
            source_uri: uri.clone(),
            source_kind: source_kind_id(self.view.source_kind).to_string(),
            display_name,
            serial_number: self.view.selected_source_serial_number.clone(),
            volume_name: self.view.selected_source_volume_name.clone(),
            private_local_path: self.browse.selected_private_local_path(&uri),
        };

        if let Some(store) = self.store.as_mut() {
            match store.record_source_selection(&record) {
                Ok(session) => {
                    self.view.selected_source_uri = Some(uri.clone());
                    self.view.message = format!("Izvor je odabran: {}", session.selected_at_utc);
                    self.start_selection(&uri)
                }
                Err(error) => {
                    self.view.message = error.clone();
                    IngestDispatchResult::rejected(error)
                }
            }
        } else {
            self.view.selected_source_uri = Some(uri.clone());
            self.view.message = "Izvor je odabran.".to_string();
            self.start_selection(&uri)
        }
    }

    pub(crate) fn start_selection(&mut self, uri: &str) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        let Some(selected) = self.browse.selected(uri) else {
            return IngestDispatchResult::rejected("Odabrani izvor vise nije dostupan.");
        };
        let Some(config) = self.selection_config.clone() else {
            return IngestDispatchResult::rejected("Nema Select konfiguracije.");
        };
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected("Projektni katalog nije dostupan.");
        };
        match self
            .selection_session
            .start(config, selected, target, self.camera_registry.clone())
        {
            Ok(()) => {
                self.selection_events.reset();
                self.view.select_warning_count = 0;
                self.view.command_busy = true;
                self.view.message = "Select je pokrenut.".into();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.message = error.to_string();
                IngestDispatchResult::rejected(error.to_string())
            }
        }
    }
}

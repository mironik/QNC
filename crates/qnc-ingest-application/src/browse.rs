use super::*;

impl IngestApplication {
    pub(crate) fn browse_registered(
        &mut self,
        kind: SourceKind,
        target: Option<Option<String>>,
    ) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if !self.browse.is_connected() {
            return IngestDispatchResult::rejected("Izvor nije povezan.");
        }
        self.view.source_kind = kind;
        let step = match target {
            None => {
                self.clear_source_browser_state();
                qnc_source_browse::Step::Roots(
                    match kind {
                        SourceKind::Internet => "intranet",
                        _ => source_kind_id(kind),
                    }
                    .into(),
                )
            }
            Some(None) => qnc_source_browse::Step::Parent,
            Some(Some(uri)) => qnc_source_browse::Step::Open(uri),
        };
        self.view.browser_busy = true;
        match self.browse.start(step) {
            Ok(()) => IngestDispatchResult::accepted(None, true),
            Err(error) => {
                self.view.browser_busy = false;
                IngestDispatchResult::rejected(error)
            }
        }
    }

    pub(crate) fn clear_source_browser_state(&mut self) {
        self.pending_source = None;
        self.view.browser_roots = true;
        self.view.browser_path_label.clear();
        self.view.browser_current_uri = None;
        self.view.browser_parent_available = false;
        self.view.browser_entries.clear();
        self.view.browser_busy = false;
        self.view.browser_error = None;
        self.view.selected_source_uri = None;
        self.view.selected_source_name.clear();
        self.view.selected_source_serial_number.clear();
        self.view.selected_source_volume_name.clear();
    }

    pub(crate) fn apply_source_browser_result(
        &mut self,
        result: Result<qnc_source_browse::BrowseState, String>,
    ) -> IngestDispatchResult {
        match result {
            Ok(state) => {
                self.view.browser_roots = state.roots;
                self.view.browser_path_label = state.path_label;
                self.view.browser_current_uri = state.current_uri;
                self.view.browser_parent_available = state.parent_available;
                self.view.browser_entries = state
                    .entries
                    .into_iter()
                    .map(|entry| LocationEntry {
                        name: entry.name,
                        qnc_uri: entry.qnc_uri,
                        serial_number: entry.serial_number,
                        volume_name: entry.volume_name,
                    })
                    .collect();
                self.view.browser_busy = false;
                self.view.browser_error = None;
                self.view.message = "Odaberi lokalni izvor.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.browser_busy = false;
                self.view.browser_error = Some(error.clone());
                self.view.message = error.clone();
                IngestDispatchResult::rejected(error)
            }
        }
    }

    pub(crate) fn capture_selected_source_metadata(&mut self, uri: &str) {
        if let Some(metadata) = qnc_source_browse::selected_metadata(
            &self.view.browser_entries,
            uri,
            self.view.selected_source_name.trim().is_empty(),
        ) {
            self.view.selected_source_name = metadata.name;
            self.view.selected_source_serial_number = metadata.serial_number;
            self.view.selected_source_volume_name = metadata.volume_name;
        }
    }
}

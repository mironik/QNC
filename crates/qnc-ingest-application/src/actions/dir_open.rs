use crate::*;

impl IngestApplication {
    /// Opens the folder or disk the user chose.
    pub(crate) fn on_dir_open(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        let IngestPayload::LocationUri(uri) = payload else {
            return IngestDispatchResult::rejected("Nedostaje QNC lokacijski URI.");
        };
        if self.browse.is_connected() {
            self.capture_selected_source_metadata(&uri);
            return self.browse_registered(self.view.source_kind, Some(Some(uri)));
        }
        if self.view.source_kind != SourceKind::Local {
            return IngestDispatchResult::accepted(None, true);
        }
        self.pending_source = None;
        self.capture_selected_source_metadata(&uri);
        let result = self.source_browser.open_uri(&uri);
        self.apply_source_browser_result(result)
    }
}

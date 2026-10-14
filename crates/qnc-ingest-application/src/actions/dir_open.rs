use crate::*;

impl IngestApplication {
    /// Opens the folder or disk the user chose.
    pub(crate) fn on_dir_open(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        let IngestPayload::LocationUri(uri) = payload else {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.missing_location_uri"));
        };
        if self.browse.is_connected() {
            self.capture_selected_source_metadata(&uri);
            return self.browse_registered(self.view.source_kind, Some(Some(uri)));
        }
        IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.source_not_connected"))
    }
}

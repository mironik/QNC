use crate::*;

impl IngestApplication {
    /// One folder up.
    pub(crate) fn on_dir_up(&mut self) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(self.view.source_kind, Some(None));
        }
        IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.source_not_connected"))
    }
}

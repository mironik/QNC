use crate::*;

impl IngestApplication {
    /// Leaves the source browser.
    pub(crate) fn on_dir_cancel(&mut self) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(self.view.source_kind, None);
        }
        self.clear_source_browser_state();
        self.view.message = "Odabir izvora je otkazan.".to_string();
        IngestDispatchResult::accepted(None, true)
    }
}

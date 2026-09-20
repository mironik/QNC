use crate::*;

impl IngestApplication {
    /// Leaves the source browser.
    pub(crate) fn on_dir_cancel(&mut self) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(self.view.source_kind, None);
        }
        self.cancel_source_browser()
    }

    pub(crate) fn cancel_source_browser(&mut self) -> IngestDispatchResult {
        self.view.source_kind = SourceKind::Local;
        self.clear_source_browser_state();
        let result = self.source_browser.load_roots();
        let mut dispatch = self.apply_source_browser_result(result);
        self.view.message = "Odabir izvora je otkazan.".to_string();
        dispatch.message = None;
        dispatch
    }
}

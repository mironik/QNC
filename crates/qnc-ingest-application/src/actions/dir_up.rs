use crate::*;

impl IngestApplication {
    /// One folder up.
    pub(crate) fn on_dir_up(&mut self) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(self.view.source_kind, Some(None));
        }
        if self.view.source_kind == SourceKind::Local {
            let result = self.source_browser.open_parent();
            self.apply_source_browser_result(result)
        } else {
            IngestDispatchResult::accepted(None, true)
        }
    }
}

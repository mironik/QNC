use crate::*;

impl IngestApplication {
    /// Back to the list of disks.
    pub(crate) fn on_dir_roots(&mut self) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(self.view.source_kind, None);
        }
        if self.view.source_kind == SourceKind::Local {
            let result = self.source_browser.load_roots();
            self.apply_source_browser_result(result)
        } else {
            IngestDispatchResult::accepted(None, true)
        }
    }
}

use crate::*;

impl IngestApplication {
    /// Local, LAN or intranet: which kind of source the browser lists.
    pub(crate) fn on_source_kind(&mut self, kind: SourceKind) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(kind, None);
        }
        self.view.source_kind = kind;
        match kind {
            SourceKind::Local => {
                let result = self.source_browser.load_roots();
                self.apply_source_browser_result(result)
            }
            SourceKind::Lan => {
                self.clear_source_browser_state();
                self.view.message = "LAN izvor nije povezan u ovom rezu.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            SourceKind::Internet => {
                self.clear_source_browser_state();
                self.view.message = "Internet izvor nije povezan u ovom rezu.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
        }
    }
}

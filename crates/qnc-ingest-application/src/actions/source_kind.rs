use crate::*;

impl IngestApplication {
    /// Local, LAN or intranet: which kind of source the browser lists.
    pub(crate) fn on_source_kind(&mut self, kind: SourceKind) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(kind, None);
        }
        self.view.source_kind = kind;
        self.clear_source_browser_state();
        self.view.source_kind = kind;
        self.view.message = "Izvor nije povezan.".to_string();
        IngestDispatchResult::rejected("Izvor nije povezan.")
    }
}

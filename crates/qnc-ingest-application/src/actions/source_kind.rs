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
        self.view.message = qnc_ui_text::text("ingest.msg.source_not_connected").to_string();
        IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.source_not_connected"))
    }
}

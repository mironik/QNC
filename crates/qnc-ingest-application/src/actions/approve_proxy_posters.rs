use crate::*;

impl IngestApplication {
    /// The import worker creates missing posters as part of the import outcome. A standalone
    /// approval path needs its own write transport operation, so it must not silently pretend
    /// to run through the generic fallback.
    pub(crate) fn on_approve_proxy_posters(&mut self) -> IngestDispatchResult {
        let count = self.view.proxy_poster_approval_count();
        if count == 0 {
            return IngestDispatchResult::rejected("Nema odabranih klipova bez postera.");
        }
        IngestDispatchResult::rejected(
            "Samostalno generiranje postera jos nije spojeno; posteri se generiraju tijekom uvoza.",
        )
    }
}

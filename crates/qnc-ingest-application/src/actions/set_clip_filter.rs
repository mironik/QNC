use crate::*;

impl IngestApplication {
    /// Which clips the grid shows.
    pub(crate) fn on_set_clip_filter(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        match payload {
            IngestPayload::ClipFilter(filter) => {
                self.view.clip_filter = filter;
                IngestDispatchResult::accepted(None, true)
            }
            _ => IngestDispatchResult::rejected("Nedostaje filter klipova."),
        }
    }
}

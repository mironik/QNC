use crate::*;

impl IngestApplication {
    /// Which clips the grid shows.
    pub(crate) fn on_set_clip_filter(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        match payload {
            IngestPayload::ClipFilter(filter) => {
                self.view.clip_filter = filter;
                IngestDispatchResult::accepted(None, true)
            }
            // From a key there is no payload: New and All switch.
            _ => self.on_set_clip_filter(IngestPayload::ClipFilter(if self.view.clip_filter == ClipFilter::New { ClipFilter::All } else { ClipFilter::New })),
        }
    }
}

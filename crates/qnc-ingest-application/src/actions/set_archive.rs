use crate::*;

impl IngestApplication {
    /// The "archive the original" switch.
    pub(crate) fn on_set_archive(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        match payload {
            IngestPayload::Bool(value) => {
                self.view.archive_original = value;
                IngestDispatchResult::accepted(None, true)
            }
            _ => IngestDispatchResult::rejected("Nedostaje bool vrijednost."),
        }
    }
}

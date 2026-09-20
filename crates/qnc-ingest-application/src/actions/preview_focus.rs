use crate::*;

impl IngestApplication {
    /// The user chose a clip to preview.
    pub(crate) fn on_preview_focus(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        match payload {
            IngestPayload::ClipId(clip_id) => self.prepare_preview(clip_id),
            _ => IngestDispatchResult::rejected("Nedostaje clip_id."),
        }
    }
}

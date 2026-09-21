use crate::*;

impl IngestApplication {
    /// Marks every visible clip.
    pub(crate) fn on_select_all(&mut self) -> IngestDispatchResult {
        let ids = self
            .view
            .visible_clips()
            .map(|c| c.clip_id.clone())
            .collect();
        self.select_clips(ids, true)
    }
}

use crate::*;

impl IngestApplication {
    /// Removes the mark from every visible clip.
    pub(crate) fn on_clear_selection(&mut self) -> IngestDispatchResult {
        let ids = self.view.visible_clips().map(|c| c.clip_id.clone()).collect();
        self.select_clips(ids, false)
    }
}

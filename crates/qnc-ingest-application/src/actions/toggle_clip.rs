use crate::*;

impl IngestApplication {
    /// The checkbox of one clip.
    pub(crate) fn on_toggle_clip(&mut self, payload: IngestPayload) -> IngestDispatchResult {
        let IngestPayload::ClipId(clip_id) = payload else {
            return IngestDispatchResult::rejected("Nedostaje clip_id.");
        };
        match self.view.clips.iter().find(|clip| clip.clip_id == clip_id) {
            Some(clip) => {
                let selected = !clip.selected;
                self.select_clips(vec![clip_id], selected)
            }
            None => IngestDispatchResult::rejected("Clip nije pronađen."),
        }
    }

    /// Selecting clips is only a mark in the cache of the view: no thread, no database.
    /// The database sees the selection when the user starts the import.
    pub(crate) fn select_clips(
        &mut self,
        ids: Vec<String>,
        selected: bool,
    ) -> IngestDispatchResult {
        for clip in &mut self.view.clips {
            if ids.contains(&clip.clip_id) {
                clip.selected = selected;
            }
        }
        IngestDispatchResult::accepted(None, true)
    }
}

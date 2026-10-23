//! The generic pass of the application through the pieces of the form (piece
//! `qnc-form-pieces`, the list `qnc-ingest-pieces`): a new possibility is a new piece.

use super::*;
use qnc_form_pieces::{Answer, Now};

impl IngestApplication {
    /// The pieces and what they see of the form.
    fn pieces(&mut self) -> (&mut qnc_form_pieces::Pieces, Now<'_>) {
        let marks = self.view.timeline.visible_source_marks();
        let view = &mut self.view;
        let project = self.project.select_target().map(|target| &target.records);
        let only_new = view.clip_filter == ClipFilter::New;
        let now = Now { clips: &mut view.clips, tab: &mut view.pool_tab, only_new, chosen: view.preview_clip_id.as_deref(), marks, project };
        (&mut self.pieces, now)
    }

    /// The answer of the first piece that takes the action, `None` when none does.
    pub(crate) fn piece_act(&mut self, intent: &IngestIntent) -> Option<IngestDispatchResult> {
        let payload = if let IngestPayload::ClipId(id) = &intent.payload { Some(id.as_str()) } else { None };
        let (pieces, mut now) = self.pieces();
        let answer = pieces.act(&intent.action_id, payload, &mut now)?;
        Some(self.answered(answer))
    }

    pub(crate) fn piece_poll(&mut self) -> bool {
        let (pieces, mut now) = self.pieces();
        let answer = pieces.poll(&mut now);
        self.answered(answer).request_repaint
    }

    fn answered(&mut self, answer: Answer) -> IngestDispatchResult {
        self.view.pieces = self.pieces.shown(self.view.pool_tab.as_deref(), &self.view.clips);
        if let Some((clip_id, marks)) = answer.open {
            self.preview.mark(&clip_id, marks);
            return self.prepare_preview(clip_id);
        }
        match answer.message {
            Some(message) if answer.refused => self.refuse_shown(message),
            Some(message) => {
                self.view.message = message;
                IngestDispatchResult::accepted(None, true)
            }
            None => IngestDispatchResult::accepted(None, answer.changed),
        }
    }
}

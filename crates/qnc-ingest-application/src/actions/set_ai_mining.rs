use crate::*;

impl IngestApplication {
    /// The AI setting comes from the project database; the form cannot override it.
    pub(crate) fn on_set_ai_mining(&mut self) -> IngestDispatchResult {
        IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.ai_from_project"))
    }
}

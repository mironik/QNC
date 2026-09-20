use crate::*;

impl IngestApplication {
    /// The AI setting comes from the project database; the form cannot override it.
    pub(crate) fn on_set_ai_mining(&mut self) -> IngestDispatchResult {
        IngestDispatchResult::rejected("AI postavka dolazi iz baze; nema lokalnog overridea.")
    }
}

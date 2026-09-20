use crate::*;

impl IngestApplication {
    /// Reads the active project and its catalog again.
    pub(crate) fn on_reload(&mut self) -> IngestDispatchResult {
        self.load_work_settings(None)
    }
}

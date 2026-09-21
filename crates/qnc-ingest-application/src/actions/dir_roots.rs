use crate::*;

impl IngestApplication {
    /// Back to the list of disks.
    pub(crate) fn on_dir_roots(&mut self) -> IngestDispatchResult {
        if self.browse.is_connected() {
            return self.browse_registered(self.view.source_kind, None);
        }
        IngestDispatchResult::rejected("Izvor nije povezan.")
    }
}

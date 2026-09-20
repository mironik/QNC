//! Hand-over to the next application. Uvezi is finished on this form once the import is
//! running: the application only says so, the shell decides which application follows.

use super::*;

pub use qnc_application_sequence::SequenceStep;

impl IngestApplication {
    /// The import has started and the shell may open the next application. Handed over
    /// once; the copying goes on in the background while the form is not shown.
    pub(super) fn request_navigation_after_import(&mut self) {
        self.stop_player();
        self.importer.set_paused(false);
        self.navigation_requested = true;
    }

    pub fn take_navigation_request(&mut self) -> bool {
        std::mem::take(&mut self.navigation_requested)
    }

    /// The order of the project's applications, from its public database view.
    pub fn navigation_sequence(&self) -> Result<Vec<SequenceStep>, String> {
        let reader = self
            .settings_reader
            .as_ref()
            .ok_or("Nema citaca radnih postavki.")?;
        let plan = self.work_plan.as_ref().ok_or("Radne postavke nisu dostupne.")?;
        qnc_application_sequence::read(reader, &plan.settings)
    }
}

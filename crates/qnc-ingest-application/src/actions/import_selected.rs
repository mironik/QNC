use crate::*;

impl IngestApplication {
    /// Uvezi: the writer checks the marked clips against what the project uses, writes the
    /// selection to the database once and then the import starts in the background. What is
    /// copied and where is decided by the project settings in the work plan; the media is read
    /// through the transport of its source (local, LAN or intranet).
    pub(crate) fn start_import(&mut self) -> IngestDispatchResult {
        let Some(plan) = self.project.plan() else {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.settings_unavailable"));
        };
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        let writing = self.import.start(&plan.settings, self.project.catalog_target().cloned(), &self.view.clips, !self.view.pieces.ready.is_empty());
        self.started(writing)
    }

    /// The answer to the writer's choice (import the others, or cancel).
    pub(crate) fn answer_import(&mut self, action_id: &str) -> IngestDispatchResult {
        let answer = self.import.answer(action_id);
        self.view.import_choice = None;
        self.started(answer)
    }

    fn started(&mut self, writing: Result<bool, String>) -> IngestDispatchResult {
        match writing {
            Ok(writing) => {
                self.view.command_busy = writing;
                self.view.message = if writing { qnc_ui_text::text("ingest.msg.saving_selection").into() } else { String::new() };
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => IngestDispatchResult::rejected(error),
        }
    }

    /// The selection is in the database: the background application takes over. It reads
    /// the selected clips and the project settings and does what the settings say.
    pub(crate) fn begin_import(&mut self) -> Result<(), String> {
        self.import.wake(self.root.as_deref(), self.project.catalog_target())
    }
}

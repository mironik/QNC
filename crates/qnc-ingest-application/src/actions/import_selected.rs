use crate::*;

impl IngestApplication {
    /// Uvezi: the writer checks the marked clips against what the project uses, writes the
    /// selection to the database once and then the import starts in the background. What is
    /// copied and where is decided by the project settings in the work plan; the media is read
    /// through the transport of its source (local, LAN or intranet).
    pub(crate) fn start_import(&mut self) -> IngestDispatchResult {
        let Some(plan) = &self.work_plan else {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.settings_unavailable"));
        };
        let needs = qnc_import_fit::Needs::of(&plan.settings);
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if self.selection_writer.is_busy() {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.import_running"));
        }
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.catalog_unavailable"));
        };
        let writing = match needs {
            Ok(needs) => self.selection_writer.start_import(target, &self.view.clips, needs),
            Err(error) => Err(error),
        };
        self.started(writing.map(|()| true))
    }

    /// The answer to the writer's choice (import the others, or cancel).
    pub(crate) fn answer_import(&mut self, action_id: &str) -> IngestDispatchResult {
        let answer = self.selection_writer.answer(action_id).unwrap_or(Ok(false));
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
        let root = self.root.clone().ok_or(qnc_ui_text::text("ingest.msg.import_no_root"))?;
        let target = self.catalog_target.clone().ok_or(qnc_ui_text::text("ingest.msg.catalog_unavailable"))?;
        self.worker.start(&root, &target)
    }
}

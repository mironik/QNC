use crate::*;

impl IngestApplication {
    /// Uvezi: checks the marked clips, writes the selection to the database once and then
    /// starts the import in the background. What is copied and where is decided by the
    /// project settings in the work plan; the media is read through the transport of its
    /// source (local, LAN or intranet) and copied into the project folder of this machine.
    pub(crate) fn start_import(&mut self) -> IngestDispatchResult {
        if self.work_plan.is_none() {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.settings_unavailable"));
        }
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if self.selection_writer.is_busy() {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.import_running"));
        }
        let selected: Vec<String> = self
            .view
            .clips
            .iter()
            .filter(|clip| clip.selected)
            .map(|clip| clip.clip_id.clone())
            .collect();
        if selected.is_empty() {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.no_clips_selected"));
        }
        if self
            .view
            .clips
            .iter()
            .any(|clip| clip.selected && clip.save_state != SaveState::Saved)
        {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.clip_not_saved"));
        }
        let unselected: Vec<String> = self
            .view
            .clips
            .iter()
            .filter(|clip| !clip.selected && clip.save_state == SaveState::Saved)
            .map(|clip| clip.clip_id.clone())
            .collect();
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.catalog_unavailable"));
        };
        match self.selection_writer.start(target, selected, unselected) {
            Ok(()) => {
                self.view.command_busy = true;
                self.view.message = qnc_ui_text::text("ingest.msg.saving_selection").into();
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

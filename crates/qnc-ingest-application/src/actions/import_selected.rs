use crate::*;

impl IngestApplication {
    /// Uvezi: checks the marked clips, writes the selection to the database once and then
    /// starts the import in the background. What is copied and where is decided by the
    /// project settings in the work plan; the media is read through the transport of its
    /// source (local, LAN or intranet) and copied into the project folder of this machine.
    pub(crate) fn start_import(&mut self) -> IngestDispatchResult {
        if self.work_plan.is_none() {
            return IngestDispatchResult::rejected("Radne postavke nisu dostupne.");
        }
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if self.selection_writer.is_busy() {
            return IngestDispatchResult::rejected("Uvoz je vec u tijeku.");
        }
        let selected: Vec<String> = self
            .view
            .clips
            .iter()
            .filter(|clip| clip.selected)
            .map(|clip| clip.clip_id.clone())
            .collect();
        if selected.is_empty() {
            return IngestDispatchResult::rejected("Nema odabranih klipova.");
        }
        if self
            .view
            .clips
            .iter()
            .any(|clip| clip.selected && clip.save_state != SaveState::Saved)
        {
            return IngestDispatchResult::rejected("Klip jos nije spremljen u bazu.");
        }
        let unselected: Vec<String> = self
            .view
            .clips
            .iter()
            .filter(|clip| !clip.selected && clip.save_state == SaveState::Saved)
            .map(|clip| clip.clip_id.clone())
            .collect();
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected("Projektni katalog nije dostupan.");
        };
        match self.selection_writer.start(target, selected, unselected) {
            Ok(()) => {
                self.view.command_busy = true;
                self.view.message = "Spremanje odabira...".into();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => IngestDispatchResult::rejected(error),
        }
    }

    /// The selection is in the database: the background application takes over. It reads
    /// the selected clips and the project settings and does what the settings say.
    pub(crate) fn begin_import(&mut self) -> Result<(), String> {
        let root = self
            .root
            .clone()
            .ok_or("Uvoz nije dostupan: nema korijena aplikacije.")?;
        let target = self
            .catalog_target
            .clone()
            .ok_or("Projektni katalog nije dostupan.")?;
        self.worker.start(&root, &target)
    }
}

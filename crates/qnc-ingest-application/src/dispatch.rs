//! The table of actions: what the user asked for is handed to the file of that action.

use super::*;

impl IngestApplication {
    pub fn dispatch(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        self.dispatch_log.push(intent.action_id.clone());
        if let Some(refused) = self.refuse(&intent.action_id) {
            return refused;
        }
        match intent.action_id.as_str() {
            action_ids::INGEST_RELOAD => self.on_reload(),
            action_ids::INGEST_SET_CLIP_FILTER => self.on_set_clip_filter(intent.payload),
            action_ids::INGEST_SOURCE_KIND_LOCAL => self.on_source_kind(SourceKind::Local),
            action_ids::INGEST_SOURCE_KIND_LAN => self.on_source_kind(SourceKind::Lan),
            action_ids::INGEST_SOURCE_KIND_INTERNET => self.on_source_kind(SourceKind::Internet),
            action_ids::INGEST_DIR_ROOTS => self.on_dir_roots(),
            action_ids::INGEST_DIR_UP => self.on_dir_up(),
            action_ids::INGEST_DIR_OPEN => self.on_dir_open(intent.payload),
            action_ids::INGEST_DIR_CONFIRM => self.on_dir_confirm(intent.payload),
            action_ids::INGEST_DIR_CANCEL => self.on_dir_cancel(),
            action_ids::INGEST_SELECT_ALL => self.on_select_all(),
            action_ids::INGEST_CLEAR_SELECTION => self.on_clear_selection(),
            action_ids::INGEST_CLIP_TOGGLE => self.on_toggle_clip(intent.payload),
            action_ids::INGEST_PREVIEW_FOCUS => self.on_preview_focus(intent.payload),
            action_ids::INGEST_SET_ARCHIVE => self.on_set_archive(intent.payload),
            action_ids::INGEST_SET_AI_MINING => self.on_set_ai_mining(),
            action_ids::INGEST_APPROVE_PROXY_POSTERS => self.on_approve_proxy_posters(),
            action_ids::INGEST_IMPORT_SELECTED => self.start_import(),
            action_ids::PLAY_PAUSE
            | action_ids::STEP_BACK_FRAME
            | action_ids::STEP_FORWARD_FRAME
            | action_ids::INGEST_CUE_FRAME => self.player_action(intent),
            _ => IngestDispatchResult::accepted(
                Some("Akcija je zapisana, komponenta za izvršenje još nije spojena.".to_string()),
                true,
            ),
        }
    }

    /// Whether the action may not run now: the player has the priority, or the source
    /// is still being processed.
    fn refuse(&mut self, action_id: &str) -> Option<IngestDispatchResult> {
        if self.playback_guard_active() && Self::playback_guard_blocks_action(action_id) {
            return Some(self.playback_guard_rejected());
        }
        let source_action = matches!(
            action_id,
            action_ids::INGEST_RELOAD
                | action_ids::INGEST_SOURCE_KIND_LOCAL
                | action_ids::INGEST_SOURCE_KIND_LAN
                | action_ids::INGEST_SOURCE_KIND_INTERNET
                | action_ids::INGEST_DIR_ROOTS
                | action_ids::INGEST_DIR_UP
                | action_ids::INGEST_DIR_OPEN
                | action_ids::INGEST_DIR_CONFIRM
                | action_ids::INGEST_DIR_CANCEL
        );
        let busy = self.selection_writer.is_busy()
            || self.view.command_busy
            || self.view.browser_busy
            || (self.pending_source.is_some() && action_id != action_ids::INGEST_DIR_CANCEL);
        (source_action && busy)
            .then(|| IngestDispatchResult::rejected("Obrada odabranog izvora je u tijeku."))
    }
}

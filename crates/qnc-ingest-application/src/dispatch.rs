//! The table of actions: what the user asked for is handed to the file of that action.

use super::*;

impl IngestApplication {
    pub fn dispatch(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        // The A1/A2 lanes of the one source timeline (piece `qnc-source-timeline`): Ctrl+1/2, the
        // keys of a taken lane and a click on a channel; the keyboard goes to the source timeline.
        let value = if let IngestPayload::Frame(value) = intent.payload { Some(value) } else { None };
        if let Some(changed) = self.source_timeline.route_input(&intent.action_id, value, self.preview.source_preview_mut(), &mut self.view.timeline) {
            (self.view.focus, self.view.message) = (self.walk.focus(qnc_focus_walk::SOURCE_TIMELINE), self.source_timeline.view().message.clone());
            return IngestDispatchResult::accepted(None, changed);
        }
        // The keys act on the panel in focus (user 2026-10-13): taken there, or standing for a tab or a button.
        let routed = self.walk.route(&intent.action_id);
        self.view.focus = self.walk.view();
        trace(|| format!("ingest-dispatch action={} walk={routed:?} panel={}", intent.action_id, self.view.focus.panel));
        let intent = match routed {
            qnc_focus_walk::Walk::Pass => intent,
            qnc_focus_walk::Walk::Done(changed) => return IngestDispatchResult::accepted(None, changed),
            qnc_focus_walk::Walk::Action(action_id) => IngestIntent::empty(action_id),
        };
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
            action_ids::INGEST_IMPORT_SELECTED => self.start_import(),
            action if action == action_ids::INGEST_CUE_FRAME || qnc_source_panel::ACTIONS.contains(&action) || action.starts_with("wheel_source") => self.player_action(intent),
            tab @ (action_ids::INGEST_TAB_ALL | action_ids::INGEST_TAB_VIRTUAL) => { self.view.pool_tab = Some(tab.to_string()); IngestDispatchResult::accepted(None, true) } // v5: only the chosen tab changes
            _ => IngestDispatchResult::accepted(Some(qnc_ui_text::text("ingest.msg.action_not_connected").to_string()), true),
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
            || self.view.browser.busy
            || (self.pending_source.is_some() && action_id != action_ids::INGEST_DIR_CANCEL);
        (source_action && busy)
            .then(|| IngestDispatchResult::rejected(qnc_ui_text::text("ingest.msg.source_busy")))
    }
}

/// What the user asked for and what became of it, in the player log (dev diagnostics only).
pub(crate) fn trace(line: impl FnOnce() -> String) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line());
    }
}

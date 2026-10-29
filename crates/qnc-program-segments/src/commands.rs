use super::*;

impl ProgramSegments {
    /// Whether a keyboard catalog action belongs to the program.
    pub fn handles(&self, action_id: &str) -> bool {
        self.command_for(action_id).is_some()
            || ((self.slot_move.is_some() || self.slot_trim.is_some())
                && matches!(action_id, "step_back_frame" | "step_forward_frame" | "clear_focus"))
    }

    /// Applies a keyboard catalog action of the program; false if it is not one.
    pub fn apply_action(&mut self, action_id: &str) -> bool {
        if let Some(done) = self.trim_key(action_id) {
            return done; // a taken slot edge (segment panel)
        }
        if let Some(done) = self.slot_key(action_id) {
            return done; // a taken slot (select_current_marker_slot)
        }
        self.command_for(action_id)
            .is_some_and(|command| self.apply(command))
    }

    /// While a marker is edited, the arrows and Escape belong to it.
    pub(crate) fn command_for(&self, action_id: &str) -> Option<SegmentCommand> {
        if self.cover_taken.is_some() && action_id == "clear_focus" {
            return Some(SegmentCommand::CancelMarkerEdit);
        }
        if self.editing.is_some() && action_id == "clear_focus" {
            return Some(SegmentCommand::CancelEdit);
        }
        if self.marker_edit.is_some() {
            match action_id {
                "step_back_frame" => return Some(SegmentCommand::NudgeMarker(-1)),
                "step_forward_frame" => return Some(SegmentCommand::NudgeMarker(1)),
                "clear_focus" => return Some(SegmentCommand::CancelMarkerEdit),
                _ => {}
            }
        }
        // Escape with nothing taken closes an enlarged slot.
        // Only `clear_focus`: Escape also gives `close_player` (two catalog actions on one key, live
        // 2026-10-21: one Escape let a marker go and closed the zoom too).
        if self.zoom_slot.is_some() && action_id == "clear_focus" && self.held_nothing() {
            return Some(SegmentCommand::CloseZoom);
        }
        SegmentCommand::from_action(action_id)
    }

    /// Applies a command; returns true so the caller repaints.
    /// A command of the segment panel (a click, a right click, a key). Every one but the wheel
    /// goes to the player log with what came of it (audit 2026-10-17: the clicks on Wrap rows
    /// left no trace, only the keys did).
    pub fn apply(&mut self, command: SegmentCommand) -> bool {
        let shown = (!matches!(command, SegmentCommand::Wheel { .. }) && qnc_dev_diagnostics::player_diagnostics_enabled())
            .then(|| format!("{command:?}"));
        let done = self.apply_command(command);
        if let Some(command) = shown {
            let taken = format!(
                "trim={} move={} marker={} sound={}",
                self.slot_trim.is_some(),
                self.slot_move.is_some(),
                self.marker_edit.is_some(),
                self.part_sound_taken()
            );
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!("story-click {command} done={done} {taken} slot={:?} message={:?}", self.selected_slot, self.view.message),
            );
        }
        done
    }

    pub(crate) fn apply_command(&mut self, command: SegmentCommand) -> bool {
        // Nothing new is taken while something else waits for Enter (user 2026-10-21).
        if let Some(wanted) = self.take_of(&command) {
            if self.refused(wanted) {
                return true;
            }
        }
        let playhead = self.playhead.unwrap_or(0);
        if matches!(
            command,
            SegmentCommand::Replace(_)
                | SegmentCommand::Exclude(_)
                | SegmentCommand::Purge(_)
                | SegmentCommand::AddSegment(_)
                | SegmentCommand::Undo
                | SegmentCommand::Redo
                | SegmentCommand::DeleteSelected
                | SegmentCommand::Move { .. }
        ) {
            self.end_edit();
        }
        match command {
            SegmentCommand::Wheel { .. } => return false, // the form turns it into the panel's arrow
            SegmentCommand::Select(segment_id) => self.select(&segment_id),
            SegmentCommand::Step { up } => self.step(up),
            SegmentCommand::Move { up } => self.move_selected(up),
            SegmentCommand::Exclude(segment_id) => {
                self.write(Operation::DeleteSegment { segment_id });
            }
            SegmentCommand::Include(segment_id) => {
                self.adopt_selection = true;
                self.write(Operation::IncludeSegment { segment_id });
            }
            SegmentCommand::Purge(segment_id) => self.write(Operation::PurgeSegment { segment_id }),
            SegmentCommand::ToggleWrapVideo => self.video_compact = !self.video_compact,
            SegmentCommand::ExportChoose(index) => {
                if let Some(export) = self.export.as_mut() {
                    export.choose(index);
                }
            }
            SegmentCommand::ExportTab(ntsc) => self.export_tab(ntsc),
            SegmentCommand::ExportCancel => self.export = None,
            SegmentCommand::ExportConfirm => {
                let Some(picked) = self.export.take().and_then(|export| export.picked()) else {
                    return true;
                };
                // Every export runs in the background (`qnc-export`), one at a time: a running
                // one goes on. Original and the settings are rendered from the originals.
                let _ = match picked.kind {
                    qnc_export_choice::Kind::Xml => self.export_run.start(&["--xml"]),
                    qnc_export_choice::Kind::Express => self.export_run.start(&["--express"]),
                    qnc_export_choice::Kind::Original | qnc_export_choice::Kind::Setting => self.export_run.start(&["--render", &picked.id]),
                };
                // "Exporting" with the format's name, red until it is done (user 2026-10-24).
                let running = qnc_ui_text::fill("export.running", &[("format", &picked.label)]);
                self.export_alert = Some(running.clone());
                self.refresh_view(running);
            }
            SegmentCommand::ToggleNormalize => {
                self.normalize = !self.normalize;
                let measured = self.loudness.as_ref().map(|(_, measured)| *measured).unwrap_or_default();
                let lufs = |value: Option<f64>| value.map_or("--".to_string(), |lufs| format!("{lufs:.1}"));
                let state = if self.normalize { "on" } else { "off" };
                qnc_dev_diagnostics::log_line(
                    qnc_dev_diagnostics::DiagnosticsStream::Player,
                    format!("story-normalize {state} a1_lufs={:?} a2_lufs={:?}", measured.a1_lufs, measured.a2_lufs),
                );
                let message = match self.normalize {
                    true => qnc_ui_text::fill("story.msg.normalize_on", &[("a1", &lufs(measured.a1_lufs)), ("a2", &lufs(measured.a2_lufs))]),
                    false => qnc_ui_text::text("story.msg.normalize_off").to_string(),
                };
                self.refresh_view(message);
            }
            SegmentCommand::Zoom { slot, frame } => self.zoom(slot, frame),
            SegmentCommand::ProgramZoom => self.zoom_program(),
            SegmentCommand::CloseZoom => {
                self.zoom_slot = None;
                self.refresh_view(String::new());
            }
            SegmentCommand::Undo | SegmentCommand::Redo => {
                // Everything taken goes: its frames belong to the story before the undo (audit
                // 2026-10-25: a taken slot moved with stale frames after Undo).
                self.let_go_edits(false);
                self.adopt_selection = true;
                self.write(if matches!(command, SegmentCommand::Undo) {
                    Operation::UndoStory
                } else {
                    Operation::RedoStory
                });
            }
            SegmentCommand::Replace(segment_id) => {
                if let Some((clip_id, (in_frame, out_frame), (fps_num, fps_den))) =
                    self.marked_source()
                {
                    self.write(Operation::ReplaceSegment {
                        segment_id,
                        clip_id,
                        in_frame,
                        out_frame,
                        fps_num,
                        fps_den,
                    });
                }
            }
            SegmentCommand::DeleteSelected => self.delete_selected(),
            SegmentCommand::SelectMarker(marker_id) => self.select_marker(&marker_id),
            SegmentCommand::TakeMarker(marker_id) => {
                self.select_marker(&marker_id);
                self.edit_marker();
            }
            SegmentCommand::TakeSlot(slot_id) => {
                self.take_slot(&slot_id);
                self.take_slot_move(&slot_id);
            }
            SegmentCommand::TakeSound { lane, frame } => {
                self.take_part_sound(lane, frame);
            }
            // The channel picked by mouse is the segment panel keys' (piece `qnc-segments-keys`:
            // the draft is heard at once); nothing to do here.
            SegmentCommand::PickChannel { .. } => return false,
            SegmentCommand::SelectSlot { slot_id, frame } => {
                self.drop_selected_cover();
                self.select_slot(&slot_id, Some(frame))
            }
            SegmentCommand::StepMarker { up } => {
                match neighbour_marker(&self.view.markers, playhead, up).cloned() {
                    Some(pin) => self.select_marker(&pin.marker_id),
                    None => self.refresh_view(qnc_ui_text::text("story.msg.no_adjacent_marker").into()),
                }
            }
            SegmentCommand::StepSlot { up } => {
                match neighbour_slot(&self.view.slots, playhead, up).cloned() {
                    Some(slot) => self.select_slot(&slot.slot_id, None),
                    None => self.refresh_view(qnc_ui_text::text("story.msg.no_adjacent_slot").into()),
                }
            }
            SegmentCommand::ProgramStart => self.cue_program(0),
            SegmentCommand::Cue(frame) => {
                // A click that only places the playhead lets go of a slot it leaves, so
                // Ctrl+I/O take the slot under the playhead.
                let inside = self.view.slots.iter().any(|slot| {
                    self.selected_slot.as_deref() == Some(slot.slot_id.as_str()) && (slot.start_frame..slot.end_frame).contains(&frame)
                });
                // The stored selection goes too, or a later reread brings it back (user
                // 2026-10-23: after Sync the B-roll button did nothing).
                if !inside && self.selected_slot.take().is_some() {
                    self.write(Operation::SelectSlot { slot_id: String::new() });
                }
                self.drop_selected_cover();
                self.cue_at(frame)
            }
            SegmentCommand::Marker => self.marker_at_playhead(),
            SegmentCommand::AddSegment(kind) => self.create_from_source(kind),
            SegmentCommand::Cover { overwrite } => self.cover_from_source(overwrite),
            SegmentCommand::SelectCover { cover_id, frame } => self.select_cover(cover_id, frame),
            SegmentCommand::TakeCover { cover_id, frame } => {
                self.select_cover(cover_id.clone(), frame);
                if self.selected_cover.as_deref() == Some(cover_id.as_str()) {
                    self.cover_taken = Some(cover_id);
                    self.refresh_view(qnc_ui_text::text("story.msg.cover_taken").into());
                }
            }
            SegmentCommand::ToggleSync => self.toggle_sync(),
            SegmentCommand::CoverMode { sync } => {
                self.covers_chosen = !sync && !self.view.rows.is_empty();
                if self.sync.enabled() != sync {
                    self.toggle_sync();
                } else {
                    self.refresh_view(String::new());
                }
                // The B-roll button (and P) also puts the marked source into the empty slot,
                // the chosen one or the one under the Wrap playhead, as B does (user 2026-10-17:
                // "the B-roll button does not work").
                // When it cannot, it says why instead of doing nothing.
                if !sync && self.source.marks.is_some() {
                    self.cover_from_source(false);
                }
            }
            SegmentCommand::CommitSync => {
                if !self.commit_marker_edit() && !self.commit_edit() {
                    if !self.finish_sync_with_cover() {
                        self.commit_sync(true);
                    }
                }
            }
            // Ctrl+M on the taken marker: its whole cut moves again (user 2026-10-21).
            SegmentCommand::EditMarker if self.marker_edit.is_some() => {
                self.whole_cut();
                self.refresh_view(qnc_ui_text::text("story.msg.marker_taken").into());
            }
            SegmentCommand::EditMarker => self.edit_marker(),
            SegmentCommand::CutPart { marker_id, picture, lane } => {
                return self.take_cut_part(Some(marker_id), picture, lane);
            }
            SegmentCommand::AddMarker => {
                self.marker_edit = None;
                self.selected_marker = None;
                self.marker_at_playhead();
            }
            SegmentCommand::AddMarkerAt(frame) => {
                self.cue_at(frame);
                self.marker_edit = None;
                self.selected_marker = None;
                self.marker_at_playhead();
            }
            SegmentCommand::NudgeMarker(frames) => self.nudge_marker(frames),
            SegmentCommand::DragMarker { marker_id, frame } => {
                // User rule: a marker moves only after Ctrl+M took it.
                if self.marker_edit.as_ref().map(|(id, _)| id) == Some(&marker_id) {
                    self.set_marker_draft(frame);
                }
            }
            SegmentCommand::TakeCutEdge { marker_id, frame } => return self.take_sound_edge_at(&marker_id, frame),
            SegmentCommand::DragCutEdge { marker_id, frame } => return self.drag_sound_edge(&marker_id, frame),
            SegmentCommand::SelectSlotAtPlayhead => self.select_slot_at_playhead(),
            SegmentCommand::FocusEmptySlot => self.focus_empty_slot(),
            SegmentCommand::NavigateObject { up } => self.navigate_object(up),
            SegmentCommand::Edit(segment_id) => self.edit(segment_id),
            SegmentCommand::CancelEdit => self.cancel_edit(),
            SegmentCommand::CancelMarkerEdit => {
                if self.cover_taken.take().is_some() {
                    self.refresh_view(qnc_ui_text::text("story.msg.cover_delete_cancelled").into());
                }
                self.cancel_marker_edit();
            }
            SegmentCommand::DeleteFocused => self.delete_focused(),
            SegmentCommand::PurgeSelected => self.purge_selected(),
        }
        true
    }

    /// Up or down in the open export box.
    pub fn export_step(&mut self, up: bool) {
        if let Some(export) = self.export.as_mut() {
            export.step(up);
        }
    }

    pub(crate) fn step(&mut self, up: bool) {
        // The segments of the user; the lead-in and lead-out are glued to them.
        let rows: Vec<_> = self.view.rows.iter().filter(|row| !row.lead).collect();
        let from = self
            .selected
            .as_deref()
            .and_then(|id| rows.iter().position(|row| row.segment_id == id));
        let target = match from {
            Some(index) => {
                let next = if up {
                    index.saturating_sub(1)
                } else {
                    (index + 1).min(rows.len() - 1)
                };
                rows.get(next).copied()
            }
            None if up => rows.last().copied(),
            None => rows.first().copied(),
        };
        if let Some(segment_id) = target.map(|row| row.segment_id.clone()) {
            self.select(&segment_id);
        }
    }
}

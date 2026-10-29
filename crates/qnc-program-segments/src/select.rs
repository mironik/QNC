use super::*;

impl ProgramSegments {
    /// Selects a segment (clears the selected marker and slot, docs/94 3.1) and
    /// cues its start in the Wrap view.
    pub fn select(&mut self, segment_id: &str) {
        let Some(start) = self
            .view
            .rows
            .iter()
            .find(|row| row.segment_id == segment_id)
            .map(|row| row.start_frame)
        else {
            return;
        };
        self.selected = Some(segment_id.to_string());
        self.selected_marker = None;
        self.marker_edit = None;
        self.selected_slot = None;
        self.write(Operation::SelectPart {
            part_id: segment_id.to_string(),
        });
        if self.selected_cover.take().is_some() {
            self.write(Operation::SelectCover {
                cover_id: String::new(),
            });
        }
        self.refresh_view(String::new());
        self.cue_program(start);
    }

    pub(crate) fn select_marker(&mut self, marker_id: &str) {
        self.marker_edit = None;
        let Some(frame) = self
            .view
            .markers
            .iter()
            .chain(&self.view.joins)
            .find(|pin| pin.marker_id == marker_id)
            .map(|pin| pin.frame)
        else {
            return;
        };
        // v5 `select_marker`: the start marker is locked and is not selected.
        if frame == 0 {
            self.selected_marker = None;
            self.refresh_view(qnc_ui_text::text("story.msg.start_marker_locked").into());
        } else {
            self.selected_marker = Some(marker_id.to_string());
            self.selected_slot = None;
            self.refresh_view(String::new());
        }
        self.cue_program(frame);
    }

    /// Selects a slot; the playhead goes to the clicked frame, else to the slot
    /// start (v5 `SelectMarkerSlot` and `select_adjacent_marker_slot`).
    pub(crate) fn select_slot(&mut self, slot_id: &str, frame: Option<u64>) {
        let Some(start) = self
            .view
            .slots
            .iter()
            .find(|slot| slot.slot_id == slot_id)
            .map(|slot| slot.start_frame)
        else {
            return;
        };
        self.selected_slot = Some(slot_id.to_string());
        self.selected_marker = None;
        self.marker_edit = None;
        self.write(Operation::SelectSlot {
            slot_id: slot_id.to_string(),
        });
        self.refresh_view(String::new());
        self.cue_at(frame.unwrap_or(start));
    }

    /// A program frame the user pointed at on a Wrap segment: the segment under it
    /// becomes the selected one (v5 keeps `selected_part_id` on the playhead
    /// segment) and the player is asked for that frame.
    pub(crate) fn cue_at(&mut self, frame: u64) {
        if let Some(segment) = self.view.segment_at(frame) {
            if self.selected.as_deref() != Some(segment.segment_id.as_str()) {
                self.selected = Some(segment.segment_id.clone());
                self.refresh_view(String::new());
            }
        }
        self.cue_program(frame);
    }

    /// Wrap view: the program playhead goes to the frame at once (v5
    /// `set_wrap_playhead_frame`) and the Wrap view is asked for that program
    /// frame. No source clip is chosen here: Wrap plays the program (v5).
    pub(crate) fn cue_program(&mut self, frame: u64) {
        if self.view.is_empty() {
            return;
        }
        let frame = frame.min(self.view.total_frames);
        // A stopped Sync's draft stays until Enter or Delete (live 2026-10-19: a click on a
        // Wrap row lost it, the mouse could not move its end any more).
        if self.sync.stopped() {
            return self.refresh_view(qnc_ui_text::text("story.msg.sync_stopped").into());
        }
        // Pointing elsewhere stops a Sync play: the whole program opens again.
        self.program_changed |= self.sync.cancel();
        self.set_playhead(Some(frame));
        self.seek = Some(frame);
    }

    /// M on the Wrap segment under the playhead: the marker is placed on that
    /// segment at the frame inside it (v5 `create_marker_from_part_frame`); the
    /// program only shows it. A selected marker moves there (docs/94 7a).
    pub(crate) fn marker_at_playhead(&mut self) {
        let Some(frame) = self.playhead else {
            return self.refresh_view(qnc_ui_text::text("story.msg.marker_on_wrap").into());
        };
        // User rule: only a marker taken with Ctrl+M goes to the playhead (a draft,
        // Enter confirms); otherwise M sets a new marker.
        let operation = match self.marker_edit.clone() {
            Some((marker_id, _)) => return self.marker_to_playhead(marker_id),
            None => {
                let Some(segment) = self.view.segment_at(frame) else {
                    return self.refresh_view(qnc_ui_text::text("story.msg.no_wrap_segment").into());
                };
                Operation::CreateMarker {
                    part_id: segment.segment_id.clone(),
                    local_frame: frame - segment.start_frame,
                }
            }
        };
        self.write(operation);
    }
}

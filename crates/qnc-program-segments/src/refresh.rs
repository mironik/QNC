use super::*;

impl ProgramSegments {
    pub(crate) fn refresh_view(&mut self, message: String) {
        if !known(
            &self.selected,
            self.stored.iter().map(|row| row.segment_id.as_str()),
        ) {
            self.selected = None;
        }
        let mut view = program(&self.stored, self.selected.as_deref(), self.story_rate);
        (view.undo_depth, view.redo_depth) = self.history;
        view.markers = resolve(&view, &self.stored_markers, self.selected_marker.as_deref());
        view.alert = self.alert.clone();
        // A cut whose A2 sound is stored away from its picture (a J or an L) shows it always
        // (user 2026-10-22: the moved red cut stays visible).
        for pin in view.markers.iter_mut() {
            let offset = self.cut_offset(&pin.marker_id);
            pin.sound_at = (offset != 0).then(|| pin.frame.saturating_add_signed(offset));
        }
        if let Some((marker_id, draft)) = &self.marker_edit {
            // The draft is drawn where the marker is going.
            for pin in view
                .markers
                .iter_mut()
                .filter(|pin| &pin.marker_id == marker_id)
            {
                pin.origin = Some(pin.frame);
                pin.sound_at = self.cut_sound_frame().or(pin.sound_at);
                pin.frame = *draft;
                pin.taken = true;
                pin.taken_part = match self.cut_part {
                    CutPart::Whole => qnc_program_view::TakenPart::Whole,
                    CutPart::Picture => qnc_program_view::TakenPart::Picture,
                    CutPart::Sound { .. } => qnc_program_view::TakenPart::Sound,
                };
            }
        }
        if !known(
            &self.selected_marker,
            view.markers.iter().map(|pin| pin.marker_id.as_str()),
        ) {
            self.selected_marker = None;
        }
        view.slots = slots(&self.stored_slots, self.selected_slot.as_deref());
        if !known(
            &self.selected_slot,
            view.slots.iter().map(|slot| slot.slot_id.as_str()),
        ) {
            self.selected_slot = None;
        }
        if self.slot_move.as_ref().map(|slot| &slot.slot_id) != self.selected_slot.as_ref() {
            self.slot_move = None; // another selection lets the slot go
        }
        if self.slot_trim.as_ref().map(|trim| &trim.slot_id) != self.selected_slot.as_ref() {
            self.slot_trim = None; // another selection lets the slot edge go
        }
        if let Some(trim) = &self.slot_trim {
            // The draft: the markers from the slot's end on move with the trim.
            let total = view.total_frames;
            view.markers.iter_mut().filter(|pin| !pin.locked).for_each(|pin| pin.frame = trim.moved(pin.frame, total));
            for drawn in &mut view.slots {
                (drawn.start_frame, drawn.end_frame) = (trim.moved(drawn.start_frame, total), trim.moved(drawn.end_frame, total));
            }
        }
        if let Some(slot) = &self.slot_move {
            // The draft is drawn where the slot and its two markers are going.
            view.markers.iter_mut().for_each(|pin| pin.frame = slot.moved(pin.frame));
            for drawn in &mut view.slots {
                (drawn.start_frame, drawn.end_frame) = (slot.moved(drawn.start_frame), slot.moved(drawn.end_frame));
            }
        }
        if !known(
            &self.selected_cover,
            self.stored_covers
                .iter()
                .map(|cover| cover.cover_id.as_str()),
        ) {
            self.selected_cover = None;
        }
        view.covers = cover_spans(&self.stored_covers, self.selected_cover.as_deref(), view.timebase);
        if view.rows.is_empty() {
            // No segments: neither way of making covers is chosen (user 2026-10-10).
            self.covers_chosen = false;
            if self.sync.enabled() {
                self.program_changed |= self.sync.toggle();
            }
        }
        view.sync_enabled = self.sync.enabled();
        view.covers_chosen = self.covers_chosen;
        view.playhead = self.playhead;
        view.cover_pressed = self.cover_pressed;
        view.message = match self.cover_pressed {
            // A Cover slot or Overwrite asked for in the last moments says so (user
            // 2026-10-13: Overwrite gave no sign that it was taken).
            Some((overwrite, at)) if message.is_empty() && at.elapsed() < COVER_NOTICE => {
                if overwrite { qnc_ui_text::text("story.msg.overwrite_done").into() } else { qnc_ui_text::text("story.msg.cover_slot_done").into() }
            }
            _ => message,
        };
        view.editing = self.editing.clone();
        view.edit_stills = match &self.slot_trim {
            Some(trim) => qnc_program_view::trim_stills(&view, &slots(&self.stored_slots, None), &trim.slot_id, trim.start, trim.frames),
            None => qnc_program_view::edit_stills(&view, &slots(&self.stored_slots, None), &self.edit_moves()),
        };
        view.source_edit_stills = self.edge_stills();
        view.trim_frames = self.edit_moves().into_iter().map(|(_, new)| new).collect();
        view.trim_frames.extend(self.overlap.as_ref().map(overlap::OverlapEdit::frame));
        view.trim_frames.extend(self.cut_sound_frame());
        view.a1_splits = self.picture_leads();
        qnc_program_view::mark_clip_edges(&mut view.edit_stills, |id| self.clip_frames.get(id).copied());
        self.lane_view(&mut view);
        view.zoom = self.zoom_window(&view);
        self.view = view;
    }

    /// What the view shows of a segment's or cover's sound taken on the Wrap row.
    pub(crate) fn lane_view(&self, view: &mut SegmentsView) {
        view.part_sound = self.part_sound_view();
    }

    /// A1 (lane 0) or A2 as the source timeline keeps it: what a new segment, cover or
    /// Sync cover takes (user 2026-10-16).
    pub fn kept_lane(&self, lane: u8) -> qnc_lane_choice::Lane {
        self.source.lanes[usize::from(lane.min(1))]
    }
}

//! Moving an M marker (user rule 2026-09-25, the "key sets, Ctrl+key opens a
//! control" pattern): Ctrl+M takes the selected marker (else the unlocked one
//! nearest to the Wrap playhead) into editing; arrows move its draft frame by one,
//! M puts the draft on the playhead, a mouse drag sets it; the Wrap playhead shows
//! the draft. Enter writes it, Escape drops it. A marker moves only between its
//! neighbours (docs/94 7a); the locked start and end never move.

use crate::{check_move, ProgramSegments};

impl ProgramSegments {
    /// Ctrl+M: the marker to edit and its draft on its own frame.
    pub(crate) fn edit_marker(&mut self) {
        let playhead = self.playhead.unwrap_or(0);
        // Every cut is a marker (user 2026-10-29): the cuts between two clips are taken too.
        let pin = match &self.selected_marker {
            Some(id) => self.view.markers.iter().chain(&self.view.joins).find(|pin| &pin.marker_id == id),
            None => self
                .view
                .markers
                .iter()
                .chain(&self.view.joins)
                .filter(|pin| !pin.locked)
                .min_by_key(|pin| pin.frame.abs_diff(playhead)),
        };
        let Some((marker_id, frame)) = pin
            .filter(|pin| !pin.locked)
            .map(|pin| (pin.marker_id.clone(), pin.frame))
        else {
            return self.refresh_view(qnc_ui_text::text("story.msg.no_marker_to_move").into());
        };
        self.start_marker_edit(marker_id, frame);
    }

    /// A draft for a marker: Ctrl+M, M on a selected marker, or a drag.
    pub(crate) fn start_marker_edit(&mut self, marker_id: String, frame: u64) {
        let pins = self.stored_pins();
        let Some(pin) = pins
            .iter()
            .chain(&self.view.joins)
            .find(|pin| pin.marker_id == marker_id && !pin.locked)
            .cloned()
        else {
            return self.refresh_view(qnc_ui_text::text("story.msg.edge_markers_locked").into());
        };
        if self.refused(crate::confirm::Take::Marker(marker_id.clone())) {
            return;
        }
        // The draft starts where the marker is stored; a refused frame keeps it there.
        self.let_go_edits(false);
        self.selected_marker = Some(marker_id.clone());
        self.selected_slot = None;
        self.marker_edit = Some((marker_id, pin.frame));
        self.cut_part = crate::CutPart::Whole;
        self.set_marker_draft(frame);
    }

    /// Arrows while editing: the draft one frame back or forward.
    pub(crate) fn nudge_marker(&mut self, frames: i64) {
        if self.nudge_cut_sound(frames) {
            return; // only the sound of the cut moves
        }
        if self.at_clip_end(frames) {
            return;
        }
        if let Some((_, draft)) = &self.marker_edit {
            let frame = draft.saturating_add_signed(frames);
            self.set_marker_draft(frame);
        }
    }

    /// M while a marker is selected: its draft on the Wrap playhead.
    pub(crate) fn marker_to_playhead(&mut self, marker_id: String) {
        let frame = self.playhead.unwrap_or(0);
        self.start_marker_edit(marker_id, frame);
    }

    /// The draft goes to `frame` only between the neighbours; the playhead shows it.
    pub(crate) fn set_marker_draft(&mut self, frame: u64) {
        let Some((marker_id, _)) = self.marker_edit.clone() else {
            return;
        };
        // A cut between two clips (user 2026-10-29) moves as an M marker does, between the cuts
        // beside it (markers and other joins): whole a roll, its picture alone a picture J or L.
        let join = qnc_program_view::join_segment(&marker_id).is_some();
        let pins = if join { self.cut_pins() } else { self.stored_pins() };
        if let Err(error) = check_move(&pins, &marker_id, frame) {
            return self.refresh_view(error);
        }
        let before = self.marker_edit.clone();
        let direction = before.as_ref().map_or(0, |(_, draft)| frame as i64 - *draft as i64);
        self.marker_edit = Some((marker_id, frame));
        self.refresh_view(String::new());
        if self.past_clip_end(direction) {
            self.marker_edit = before;
            return self.refresh_view(qnc_ui_text::text("story.msg.clip_at_end").into());
        }
        // Only its picture moving says so on every step (the log 2026-10-22: the whole marker's
        // words came back after the first step).
        let words = match (self.cut_part == crate::CutPart::Picture, join) {
            (true, _) => "story.msg.cut_picture_taken",
            (false, true) => "story.msg.join_taken",
            (false, false) => "story.msg.marker_taken",
        };
        self.refresh_view(qnc_ui_text::text(words).into());
        self.cue_program(frame);
    }

    /// Enter: the draft is written (v5 `marker.move`).
    pub(crate) fn commit_marker_edit(&mut self) -> bool {
        let Some((marker_id, frame)) = self.marker_edit.take() else {
            return false;
        };
        self.commit_cut(marker_id, frame);
        true
    }

    /// Escape: the marker stays where it is stored.
    pub(crate) fn cancel_marker_edit(&mut self) {
        self.cut_part = crate::CutPart::Whole;
        if self.marker_edit.take().is_some() {
            self.refresh_view(qnc_ui_text::text("story.msg.marker_move_cancelled").into());
        }
    }

    /// Whether a marker draft is open (arrows, Enter and Escape belong to it).
    pub fn editing_marker(&self) -> bool {
        self.marker_edit.is_some()
    }

    /// The stored markers without the draft: neighbours are checked against them.
    fn stored_pins(&self) -> Vec<crate::MarkerPin> {
        crate::resolve(&self.view, &self.stored_markers, None)
    }

    /// Every stored cut: the markers and the joins of two clips, in program order.
    fn cut_pins(&self) -> Vec<crate::MarkerPin> {
        let mut pins = self.stored_pins();
        pins.extend(self.view.joins.iter().cloned());
        pins.sort_by_key(|pin| pin.frame);
        pins
    }
}

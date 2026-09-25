//! Moving an M marker (user rule 2026-09-25, the "key sets, Ctrl+key opens a
//! control" pattern): Ctrl+M takes the selected marker (else the unlocked one
//! nearest to the Wrap playhead) into editing; arrows move its draft frame by one,
//! M puts the draft on the playhead, a mouse drag sets it; the Wrap playhead shows
//! the draft. Enter writes it, Escape drops it. A marker moves only between its
//! neighbours (docs/94 7a); the locked start and end never move.

use qnc_content_store::Operation;

use crate::{check_move, ProgramSegments};

impl ProgramSegments {
    /// Ctrl+M: the marker to edit and its draft on its own frame.
    pub(crate) fn edit_marker(&mut self) {
        let playhead = self.playhead.unwrap_or(0);
        let pin = match &self.selected_marker {
            Some(id) => self.view.markers.iter().find(|pin| &pin.marker_id == id),
            None => self
                .view
                .markers
                .iter()
                .filter(|pin| !pin.locked)
                .min_by_key(|pin| pin.frame.abs_diff(playhead)),
        };
        let Some((marker_id, frame)) = pin
            .filter(|pin| !pin.locked)
            .map(|pin| (pin.marker_id.clone(), pin.frame))
        else {
            return self.refresh_view("Nema M markera za pomak.".into());
        };
        self.start_marker_edit(marker_id, frame);
    }

    /// A draft for a marker: Ctrl+M, M on a selected marker, or a drag.
    pub(crate) fn start_marker_edit(&mut self, marker_id: String, frame: u64) {
        let pins = self.stored_pins();
        let Some(pin) = pins
            .iter()
            .find(|pin| pin.marker_id == marker_id && !pin.locked)
        else {
            return self.refresh_view("Početni i završni M marker su zaključani.".into());
        };
        // The draft starts where the marker is stored; a refused frame keeps it there.
        self.selected_marker = Some(marker_id.clone());
        self.selected_slot = None;
        self.marker_edit = Some((marker_id, pin.frame));
        self.set_marker_draft(frame);
    }

    /// Arrows while editing: the draft one frame back or forward.
    pub(crate) fn nudge_marker(&mut self, frames: i64) {
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
        if let Err(error) = check_move(&self.stored_pins(), &marker_id, frame) {
            return self.refresh_view(error);
        }
        self.marker_edit = Some((marker_id, frame));
        self.refresh_view("Marker: ←/→ pomak, M na playhead, Enter potvrda, Esc odustani".into());
        self.cue_program(frame);
    }

    /// Enter: the draft is written (v5 `marker.move`).
    pub(crate) fn commit_marker_edit(&mut self) -> bool {
        let Some((marker_id, frame)) = self.marker_edit.take() else {
            return false;
        };
        self.marker_committing = true;
        self.write(Operation::MoveMarker {
            marker_id,
            program_frame: frame,
        });
        true
    }

    /// Escape: the marker stays where it is stored.
    pub(crate) fn cancel_marker_edit(&mut self) {
        if self.marker_edit.take().is_some() {
            self.refresh_view("Pomak markera odustan.".into());
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
}

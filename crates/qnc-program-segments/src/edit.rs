//! Edit of a segment (user rule 2026-10-01): the Edit button of the Segment tab opens
//! the segment's clip in the source timeline with its IN/OUT; I/O move them, Enter
//! writes the new range (`TrimSegment`, the markers follow the picture), Escape drops
//! the edit. Nothing here plays or opens media: the source view is asked to.

use qnc_program_db::Operation;

use crate::ProgramSegments;

impl ProgramSegments {
    /// Edit: the source view opens the segment's clip with its IN/OUT.
    pub(crate) fn edit(&mut self, segment_id: String) {
        let Some(segment) = self.stored.iter().find(|row| row.segment_id == segment_id && row.active) else {
            return self.refresh_view("Ureduje se samo segment u programu.".into());
        };
        self.source_request = Some((segment.clip_id.clone(), segment.in_frame, segment.out_frame));
        self.selected = Some(segment_id.clone());
        self.editing = Some(segment_id);
        self.refresh_view("Uredivanje segmenta: I/O na source timelineu, Enter sprema, Esc odustaje".into());
    }

    /// The clip and IN/OUT the source view should open, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }

    /// Enter while a segment is edited: its new IN/OUT from the source marks of the
    /// same clip. False when no segment is edited.
    pub(crate) fn commit_edit(&mut self) -> bool {
        let Some(segment_id) = self.editing.clone() else {
            return false;
        };
        let clip = self.stored.iter().find(|row| row.segment_id == segment_id).map(|row| row.clip_id.clone());
        let Some((clip_id, (in_frame, out_frame), _)) = self.marked_source() else {
            return true;
        };
        if clip.as_deref() != Some(clip_id.as_str()) {
            self.refresh_view("Segment se ureduje na svom klipu; vrati se na njega ili Esc.".into());
            return true;
        }
        self.editing = None;
        self.adopt_selection = true;
        self.write(Operation::TrimSegment { segment_id, in_frame, out_frame });
        true
    }
}

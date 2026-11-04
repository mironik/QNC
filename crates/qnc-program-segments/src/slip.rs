//! Slip of a segment (docs/98 1.5, user 2026-11-04): Ctrl+Shift+S takes the segment under the
//! Wrap playhead; the arrows (and the wheel as arrows) make it play its clip a frame later or
//! earlier, the same length in the same place; Enter (or the zoom's Save) writes it
//! (`SlipSegment`, one Undo step), Escape gives it up. Its markers, slots and covers stay.

use qnc_program_db::Operation;

use crate::ProgramSegments;

/// The catalog action that takes a slip (contracts/qnc-keyboard-shortcuts.json).
pub const SLIP: &str = "select_slip";

impl ProgramSegments {
    /// A key while a slip is taken, or the key that takes one; `None` for any other.
    pub(crate) fn slip_key(&mut self, action_id: &str) -> Option<bool> {
        if action_id == SLIP {
            return Some(self.take_slip());
        }
        let (segment_id, frames, source_in) = self.slip.clone()?;
        let step = match action_id {
            "step_back_frame" => -1,
            "step_forward_frame" => 1,
            "activate_focused_item" => return Some(self.commit_slip()),
            "clear_focus" => {
                self.slip = None;
                self.refresh_view(String::new());
                return Some(true);
            }
            _ => return None,
        };
        // Never before the clip's first frame (its end the store checks).
        let frames = (frames + step).max(-(source_in as i64));
        self.slip = Some((segment_id, frames, source_in));
        self.refresh_view(qnc_ui_text::fill("story.msg.slip", &[("frames", &format!("{frames:+}"))]));
        Some(true)
    }

    fn take_slip(&mut self) -> bool {
        let at = self.playhead.unwrap_or(0);
        let Some(row) = self.view.rows.iter().find(|row| !row.lead && (row.start_frame..row.end_frame).contains(&at)).cloned() else {
            self.refresh_view(qnc_ui_text::text("story.msg.no_segment_under_playhead").into());
            return false;
        };
        self.let_go_edits(false);
        self.slip = Some((row.segment_id, 0, row.source_in_frame));
        self.refresh_view(qnc_ui_text::fill("story.msg.slip", &[("frames", "+0")]));
        true
    }

    /// Enter: the slip is written; false when none is taken.
    pub(crate) fn commit_slip(&mut self) -> bool {
        let Some((segment_id, frames, _)) = self.slip.take() else {
            return false;
        };
        if frames != 0 {
            self.write(Operation::SlipSegment { segment_id, frames });
        } else {
            self.refresh_view(String::new());
        }
        true
    }
}

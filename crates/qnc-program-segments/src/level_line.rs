//! The level curve of a stored segment (A1) or cover (A2) on the Wrap row (docs/98 2.1, user
//! 2026-11-06: "a right click on the level line takes it; a click places the playhead; the line
//! of the playhead is the point that is controlled", in the style of every other Ctrl+). The
//! curve and its draft are the piece `qnc-level-curve`; here the program model finds the item,
//! tells the user and writes the curve (one Undo step).

use qnc_level_curve::LevelLine;
use qnc_program_db::Operation;

use crate::ProgramSegments;

impl ProgramSegments {
    /// Ctrl+Shift+1 (lane 0: the segment's A1) or Ctrl+Shift+2 (lane 1: the cover's A2), or a
    /// right click on its line, at program frame `frame`.
    pub fn take_level_line(&mut self, lane: u8, frame: u64) -> bool {
        let Some((id, span, keys)) = self.view.level_item_at(lane, frame) else {
            let key = if lane == 0 { "story.msg.no_segment_at_playhead" } else { "story.msg.no_cover_at_playhead" };
            self.refresh_view(qnc_ui_text::text(key).into());
            return false;
        };
        if self.level_line.as_ref().is_some_and(|line| line.lane == lane && line.id == id) {
            self.say_level(frame);
            return true;
        }
        if self.refused(crate::confirm::Take::LevelLine(lane, id.clone())) {
            return false;
        }
        self.let_go_edits(false);
        self.level_line = Some(LevelLine::new(lane, id, span, keys));
        self.say_level(frame);
        true
    }

    pub fn level_line_taken(&self) -> bool {
        self.level_line.is_some()
    }

    /// Up/Down, Ctrl+left/right or Ctrl+wheel: the point on the playhead line up or down by
    /// `steps` dB (made there when there is none).
    pub fn nudge_level(&mut self, frame: u64, steps: i16) -> bool {
        let changed = self.level_line.as_mut().is_some_and(|line| line.nudge(frame, steps));
        self.say_level(frame);
        changed
    }

    /// Delete: the point on the playhead line goes.
    pub fn remove_level_point(&mut self, frame: u64) -> bool {
        let removed = self.level_line.as_mut().is_some_and(|line| line.remove_at(frame));
        if removed {
            self.say_level(frame);
        } else if self.level_line.is_some() {
            self.refresh_view(qnc_ui_text::text("story.msg.level_no_point").into());
        }
        removed
    }

    /// Enter (`keep_taken` false) or a click on the row (true: the curve stays taken and the
    /// playhead goes where it was clicked): the drafted curve is written, one Undo step.
    pub fn commit_level_line(&mut self, keep_taken: bool) -> bool {
        let Some(line) = self.level_line.as_mut() else {
            return false;
        };
        let (id, keys) = (line.id.clone(), line.keep());
        if !keep_taken {
            self.level_line = None;
        }
        if let Some(keys) = keys {
            self.write(Operation::SetLevelKeys { item_id: id, keys });
        }
        self.refresh_view(String::new());
        true
    }

    /// Escape: the taken curve is let go, nothing written.
    pub fn release_level_line(&mut self) -> bool {
        let released = self.level_line.take().is_some();
        if released {
            self.refresh_view(String::new());
        }
        released
    }

    /// The drafted curve (lane, item, points), as the program plays it before Enter.
    pub fn level_line_draft(&self) -> Option<(u8, String, Vec<qnc_program_db::LevelKey>)> {
        let line = self.level_line.as_ref()?;
        Some((line.lane, line.id.clone(), line.draft().to_vec()))
    }

    /// The status: the level of the curve on the playhead line and whether a point is there.
    pub(crate) fn say_level(&mut self, frame: u64) {
        let Some(line) = &self.level_line else { return };
        let level = format!("{:+.1}", f64::from(line.level_at(frame)) / 10.0);
        let key = if line.point_at(frame).is_some() { "story.msg.level_point" } else { "story.msg.level_line" };
        let lane = if line.lane == 0 { "A1" } else { "A2" };
        self.refresh_view(qnc_ui_text::fill(key, &[("lane", lane), ("level", &level)]));
    }
}

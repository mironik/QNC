//! The source IN/OUT taken with the keyboard (v5 `story.rs` `select_mark_in`,
//! `select_mark_out`, `nudge_in`, `nudge_out`; user rule 2026-09-25: Ctrl+key selects).
//! Ctrl+I or Ctrl+O takes the mark and the playhead goes to it; the arrows move the
//! taken mark one frame and the playhead with it; Escape gives the keys back to the
//! playhead. It changes only the marks of the source timeline it is given and says
//! which source frame the player should cue; it plays nothing.

use qnc_timeline::{TimelineProjection, TimelineSourceMarkFocus};

/// What a key did: the source frame to cue, or only a change of the marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKey {
    Cue(u64),
    Changed,
    /// The mark is not set or would cross the other one: nothing changed.
    Refused,
}

/// Whether IN or OUT is taken (the arrows then move it).
pub fn is_taken(timeline: &TimelineProjection) -> bool {
    timeline.source_mark_focus != TimelineSourceMarkFocus::Playhead
}

/// Ctrl+I (`in_mark`) or Ctrl+O: takes a set mark and cues the playhead to it.
pub fn take(timeline: &mut TimelineProjection, in_mark: bool) -> MarkKey {
    let (mark, focus) = if in_mark {
        (timeline.source_in_frame, TimelineSourceMarkFocus::In)
    } else {
        (timeline.source_out_frame, TimelineSourceMarkFocus::Out)
    };
    let Some(frame) = mark else {
        return MarkKey::Refused; // v5: "Prvo stavi IN"
    };
    timeline.source_mark_focus = focus;
    MarkKey::Cue(cue_frame(timeline, frame, !in_mark))
}

/// An arrow while a mark is taken: the mark moves by `frames`, never across the
/// other one or out of the clip, and the playhead follows it.
pub fn nudge(timeline: &mut TimelineProjection, frames: i64) -> MarkKey {
    let duration = timeline.duration_frames;
    let Some((in_frame, out_frame)) = timeline.visible_source_marks() else {
        return MarkKey::Refused;
    };
    let moved = |frame: u64| u64::try_from(frame as i64 + frames).ok();
    match timeline.source_mark_focus {
        TimelineSourceMarkFocus::In => match moved(in_frame).filter(|next| *next < out_frame) {
            Some(next) => {
                timeline.source_in_frame = Some(next);
                MarkKey::Cue(cue_frame(timeline, next, false))
            }
            None => MarkKey::Refused, // v5: "IN ne smije prijeći OUT"
        },
        TimelineSourceMarkFocus::Out => match moved(out_frame).filter(|next| *next > in_frame && *next <= duration) {
            Some(next) => {
                timeline.source_out_frame = Some(next);
                MarkKey::Cue(cue_frame(timeline, next, true))
            }
            None => MarkKey::Refused,
        },
        TimelineSourceMarkFocus::Playhead => MarkKey::Refused,
    }
}

/// Escape: the keys go back to the playhead.
pub fn release(timeline: &mut TimelineProjection) -> MarkKey {
    timeline.source_mark_focus = TimelineSourceMarkFocus::Playhead;
    MarkKey::Changed
}

/// The source frame the player shows for a mark: OUT is exclusive, so its last frame.
fn cue_frame(timeline: &TimelineProjection, frame: u64, out: bool) -> u64 {
    let frame = if out { frame.saturating_sub(1) } else { frame };
    timeline.range_start_frame + frame
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline() -> TimelineProjection {
        TimelineProjection {
            duration_frames: 100,
            playhead_frame: Some(0),
            source_in_frame: Some(10),
            source_out_frame: Some(40),
            ..Default::default()
        }
    }

    #[test]
    fn ctrl_i_takes_in_the_arrows_move_it_and_escape_lets_it_go() {
        let mut t = timeline();
        assert_eq!(take(&mut t, true), MarkKey::Cue(10));
        assert!(is_taken(&t));
        assert_eq!(nudge(&mut t, -1), MarkKey::Cue(9));
        assert_eq!(t.source_in_frame, Some(9));
        assert_eq!(release(&mut t), MarkKey::Changed);
        assert!(!is_taken(&t));
    }

    #[test]
    fn ctrl_o_takes_out_and_marks_never_cross_or_leave_the_clip() {
        let mut t = timeline();
        assert_eq!(take(&mut t, false), MarkKey::Cue(39), "the last frame before OUT");
        assert_eq!(nudge(&mut t, 1), MarkKey::Cue(40));
        assert_eq!(t.source_out_frame, Some(41));
        t.source_out_frame = Some(100);
        assert_eq!(nudge(&mut t, 1), MarkKey::Refused, "out of the clip");
        t.source_mark_focus = TimelineSourceMarkFocus::In;
        t.source_in_frame = Some(99);
        assert_eq!(nudge(&mut t, 1), MarkKey::Refused, "IN never reaches OUT");
        let mut unset = TimelineProjection { duration_frames: 100, ..Default::default() };
        assert_eq!(take(&mut unset, true), MarkKey::Refused, "v5: set IN first");
    }
}

//! The keys of the A1/A2 lanes, the same on every timeline (user 2026-10-17; core since docs/99:
//! moved from `qnc-timeline`, which re-exports them).

use crate::LaneChoice;

/// What a catalog action does to the lanes of a timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneKey {
    /// Ctrl+1 (0: A1) or Ctrl+2: take the lane.
    Take(u8),
    /// Up (-1) or down: the channel of the taken lane.
    Channel(i64),
    /// A click on a channel of the column (0-based): that channel of the taken lane, by
    /// mouse, pad or any other device, as up/down by keys (user 2026-10-18).
    Pick(u16),
    /// Left (off) or right (on).
    Off(bool),
    /// Ctrl+left/right, Ctrl+wheel: the level in dB.
    Level(i8),
    /// Enter: what is drafted is kept.
    Keep,
    /// Escape: the draft is dropped.
    Drop,
}

impl LaneKey {
    /// The lane key of a catalog action, the same on every timeline; `None` for any other.
    pub fn of(action_id: &str) -> Option<Self> {
        Some(match action_id {
            "select_audio_a1" => Self::Take(0),
            "select_audio_a2" => Self::Take(1),
            "step_prev_part" => Self::Channel(-1),
            "step_next_part" => Self::Channel(1),
            "step_back_frame" => Self::Off(true),
            "step_forward_frame" => Self::Off(false),
            "lane_level_down" | "wheel_source_ctrl_back" | "wheel_segments_ctrl_back" => Self::Level(-1),
            "lane_level_up" | "wheel_source_ctrl_forward" | "wheel_segments_ctrl_forward" => Self::Level(1),
            "activate_focused_item" => Self::Keep,
            "clear_focus" => Self::Drop,
            _ => return None,
        })
    }
}

/// A key on the lanes: true when something changed. [`LaneKey::Keep`] also notes the
/// moment (the same Enter does nothing else).
pub fn lane_key(lanes: &mut LaneChoice, key: LaneKey) -> bool {
    match key {
        // The taken lane taken again (a click on its A1/A2 button, Ctrl+1/Ctrl+2) keeps it and
        // closes its zoom (user 2026-11-07: the way out of the zoom is the button again or Esc).
        LaneKey::Take(lane) if lanes.taken().is_some_and(|(taken, _)| taken == lane) => lane_key(lanes, LaneKey::Keep),
        LaneKey::Take(lane) => lanes.take(lane),
        LaneKey::Channel(delta) => lanes.draft(delta),
        LaneKey::Pick(channel) => lanes.taken().is_some_and(|(lane, _)| lanes.set_draft(lane, channel)),
        LaneKey::Off(off) => lanes.set_off(off),
        LaneKey::Level(delta_db) => lanes.nudge_level(delta_db),
        LaneKey::Keep => {
            let kept = lanes.commit();
            lanes.mark_committed();
            kept
        }
        LaneKey::Drop => lanes.release(),
    }
}

/// The taken lane in the words of the status line: its channel, on/off and level; empty
/// when none is taken.
pub fn lane_message(lanes: &LaneChoice) -> String {
    let Some((lane, _)) = lanes.taken() else {
        return String::new();
    };
    let (now, name) = (lanes.lane(lane), if lane == 0 { "A1" } else { "A2" }.to_string());
    if now.off {
        return qnc_ui_text::fill("story.msg.lane_off", &[("lane", &name)]);
    }
    qnc_ui_text::fill(
        "story.msg.lane_level",
        &[("lane", &name), ("channel", &(now.channel + 1).to_string()), ("level", &format!("{:+}", now.level_db))],
    )
}


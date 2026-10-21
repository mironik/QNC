//! The A1/A2 lanes of every timeline (user 2026-10-17: the universal timeline is the basis;
//! the source timeline, the Wrap rows and the program row handle their lanes the same way).
//! Ctrl+1/Ctrl+2 take a lane; while it is taken up/down pick its channel, left turns it off,
//! right on, Ctrl+left/right (or Ctrl+wheel) set its level by 1 dB; Enter keeps, Escape
//! drops. Beside A1/A2 the lane's level is written when it is not as recorded or the lane is
//! taken. What a lane belongs to (the clip on the source timeline, a stored segment or cover
//! on a Wrap row) is the owner's: it takes the lane and writes what Enter kept. The lane state
//! itself is the piece `qnc-lane-choice`; nothing here plays, reads or writes.

pub use qnc_lane_choice::{Lane, LaneChoice};

use crate::ChannelChoice;

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

/// The channel column and button of A1 (lane 0) or A2 as the lane plays now; its wave is
/// already drawn at `drawn_db` (0 on the source timeline, the stored level on a Wrap row).
pub fn lane_choice(lanes: &LaneChoice, lane: u8, drawn_db: i8) -> Option<ChannelChoice> {
    let (_, count) = lanes.choice(lane)?;
    let open = lanes.taken().is_some_and(|(taken, _)| taken == lane);
    Some(drafted_choice(lanes.lane(lane), count, open, drawn_db))
}

/// The channel column and button of a lane playing `now` among `count` channels, `open`
/// while it is taken; its wave is already drawn at `drawn_db`.
pub fn drafted_choice(now: Lane, count: u16, open: bool, drawn_db: i8) -> ChannelChoice {
    ChannelChoice {
        selected: now.channel,
        count,
        open,
        off: now.off,
        level_db: now.level_db.saturating_sub(drawn_db),
        heard_db: now.level_db,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_keys_drive_every_timelines_lanes() {
        let mut lanes = LaneChoice::default();
        lanes.new_clip();
        lanes.set_channels(Some(4));
        assert!(lane_key(&mut lanes, LaneKey::of("select_audio_a2").unwrap()));
        lane_key(&mut lanes, LaneKey::of("step_next_part").unwrap());
        lane_key(&mut lanes, LaneKey::of("wheel_segments_ctrl_forward").unwrap());
        let choice = lane_choice(&lanes, 1, 0).unwrap();
        assert_eq!((choice.selected, choice.open, choice.heard_db), (2, true, 1));
        assert!(lane_message(&lanes).contains("+1"));
        lane_key(&mut lanes, LaneKey::Keep);
        assert_eq!((lanes.kept(1).channel, lanes.kept(1).level_db, lanes.taken()), (2, 1, None));
        assert_eq!(lane_choice(&lanes, 1, 1).map(|choice| choice.level_db), Some(0), "drawn at its stored level");
    }
}

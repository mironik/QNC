//! The one source timeline (user 2026-10-16/17: "only one source timeline, forms only
//! embed it"). Story, Media Assist, Ingest and every later form take this piece as it is:
//! the same A1/A2 lanes (channel, off/on, level, the large wave), the same keys, the same
//! wheel and the same answer to a click. It composes the existing pieces without merging
//! them: `qnc-source-panel` (play, steps, IN/OUT and the taken mark), `qnc-lane-choice`
//! (the lanes), `qnc-source-channels` and `qnc-source-timecode` (what the stored record of
//! the shown clip says) and `qnc-source-dock` (the drawing). It drives the universal source
//! preview; it knows no form, no story and writes no database.
//!
//! A form keeps one [`SourceTimeline`] beside its preview, calls [`SourceTimeline::sync`]
//! once a frame, routes the catalog actions of its source timeline panel through
//! [`SourceTimeline::route`] and draws [`show`] with the [`SourceTimelineView`] in its view.
//! What a form writes (a segment, a cover) takes the kept lanes from [`SourceTimeline::kept`].

use std::time::Duration;

use eframe::egui::{Rect, Ui};
use qnc_lane_choice::{Lane, LaneChoice};
use qnc_source_dock::{show_timeline_dock, SourceTimeline as DockTimeline, TimelineDockStyle};
use qnc_source_panel::SourcePanel;
use qnc_source_preview::SourcePreview;
use qnc_timeline::{lane_choice, lane_key, lane_message, AudioLane, TimelineIntent, TimelineProjection, TimelineTheme, LaneKey};
use qnc_timeline_assets::SourceTimelineAssets;

pub use qnc_source_channels::ContentTarget;
pub use qnc_source_timecode::SourceTimecode;

pub const MODULE_ID: &str = "qnc.module.source-timeline";
/// The catalog scope of the source timeline's keys, the same in every form.
pub const KEY_SCOPE: &str = "source_timeline";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The catalog actions of a click on a channel in the A1 or A2 column.
pub const CHOOSE_A1_CHANNEL: &str = "source_choose_a1_channel";
/// A left click on the timeline while IN or OUT is drawn taken: the mark stays where it is
/// and is let go (its own action, never Enter: audit 2026-10-17, the click as Enter walked
/// the panels and the next click pressed Add SubClip).
pub const KEEP_MARK: &str = "source_keep_mark";
pub const CHOOSE_A2_CHANNEL: &str = "source_choose_a2_channel";

/// One Enter does one thing: the other catalog actions of the Enter that kept a lane
/// arrive at once and are its.
const SAME_ENTER: Duration = Duration::from_millis(150);

/// What the board draws of the source timeline besides the clip's projection and
/// artifacts: the lane pickers, the large wave and the clip's timecode.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceTimelineView {
    pub a1: Option<qnc_timeline::ChannelChoice>,
    pub a2: Option<qnc_timeline::ChannelChoice>,
    pub wave_over_video: Option<(AudioLane, u8)>,
    pub timecode: Option<SourceTimecode>,
    /// What a taken lane does, for the form's status line (empty: nothing to say).
    pub message: String,
}

/// The state of the source timeline of one form.
#[derive(Debug, Default)]
pub struct SourceTimeline {
    keys: SourcePanel,
    lanes: LaneChoice,
    channels: qnc_source_channels::SourceChannels,
    timecodes: qnc_source_timecode::SourceTimecodes,
    clip: Option<String>,
    message: String,
    view: SourceTimelineView,
}

impl SourceTimeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Once a frame: the shown clip and the project content its stored record is read
    /// from. Another clip starts on A1 channel 1 and A2 channel 2 (user rule 2026-10-01).
    pub fn sync(&mut self, clip_id: Option<&str>, content: Option<&ContentTarget>) -> &SourceTimelineView {
        if self.clip.as_deref() != clip_id {
            self.clip = clip_id.map(str::to_string);
            self.lanes.new_clip();
            self.message.clear();
        }
        let count = self.channels.for_clip(content, clip_id);
        self.lanes.set_channels(count);
        let timecode = self.timecodes.for_clip(content, clip_id);
        self.refresh(timecode);
        &self.view
    }

    pub fn view(&self) -> &SourceTimelineView {
        &self.view
    }

    /// A1 (lane 0) or A2 as kept (Enter): what a new segment, cover or Sync cover takes.
    pub fn kept(&self, lane: u8) -> Lane {
        self.lanes.kept(lane)
    }

    /// A1 and A2 as kept.
    pub fn kept_lanes(&self) -> [Lane; 2] {
        [self.kept(0), self.kept(1)]
    }

    /// A lane is taken (its keys belong to it).
    pub fn lane_taken(&self) -> bool {
        self.lanes.taken().is_some()
    }

    /// A lane waits for Enter, or this Enter kept it: the same Enter does nothing else.
    pub fn holds_enter(&self) -> bool {
        self.lane_taken() || self.lanes.committed_within(SAME_ENTER)
    }

    /// IN or OUT is taken: the arrows move it.
    pub fn mark_taken(&mut self, timeline: &TimelineProjection) -> bool {
        self.keys.mark_taken(timeline)
    }

    /// A mark was taken or let go by someone else (a segment edge opened with its mark).
    pub fn set_mark_taken(&mut self, taken: bool) {
        self.keys.set_mark_taken(taken);
    }

    /// The catalog action a timeline intent stands for, when it is one: a click on A1/A2
    /// is Ctrl+1/Ctrl+2, the wheel is the arrows (Ctrl+wheel the level), a click on a
    /// channel of the column is [`CHOOSE_A1_CHANNEL`]/[`CHOOSE_A2_CHANNEL`] with the channel.
    pub fn action_of(intent: &TimelineIntent) -> Option<(&'static str, Option<u16>)> {
        match intent {
            TimelineIntent::TakeAudioLane(AudioLane::A2) => Some(("select_audio_a2", None)),
            TimelineIntent::TakeAudioLane(_) => Some(("select_audio_a1", None)),
            TimelineIntent::Wheel(wheel) => Some((wheel.action(qnc_wheel_jog::SOURCE_ACTIONS), None)),
            // A left click keeps a taken IN/OUT (Enter); a right click takes the nearer (Ctrl+I/O).
            TimelineIntent::KeepMark => Some((KEEP_MARK, None)),
            TimelineIntent::TakeMark(in_mark) => Some((if *in_mark { "select_mark_in" } else { "select_mark_out" }, None)),
            TimelineIntent::ChooseA1Channel(channel) => Some((CHOOSE_A1_CHANNEL, Some(*channel))),
            TimelineIntent::ChooseA2Channel(channel) => Some((CHOOSE_A2_CHANNEL, Some(*channel))),
            _ => None,
        }
    }

    /// A click on a channel of the column (`action_id` from [`Self::action_of`]), else the
    /// keys of a lane (Ctrl+1/Ctrl+2, and those of a taken lane); `None` for any other.
    /// Enter (or a left click) while IN or OUT is taken keeps the edit and lets the mark go.
    pub fn route_input(&mut self, action_id: &str, channel: Option<u16>, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> Option<bool> {
        if action_id == "activate_focused_item" && !self.lane_taken() && self.keys.mark_taken(timeline) {
            return self.keys.route("clear_focus", preview, timeline);
        }
        if action_id == KEEP_MARK && self.lane_taken() {
            return self.route_lane("activate_focused_item", preview); // a left click keeps the lane
        }
        if action_id == KEEP_MARK {
            // The timeline draws the mark taken: let it go whoever took it.
            self.keys.set_mark_taken(false);
            qnc_source_mark_focus::release(timeline);
            return Some(true);
        }
        channel
            .and_then(|channel| self.choose_action(action_id, channel, preview))
            .or_else(|| self.route_lane(action_id, preview))
    }

    /// A click on a channel of the column (`action_id` from [`Self::action_of`]); `None`
    /// for any other action.
    pub fn choose_action(&mut self, action_id: &str, channel: u16, preview: &mut SourcePreview) -> Option<bool> {
        let lane = match action_id {
            CHOOSE_A1_CHANNEL => 0,
            CHOOSE_A2_CHANNEL => 1,
            _ => return None,
        };
        Some(self.choose(lane, channel, preview))
    }

    /// A click on the source timeline (a frame, or V: its start): the playhead goes there,
    /// in every form the same; while the player plays a story program, the clip comes back.
    pub fn cue(&mut self, intent: &TimelineIntent, preview: &mut SourcePreview) -> bool {
        preview.timeline_intent(intent)
    }

    /// A click on a channel in the column of A1 (lane 0) or A2: on the taken lane it only
    /// drafts (Enter keeps it), otherwise it is kept at once. Heard at once.
    pub fn choose(&mut self, lane: u8, channel: u16, preview: &mut SourcePreview) -> bool {
        let changed = if self.lanes.taken().is_some_and(|(taken, _)| taken == lane) {
            self.lanes.set_draft(lane, channel)
        } else {
            self.lanes.choose(lane, channel)
        };
        self.after_lane(changed, preview)
    }

    /// One catalog action of the source timeline. `None`: not one of its actions, the
    /// form handles it; `Some(changed)`: taken here.
    pub fn route(&mut self, action_id: &str, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> Option<bool> {
        let routed = self.route_keys(action_id, preview, timeline);
        // Every key of the source timeline in the player log, the same for every form.
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!(
                    "source-key action={action_id} routed={routed:?} in={:?} out={:?} playhead={:?} mark_taken={} lane={:?}",
                    timeline.source_in_frame, timeline.source_out_frame, timeline.playhead_frame, self.keys.mark_taken(timeline), self.lanes.taken(),
                ),
            );
        }
        routed
    }

    fn route_keys(&mut self, action_id: &str, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> Option<bool> {
        if let Some(changed) = self.route_lane(action_id, preview) {
            return Some(changed);
        }
        match action_id {
            // The wheel is the left/right arrow; while a lane is taken (where the arrows
            // turn it off/on) it moves the playhead (user 2026-10-16). Ctrl+wheel is
            // Ctrl+left/right: the level of a taken lane, up louder.
            wheel if wheel.starts_with("wheel_") && wheel.contains("_ctrl_") => {
                let level = if wheel.ends_with("back") { "lane_level_down" } else { "lane_level_up" };
                if let Some(changed) = self.route_lane(level, preview) {
                    return Some(changed);
                }
                // No lane taken: Ctrl+wheel moves the taken IN or OUT, as the wheel does
                // (user 2026-10-17: right button and wheel cut the clip).
                if !self.keys.mark_taken(timeline) {
                    return Some(false);
                }
                let forward = wheel.ends_with("back") != qnc_wheel_jog::playhead_up_forward();
                self.keys.route(if forward { "step_forward_frame" } else { "step_back_frame" }, preview, timeline)
            }
            wheel if wheel.starts_with("wheel_") => {
                let forward = wheel.ends_with("back") != qnc_wheel_jog::playhead_up_forward();
                let frames = if forward { 1 } else { -1 };
                if self.lane_taken() {
                    return Some(preview.step(frames));
                }
                let arrow = if forward { "step_forward_frame" } else { "step_back_frame" };
                self.keys.route(arrow, preview, timeline)
            }
            // Down with no lane taken closes an open large wave.
            "step_next_part" if self.lanes.wave_zoom() != [0, 0] => {
                let closed = self.lanes.close_wave();
                Some(self.after_lane(closed, preview))
            }
            _ => self.keys.route(action_id, preview, timeline),
        }
    }

    /// The keys of A1/A2, the same on every timeline (`qnc_timeline::LaneKey`): Ctrl+1/Ctrl+2
    /// take a lane of the shown clip; while it is taken the timeline's lane keys drive it;
    /// all heard at once.
    pub fn route_lane(&mut self, action_id: &str, preview: &mut SourcePreview) -> Option<bool> {
        let key = LaneKey::of(action_id)?;
        if !matches!(key, LaneKey::Take(_)) && !self.lane_taken() {
            return None;
        }
        let changed = lane_key(&mut self.lanes, key);
        self.message = match (key, changed) {
            (LaneKey::Take(_), false) => qnc_ui_text::text("story.msg.no_audio_channels").into(),
            _ => lane_message(&self.lanes),
        };
        Some(self.after_lane(changed, preview))
    }

    /// The preview hears what the lanes play now; the view follows.
    fn after_lane(&mut self, changed: bool, preview: &mut SourcePreview) -> bool {
        let heard = preview.hear_channels(self.lanes.audible());
        let timecode = self.view.timecode;
        self.refresh(timecode);
        changed | heard
    }

    fn refresh(&mut self, timecode: Option<SourceTimecode>) {
        self.view = SourceTimelineView {
            a1: lane_choice(&self.lanes, 0, 0),
            a2: lane_choice(&self.lanes, 1, 0),
            wave_over_video: wave_over_video(&self.lanes),
            timecode,
            message: self.message.clone(),
        };
    }
}

/// The wave over the video row: the taken lane's, else A1's, else A2's.
fn wave_over_video(lanes: &LaneChoice) -> Option<(AudioLane, u8)> {
    let (zoom, names) = (lanes.wave_zoom(), [AudioLane::A1, AudioLane::A2]);
    let taken = lanes.taken().map(|(lane, _)| usize::from(lane.min(1)));
    taken.into_iter().chain([0, 1]).find(|lane| zoom[*lane] > 0).map(|lane| (names[lane], zoom[lane]))
}

/// Draws the source dock of any form: its frame, the header row (the clip's `title` and
/// whatever `header` adds) and the one source timeline under it.
pub fn show(
    ui: &mut Ui,
    rect: Rect,
    style: &TimelineDockStyle,
    title: &str,
    header: impl FnOnce(&mut Ui),
    (projection, theme, assets): (&TimelineProjection, TimelineTheme, &SourceTimelineAssets),
    view: &SourceTimelineView,
) -> TimelineIntent {
    let timeline = DockTimeline::from_assets(projection, theme, assets)
        .with_timecode(view.timecode)
        .with_a1_channel(view.a1)
        .with_a2_channel(view.a2)
        .with_wave_over_video(view.wave_over_video);
    show_timeline_dock(ui, rect, style, title, header, timeline)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_channels(count: u16) -> SourceTimeline {
        let mut source = SourceTimeline::new();
        source.lanes.new_clip();
        source.lanes.set_channels(Some(count));
        source
    }

    #[test]
    fn ctrl_2_takes_a2_arrows_draft_enter_keeps_escape_drops() {
        let (mut source, mut preview, mut timeline) = (with_channels(4), SourcePreview::default(), TimelineProjection::new(0, 100));
        assert_eq!(source.route("select_audio_a2", &mut preview, &mut timeline), Some(true));
        assert_eq!(source.view().a2.map(|a2| (a2.selected, a2.open)), Some((1, true)), "A2 starts on channel 2");
        source.route("step_next_part", &mut preview, &mut timeline);
        source.route("step_next_part", &mut preview, &mut timeline);
        source.route("step_next_part", &mut preview, &mut timeline);
        assert_eq!(source.view().a2.map(|a2| a2.selected), Some(3), "never past the last channel");
        assert_eq!(source.kept(1).channel, 1, "nothing kept before Enter");
        source.route("activate_focused_item", &mut preview, &mut timeline);
        assert!(source.holds_enter(), "the same Enter does nothing else");
        assert_eq!((source.kept(1).channel, source.lane_taken()), (3, false));
        source.route("select_audio_a2", &mut preview, &mut timeline);
        source.route("step_prev_part", &mut preview, &mut timeline);
        source.route("clear_focus", &mut preview, &mut timeline);
        assert_eq!(source.kept(1).channel, 3, "Escape keeps the channel");
    }

    #[test]
    fn left_right_turn_a_lane_off_and_on_and_ctrl_wheel_sets_its_level() {
        let (mut source, mut preview, mut timeline) = (with_channels(2), SourcePreview::default(), TimelineProjection::new(0, 100));
        source.route("select_audio_a1", &mut preview, &mut timeline);
        source.route("step_back_frame", &mut preview, &mut timeline);
        assert!(source.view().a1.is_some_and(|a1| a1.off), "left: off");
        source.route("step_forward_frame", &mut preview, &mut timeline);
        source.route("wheel_source_ctrl_forward", &mut preview, &mut timeline);
        source.route("wheel_source_ctrl_forward", &mut preview, &mut timeline);
        assert_eq!(source.view().a1.map(|a1| (a1.off, a1.level_db)), Some((false, 2)), "the wave is drawn at the draft level");
        assert!(source.view().message.contains("+2"), "{}", source.view().message);
        source.route("activate_focused_item", &mut preview, &mut timeline);
        assert_eq!(source.kept(0).level_db, 2);
    }

    #[test]
    fn a_click_on_a_lane_is_its_key_and_the_wheel_its_arrows() {
        assert_eq!(SourceTimeline::action_of(&TimelineIntent::TakeAudioLane(AudioLane::A2)), Some(("select_audio_a2", None)));
        assert_eq!(SourceTimeline::action_of(&TimelineIntent::ChooseA2Channel(3)), Some((CHOOSE_A2_CHANNEL, Some(3))));
        assert_eq!(SourceTimeline::action_of(&TimelineIntent::CueFrame(3)), None, "a cue is the form's");
    }

    #[test]
    fn enter_or_a_left_click_keeps_a_taken_mark_and_a_right_click_takes_one() {
        assert_eq!(SourceTimeline::action_of(&TimelineIntent::KeepMark), Some((KEEP_MARK, None)), "a click is never Enter");
        assert_eq!(SourceTimeline::action_of(&TimelineIntent::TakeMark(false)), Some(("select_mark_out", None)));
        let (mut source, mut preview) = (SourceTimeline::new(), SourcePreview::default());
        let mut timeline = TimelineProjection::new(0, 100).with_playhead(10).with_source_in_at_confirmed().with_playhead(40).with_source_out_at_confirmed();
        source.route("select_mark_in", &mut preview, &mut timeline);
        source.route("step_forward_frame", &mut preview, &mut timeline);
        assert!(source.mark_taken(&timeline));
        assert_eq!(source.route_input("activate_focused_item", None, &mut preview, &mut timeline), Some(true), "Enter keeps the edit");
        assert!(!source.mark_taken(&timeline));
        assert_eq!(timeline.source_in_frame, Some(11), "the moved IN stays");
        assert_eq!(source.route_input("activate_focused_item", None, &mut preview, &mut timeline), None, "nothing taken: the form's Enter");
    }

    #[test]
    fn a_left_click_keeps_a_taken_lane_as_enter_does() {
        let (mut source, mut preview) = (SourceTimeline::new(), SourcePreview::default());
        let mut timeline = TimelineProjection::new(0, 100);
        source.lanes.new_clip();
        source.lanes.set_channels(Some(4));
        assert_eq!(source.route_lane("select_audio_a2", &mut preview), Some(true));
        source.route_lane("lane_level_up", &mut preview);
        assert!(source.lane_taken());
        assert_eq!(source.route_input(KEEP_MARK, None, &mut preview, &mut timeline), Some(true));
        assert!(!source.lane_taken(), "the click kept the lane and closed it");
        assert_eq!(source.kept(1).level_db, 1, "its level is kept");
    }

    #[test]
    fn a_click_lets_go_of_a_mark_drawn_taken_that_nobody_holds_and_never_walks_on() {
        let (mut source, mut preview) = (SourceTimeline::new(), SourcePreview::default());
        let mut timeline = TimelineProjection::new(0, 100).with_playhead(10).with_source_in_at_confirmed();
        timeline.source_mark_focus = qnc_timeline::TimelineSourceMarkFocus::In; // drawn taken, the keys hold nothing
        assert_eq!(source.route_input(KEEP_MARK, None, &mut preview, &mut timeline), Some(true), "taken here, not the form's Enter");
        assert!(!qnc_source_mark_focus::is_taken(&timeline));
    }

    #[test]
    fn another_clip_starts_on_its_own_channels() {
        let (mut source, mut preview, mut timeline) = (with_channels(4), SourcePreview::default(), TimelineProjection::new(0, 100));
        source.clip = Some("a".into());
        source.route("select_audio_a1", &mut preview, &mut timeline);
        source.route("step_next_part", &mut preview, &mut timeline);
        source.route("activate_focused_item", &mut preview, &mut timeline);
        assert_eq!(source.kept(0).channel, 1);
        source.sync(Some("b"), None);
        assert_eq!((source.kept(0).channel, source.kept(1).channel), (0, 1), "A1 on channel 1, A2 on channel 2");
    }
}

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
use qnc_source_dock::{show_timeline_dock, SourceTimeline as DockTimeline};
use qnc_source_panel::SourcePanel;
use qnc_source_preview::SourcePreview;
use qnc_timeline::{lane_choice, lane_key, lane_message, AudioLane, TimelineIntent, TimelineProjection, LaneKey};
use qnc_timeline_assets::SourceTimelineAssets;

pub use qnc_source_channels::ContentTarget;
pub use qnc_source_timecode::SourceTimecode;

pub const MODULE_ID: &str = "qnc.module.source-timeline";
/// The catalog scope of the source timeline's keys, the same in every form.
pub const KEY_SCOPE: &str = qnc_key_intents::SOURCE_TIMELINE_SCOPE;
/// The panel name of the source timeline in the layout contracts (where a click happened).
pub const PANEL: &str = "source_timeline";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The catalog actions of a click on a channel in the A1 or A2 column.
pub const CHOOSE_A1_CHANNEL: &str = "source_choose_a1_channel";
/// A left click on the timeline while IN or OUT is drawn taken: the mark stays where it is
/// and is let go (its own action, never Enter: audit 2026-10-17, the click as Enter walked
/// the panels and the next click pressed Add SubClip).
pub const KEEP_MARK: &str = "source_keep_mark";
/// IN or OUT dragged with the mouse (user 2026-10-17); its value is the frame of the clip.
pub const DRAG_IN: &str = "source_drag_in";
/// The cut (scissors) at the playhead (user 2026-10-18).
pub const CUT: &str = "source_cut";
pub const DRAG_OUT: &str = "source_drag_out";
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
    /// The first cut of a pair (its frame), until the second.
    first_cut: Option<u64>,
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
            self.first_cut = None; // a pair of cuts belongs to its clip
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

    /// The system action a pointer intent on the source timeline stands for, on this panel:
    /// the mouse map's (piece `qnc-mouse-map`, contracts/input/mouse.json; user rule
    /// 2026-10-20), the same in every form.
    pub fn event_of(intent: &TimelineIntent) -> Option<qnc_action_event::ActionEvent> {
        qnc_timeline::event(intent, PANEL, &|frame| frame)
    }

    /// A click on a channel of the column (`action_id` from [`Self::action_of`]), else the
    /// keys of a lane (Ctrl+1/Ctrl+2, and those of a taken lane); `None` for any other.
    /// Enter (or a left click) while IN or OUT is taken keeps the edit and lets the mark go.
    pub fn route_input(&mut self, action_id: &str, value: Option<i64>, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> Option<bool> {
        if action_id == CUT {
            return Some(self.cut(timeline));
        }
        if let (DRAG_IN | DRAG_OUT, Some(frame)) = (action_id, value.and_then(|frame| u64::try_from(frame).ok())) {
            return Some(self.drag(action_id == DRAG_IN, frame, preview, timeline));
        }
        let channel = value.and_then(|channel| u16::try_from(channel).ok());
        if action_id == "activate_focused_item" && !self.lane_taken() && self.keys.mark_taken(timeline) {
            return self.keys.route("clear_focus", preview, timeline);
        }
        if action_id == KEEP_MARK {
            // A left click while something is taken keeps it, then the playhead goes where it
            // was clicked (user rule 2026-10-20); the end of a drag only keeps.
            let kept = if self.lane_taken() {
                self.route_lane("activate_focused_item", preview).unwrap_or(false)
            } else {
                // The timeline draws the mark taken: let it go whoever took it.
                self.keys.set_mark_taken(false);
                qnc_source_mark_focus::release(timeline);
                true
            };
            let cued = value.and_then(|frame| u64::try_from(frame).ok()).is_some_and(|frame| self.cue(&TimelineIntent::CueFrame(frame), preview));
            return Some(kept || cued);
        }
        channel
            .and_then(|channel| self.choose_action(action_id, channel, preview))
            .or_else(|| self.route_lane(action_id, preview))
    }

    /// A cut (scissors) at the playhead (user 2026-10-18: the distance between two cuts is
    /// what is marked, not their order): the first is IN with no OUT, the second marks the
    /// earlier IN and the later OUT, the next starts a new pair. A taken mark is let go.
    pub fn cut(&mut self, timeline: &mut TimelineProjection) -> bool {
        let Some(at) = timeline.playhead_frame else {
            return false;
        };
        self.keys.set_mark_taken(false);
        qnc_source_mark_focus::release(timeline);
        *timeline = match self.first_cut.take() {
            Some(first) if first != at => timeline.clone().with_second_cut_at_confirmed(first),
            _ => {
                self.first_cut = Some(at);
                timeline.clone().with_first_cut_at_confirmed()
            }
        };
        true
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

    /// An intent of the timeline that is the source timeline's own and carries a value: a
    /// channel clicked in the column of A1/A2, IN or OUT dragged. False for any other.
    pub fn timeline_input(&mut self, intent: &TimelineIntent, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> bool {
        match intent {
            TimelineIntent::ChooseA1Channel(channel) => self.choose(0, *channel, preview),
            TimelineIntent::ChooseA2Channel(channel) => self.choose(1, *channel, preview),
            TimelineIntent::DragMark { in_mark, frame } => self.drag(*in_mark, *frame, preview, timeline),
            _ => false,
        }
    }

    /// IN or OUT dragged to `frame` of the clip: taken while it is dragged, the playhead
    /// on the cut; letting the button go keeps it (`KEEP_MARK`).
    pub fn drag(&mut self, in_mark: bool, frame: u64, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> bool {
        let key = qnc_source_mark_focus::drag(timeline, in_mark, frame);
        self.keys.set_mark_taken(key != qnc_source_mark_focus::MarkKey::Refused);
        qnc_source_panel::mark_cue(preview, key)
    }

    /// A click on a channel in the column of A1 (lane 0) or A2: on the taken lane it only
    /// drafts (Enter keeps it), otherwise it is kept at once. Heard at once.
    pub fn choose(&mut self, lane: u8, channel: u16, preview: &mut SourcePreview) -> bool {
        // The click on a channel of a taken lane is the timeline's own lane key, the same on
        // every timeline (Wrap rows too; user 2026-10-18: written once, used everywhere).
        let changed = if self.lanes.taken().is_some_and(|(taken, _)| taken == lane) {
            qnc_timeline::lane_key(&mut self.lanes, qnc_timeline::LaneKey::Pick(channel))
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
    (theme, header_gap): (&qnc_desktop_theme::Theme, f32),
    title: &str,
    header: impl FnOnce(&mut Ui),
    (projection, assets): (&TimelineProjection, &SourceTimelineAssets),
    view: &SourceTimelineView,
) -> TimelineIntent {
    let style = &qnc_theme_styles::dock(theme, header_gap);
    let timeline = DockTimeline::from_assets(projection, qnc_theme_styles::timeline(theme), assets)
        .with_timecode(view.timecode)
        .with_a1_channel(view.a1)
        .with_a2_channel(view.a2)
        .with_wave_over_video(view.wave_over_video);
    show_timeline_dock(ui, rect, style, title, header, timeline)
}

/// One action button of the header, as a form lists them (label, catalog action).
pub struct HeaderAction<'a> {
    pub label: &'a str,
    pub action_id: Option<&'a str>,
    /// The strong button of the header (the form's main action).
    pub strong: bool,
}

/// The header's action buttons, right to left as listed, each with its state from the form
/// (enabled, on) and the frame of the one the keyboard is on; the clicked action.
pub fn header_actions<'a>(
    ui: &mut Ui,
    theme: &qnc_desktop_theme::Theme,
    actions: impl IntoIterator<Item = HeaderAction<'a>>,
    enabled: impl Fn(&str) -> bool,
    on: impl Fn(&str) -> bool,
    focused: Option<&str>,
) -> Option<&'a str> {
    let mut clicked = None;
    for action in actions {
        let id = action.action_id.unwrap_or_default();
        let (enabled, on) = (action.action_id.is_some_and(&enabled), action.action_id.is_some_and(&on));
        let response = if action.strong {
            qnc_ui_kit::strong_button(ui, action.label, enabled, theme.accent, theme.chrome_control_height)
        } else {
            qnc_ui_kit::chrome_button(ui, action.label, enabled, on, qnc_theme_styles::button(theme))
        };
        qnc_ui_kit::paint_focus(ui, response.rect.expand(2.0), action.action_id.is_some() && focused == Some(id), theme.focus);
        if response.clicked() {
            clicked = action.action_id;
        }
    }
    clicked
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
        let of = |intent| SourceTimeline::event_of(&intent).map(|event| (event.action_id, event.value, event.at));
        assert_eq!(of(TimelineIntent::TakeAudioLane(AudioLane::A2)), Some(("select_audio_a2".into(), None, Some(PANEL.into()))));
        assert_eq!(of(TimelineIntent::ChooseA2Channel(3)), Some((CHOOSE_A2_CHANNEL.into(), Some(3), Some(PANEL.into()))));
        assert_eq!(of(TimelineIntent::CueFrame(3)), Some(("timeline_cue_frame".into(), Some(3), Some(PANEL.into()))), "a cue is a system action too");
    }

    #[test]
    fn enter_or_a_left_click_keeps_a_taken_mark_and_a_right_click_takes_one() {
        assert_eq!(SourceTimeline::event_of(&TimelineIntent::Keep(None)).map(|event| event.action_id), Some(KEEP_MARK.into()), "a click is never Enter");
        assert_eq!(SourceTimeline::event_of(&TimelineIntent::TakeMark(false)).map(|event| event.action_id), Some("select_mark_out".into()));
        // Scissors: two cuts mark the distance, whatever their order; the third starts again.
        let mut source = SourceTimeline::new();
        let mut marks = TimelineProjection::new(0, 100).with_playhead(50);
        assert!(source.cut(&mut marks));
        assert_eq!((marks.source_in_frame, marks.source_out_frame), (Some(50), None));
        marks = marks.with_playhead(20);
        source.cut(&mut marks);
        assert_eq!((marks.source_in_frame, marks.source_out_frame), (Some(20), Some(50)), "the earlier is IN");
        marks = marks.with_playhead(70);
        source.cut(&mut marks);
        assert_eq!((marks.source_in_frame, marks.source_out_frame), (Some(70), None), "a new pair");
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

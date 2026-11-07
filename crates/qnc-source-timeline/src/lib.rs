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

mod level;

use qnc_lane_choice::{Lane, LaneChoice};
use qnc_source_panel::SourcePanel;
use qnc_source_preview::SourcePreview;
use qnc_lane_choice::{lane_choice, lane_key, lane_message, LaneKey};
use qnc_timeline_model::{AudioLane, TimelineIntent, TimelineProjection};

pub use qnc_source_channels::ContentTarget;
pub use qnc_source_timecode::SourceTimecode;

pub const MODULE_ID: &str = "qnc.module.source-timeline";
/// The catalog scope of the source timeline's keys, the same in every form.
pub const KEY_SCOPE: &str = qnc_timeline_model::SOURCE_TIMELINE_SCOPE;
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
    pub a1: Option<qnc_lane_choice::ChannelChoice>,
    pub a2: Option<qnc_lane_choice::ChannelChoice>,
    pub wave_over_video: Option<(AudioLane, u8)>,
    pub timecode: Option<SourceTimecode>,
    /// What a taken lane does, for the form's status line (empty: nothing to say).
    pub message: String,
    /// The peaks (dBFS) the player measured at its outputs A1 and A2, for the level bar in
    /// the middle of the source timeline's bar (user 2026-10-29: in every form).
    pub levels: [Option<f32>; 2],
    /// NORM's state on that bar; `None` where the form has no story to normalise.
    pub normalize: Option<bool>,
    /// The sound assistant's (AI) state on that bar (user 2026-11-06); `None` the same.
    pub sound_assist: Option<bool>,
    /// A question about what the source timeline's marks would save (the same statement
    /// already there, user 2026-11-01), shown over everything until answered.
    pub question: Option<qnc_notice::Choice>,
    /// The clip's A1/A2 level curves on its frames (the taken one as drafted; user 2026-11-07).
    pub level_curves: Vec<qnc_program_view::LevelCurve>,
}

impl SourceTimelineView {
    /// This view with `question` shown over everything.
    pub fn with_question(mut self, question: Option<qnc_notice::Choice>) -> Self {
        self.question = question;
        self
    }
}

/// The state of the source timeline of one form.
#[derive(Debug, Default)]
pub struct SourceTimeline {
    keys: SourcePanel,
    lanes: LaneChoice,
    channels: qnc_source_channels::SourceChannels,
    /// The A1/A2 sound kept for each clip in the project database (user 2026-10-24).
    sounds: qnc_clip_sound::ClipSounds,
    /// The shown clip's kept sound is applied (or it has none).
    sound_known: bool,
    /// The kept sound of the shown clip as last read: a different answer (another form or
    /// computer kept it) is applied while no lane is taken.
    sound_read: Option<Option<qnc_clip_sound::ClipSound>>,
    /// A sound asked for a clip, applied when that clip is shown.
    wanted: Option<qnc_clip_sound::ClipSound>,
    timecodes: qnc_source_timecode::SourceTimecodes,
    clip: Option<String>,
    message: String,
    view: SourceTimelineView,
    /// The first cut of a pair (its frame), until the second.
    first_cut: Option<u64>,
    /// The first double clicked word of a pair (its frames), until the second.
    first_word: qnc_edit_words::WordPair<()>,
    /// The kept level curves of A1 and A2 of the shown clip (user 2026-11-07), clip frames.
    curves: [Vec<(u64, i16)>; 2],
    /// The curve taken (Ctrl+Shift+1/2, a right click on its line), as drafted.
    level: Option<qnc_level_curve::LevelLine>,
}

impl SourceTimeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Once a frame: the shown clip and the project content its stored record is read
    /// from. Another clip starts on the A1/A2 sound kept for it in the project database,
    /// else on A1 channel 1 and A2 channel 2 (user rules 2026-10-01, 2026-10-24).
    pub fn sync(&mut self, clip_id: Option<&str>, content: Option<&ContentTarget>, preview: &mut SourcePreview, timeline: &TimelineProjection) -> &SourceTimelineView {
        self.keys.stop_on_out(preview, timeline);
        if self.clip.as_deref() != clip_id {
            self.clip = clip_id.map(str::to_string);
            self.first_cut = None; // a pair of cuts belongs to its clip
            self.first_word.forget();
            self.lanes.new_clip();
            self.sound_known = false;
            self.sound_read = None;
            self.message.clear();
            (self.curves, self.level) = ([Vec::new(), Vec::new()], None);
        }
        let count = self.channels.for_clip(content, clip_id);
        self.lanes.set_channels(count);
        self.apply_kept_sound(count, preview);
        // The clip's level curves in its player (sent again to every new session).
        self.hear_curves(preview);
        let timecode = self.timecodes.for_clip(content, clip_id);
        self.refresh(timecode);
        self.view.levels = preview.view().output_levels;
        self.view.level_curves = self.drawn_curves(timeline);
        &self.view
    }

    /// The sound `sound` names for its clip is heard and kept as soon as the clip is shown and
    /// its channels are known (user 2026-10-30: a take of a voice record plays on its channel).
    pub fn want_sound(&mut self, sound: qnc_clip_sound::ClipSound) {
        self.wanted = Some(sound);
    }

    /// A1 of `clip_id` on `channel` (zero based), A2 the same channel turned off: one channel
    /// heard, as a take of a voice record is (user 2026-10-30).
    pub fn want_a1(&mut self, clip_id: &str, channel: u16) {
        let lane = |off| qnc_clip_sound::LaneSound { channel, off, level_db: 0 };
        self.want_sound(qnc_clip_sound::ClipSound { clip_id: clip_id.into(), a1: lane(false), a2: lane(true), a1_keys: Vec::new(), a2_keys: Vec::new() });
    }

    /// NORM's state for the level bar (a form with a story gives it; `None`: NORM off).
    pub fn offer_normalize(&mut self, on: Option<bool>) {
        self.view.normalize = on;
    }

    /// The sound assistant's state for the level bar (user 2026-11-06; `None`: AI off).
    pub fn offer_assist(&mut self, on: Option<bool>) {
        self.view.sound_assist = on;
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

    /// The A1 and A2 level curves kept for the shown clip (user 2026-11-07): a new segment or
    /// cover of the clip takes them.
    pub fn kept_curves(&self) -> [Vec<(u64, i16)>; 2] {
        self.curves.clone()
    }

    /// A1 and A2 as its lanes show them now: the lane playing, its channels and whether it is
    /// taken (its trim zoom shows the same lanes, AGENTS 0.12).
    pub fn lanes_shown(&self) -> [Option<(Lane, u16, bool)>; 2] {
        [0, 1].map(|lane| {
            let (_, count) = self.lanes.choice(lane)?;
            Some((self.lanes.lane(lane), count, self.lanes.taken().is_some_and(|(taken, _)| taken == lane)))
        })
    }

    /// A lane is taken (its keys belong to it).
    pub fn lane_taken(&self) -> bool {
        self.lanes.taken().is_some()
    }

    /// A lane waits for Enter, or this Enter kept it: the same Enter does nothing else.
    pub fn holds_enter(&self) -> bool {
        self.lane_taken() || self.level_taken() || self.lanes.committed_within(SAME_ENTER)
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
        qnc_timeline_model::event(intent, PANEL, &|frame| frame)
    }

    /// A click on a channel of the column (`action_id` from [`Self::action_of`]), else the
    /// keys of a lane (Ctrl+1/Ctrl+2, and those of a taken lane); `None` for any other.
    /// Enter (or a left click) while IN or OUT is taken keeps the edit and lets the mark go.
    pub fn route_input(&mut self, action_id: &str, value: Option<i64>, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> Option<bool> {
        if action_id == CUT {
            return Some(self.cut(timeline, value.and_then(|frame| u64::try_from(frame).ok())));
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
    /// earlier IN and the later OUT, the next starts a new pair. A taken mark is let go. `at`
    /// is the frame of a double click (user 2026-10-26), else the cut is at the playhead.
    pub fn cut(&mut self, timeline: &mut TimelineProjection, at: Option<u64>) -> bool {
        let Some(at) = at.or(timeline.playhead_frame) else {
            return false;
        };
        self.keys.set_mark_taken(false);
        qnc_source_mark_focus::release(timeline);
        self.first_word.forget();
        *timeline = match self.first_cut.take() {
            Some(first) if first != at => timeline.clone().with_second_cut(first, at),
            _ => {
                self.first_cut = Some(at);
                timeline.clone().with_first_cut_at(at)
            }
        };
        true
    }

    /// A cut at a word said over clip frames `[from, to)` (user 2026-10-26: a double click on
    /// a word of the transcript): the first is IN at its start; the second marks both words
    /// whole, whatever their order: IN at the smaller timecode (the earlier word's start), OUT
    /// at the larger (the later word's end; user 2026-10-26). The next starts a new pair.
    pub fn cut_word(&mut self, timeline: &mut TimelineProjection, from: u64, to: u64) -> bool {
        self.keys.set_mark_taken(false);
        qnc_source_mark_focus::release(timeline);
        self.first_cut = None;
        // The one meaning of two words (`qnc-edit-words`, AGENTS 0.12).
        *timeline = match self.first_word.give((), qnc_edit_words::Word::new(from, to)) {
            qnc_edit_words::Paired::Range(range) => timeline.clone().with_second_cut(range.from, range.to),
            qnc_edit_words::Paired::First(word) => timeline.clone().with_first_cut_at(word.from),
        };
        true
    }

    /// A word said over clip frames `[from, to)` with IN or OUT taken: the mark goes to it (IN
    /// to its start, OUT to its end; `qnc-edit-words`) and stays taken. `None` with no mark
    /// taken.
    pub fn mark_to_word(&mut self, from: u64, to: u64, preview: &mut SourcePreview, timeline: &mut TimelineProjection) -> Option<bool> {
        if !self.mark_taken(timeline) {
            return None;
        }
        let edge = if timeline.source_mark_focus == qnc_timeline_model::TimelineSourceMarkFocus::In { qnc_edit_edge::Edge::In } else { qnc_edit_edge::Edge::Out };
        Some(self.drag(edge == qnc_edit_edge::Edge::In, qnc_edit_words::Word::new(from, to).goal(Some(edge), 0), preview, timeline))
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
            qnc_lane_choice::lane_key(&mut self.lanes, qnc_lane_choice::LaneKey::Pick(channel))
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
        // The level curve of A1/A2 (user 2026-11-07), the same tool as on a Wrap row.
        if let Some(changed) = self.level_key(action_id, preview, timeline) {
            return Some(changed);
        }
        // Ctrl+wheel while the curve is taken: its point a dB, as Ctrl+arrows.
        if self.level_taken() {
            if let Some(steps) = qnc_edit_step::ctrl_frames(action_id).and_then(|_| qnc_edit_step::level(action_id)) {
                return self.level_key(qnc_edit_step::level_action(steps), preview, timeline);
            }
        }
        if let Some(changed) = self.route_lane(action_id, preview) {
            return Some(changed);
        }
        match action_id {
            // The wheel is the left/right arrow; while a lane is taken (where the arrows
            // turn it off/on) it moves the playhead (user 2026-10-16). Ctrl+wheel is
            // Ctrl+left/right: the level of a taken lane, up louder.
            // One step by one key (`qnc-edit-step`, the same on every timeline and its zoom).
            wheel if qnc_edit_step::ctrl_frames(wheel).is_some() => {
                let level = qnc_edit_step::level_action(qnc_edit_step::level(wheel)?);
                if let Some(changed) = self.route_lane(level, preview) {
                    return Some(changed);
                }
                // No lane taken: Ctrl+wheel moves the taken IN or OUT, as the wheel does
                // (user 2026-10-17: right button and wheel cut the clip).
                if !self.keys.mark_taken(timeline) {
                    return Some(false);
                }
                self.keys.route(qnc_edit_step::arrow(qnc_edit_step::ctrl_frames(wheel)?), preview, timeline)
            }
            wheel if qnc_edit_step::is_wheel(wheel) => {
                let frames = qnc_edit_step::frames(wheel)?;
                if self.lane_taken() {
                    return Some(preview.step(frames));
                }
                self.keys.route(qnc_edit_step::arrow(frames), preview, timeline)
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

    /// The sound kept for the shown clip, once it and the clip's channel count are read,
    /// unless a lane is taken; a channel the clip does not have is not taken. The preview
    /// hears it.
    fn apply_kept_sound(&mut self, count: Option<u16>, preview: &mut SourcePreview) {
        // A sound asked for this clip (a take of a voice record: A1 on its channel) is what
        // the lanes keep from now on, kept for the clip like a channel chosen.
        if let (Some(wanted), Some(count)) = (self.wanted.as_ref(), count) {
            if self.clip.as_deref() == Some(wanted.clip_id.as_str()) && !self.lane_taken() {
                let wanted = self.wanted.take().unwrap();
                if wanted.a1.channel < count && wanted.a2.channel < count {
                    let lane = |sound: qnc_clip_sound::LaneSound| Lane { channel: sound.channel, off: sound.off, level_db: sound.level_db };
                    self.lanes.keep(0, lane(wanted.a1));
                    self.lanes.keep(1, lane(wanted.a2));
                    self.sound_known = true;
                    self.after_lane(true, preview);
                    return;
                }
            }
        }
        if self.lane_taken() {
            return;
        }
        let Some(count) = count else {
            return;
        };
        let Some(kept) = self.sounds.for_clip(preview.project_target(), self.clip.as_deref()) else {
            return;
        };
        // Applied once per clip, and again when the database answers something else.
        if self.sound_known && self.sound_read.as_ref() == Some(&kept) {
            return;
        }
        self.sound_read = Some(kept.clone());
        self.sound_known = true;
        // The clip's level curves as kept (none when it keeps none); a curve being drawn stays.
        if !self.level_taken() {
            self.curves = kept.as_ref().map_or_else(|| [Vec::new(), Vec::new()], |kept| [kept.a1_keys.clone(), kept.a2_keys.clone()]);
        }
        if let Some(kept) = kept.filter(|kept| kept.a1.channel < count && kept.a2.channel < count) {
            let lane = |sound: qnc_clip_sound::LaneSound| Lane { channel: sound.channel, off: sound.off, level_db: sound.level_db };
            self.lanes.keep(0, lane(kept.a1));
            self.lanes.keep(1, lane(kept.a2));
            self.after_lane(true, preview);
        }
    }

    /// What the lanes keep now is kept for the shown clip (a lane kept with Enter or a
    /// channel clicked), so the clip is heard the same next time and its new segments and
    /// covers take it.
    fn keep_sound(&mut self, preview: &SourcePreview) {
        let (Some(clip_id), Some(target), true) = (self.clip.clone(), preview.project_target(), self.sound_known) else {
            return;
        };
        if self.lane_taken() {
            return;
        }
        let lane = |kept: Lane| qnc_clip_sound::LaneSound { channel: kept.channel, off: kept.off, level_db: kept.level_db };
        let [a1, a2] = self.kept_lanes();
        let [a1_keys, a2_keys] = self.curves.clone();
        let sound = qnc_clip_sound::ClipSound { clip_id, a1: lane(a1), a2: lane(a2), a1_keys, a2_keys };
        let default = |channel: u16| qnc_clip_sound::LaneSound { channel, off: false, level_db: 0 };
        let no_curve = sound.a1_keys.is_empty() && sound.a2_keys.is_empty();
        if sound.a1 == default(0) && sound.a2 == default(1) && no_curve && self.sounds.for_clip(Some(target), self.clip.as_deref()) == Some(None) {
            return; // the default of a clip that keeps nothing is not written
        }
        self.sounds.keep(target, sound);
    }

    /// The preview hears what the lanes play now; the view follows.
    fn after_lane(&mut self, changed: bool, preview: &mut SourcePreview) -> bool {
        if changed {
            self.keep_sound(preview);
        }
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
            levels: self.view.levels,
            normalize: self.view.normalize,
            sound_assist: self.view.sound_assist,
            question: None,
            level_curves: std::mem::take(&mut self.view.level_curves),
        };
    }
}

/// The wave over the video row: the taken lane's, else A1's, else A2's.
fn wave_over_video(lanes: &LaneChoice) -> Option<(AudioLane, u8)> {
    let (zoom, names) = (lanes.wave_zoom(), [AudioLane::A1, AudioLane::A2]);
    let taken = lanes.taken().map(|(lane, _)| usize::from(lane.min(1)));
    taken.into_iter().chain([0, 1]).find(|lane| zoom[*lane] > 0).map(|lane| (names[lane], zoom[lane]))
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

    /// The level line of the source timeline is a Wrap row's (user 2026-11-07): Ctrl+Shift+2 takes
    /// A2's curve, up raises the point on the playhead line a dB at the clip's frame there, the
    /// draft is drawn, Enter keeps it for the clip (a new segment or cover takes it), Esc of a
    /// new one leaves the kept one.
    #[test]
    fn the_clips_level_line_is_taken_raised_and_kept() {
        let (mut source, mut preview, mut timeline) = (with_channels(4), SourcePreview::default(), TimelineProjection::new(0, 100));
        source.clip = Some("c".into());
        timeline.playhead_frame = Some(40);
        assert_eq!(source.route("select_level_a2", &mut preview, &mut timeline), Some(true));
        assert!(source.holds_enter(), "Enter is the curve's");
        source.route("step_prev_part", &mut preview, &mut timeline);
        source.route("step_prev_part", &mut preview, &mut timeline);
        assert_eq!(source.heard_curves()[1], vec![(40, 20)], "+2 dB at clip frame 40, heard at once");
        assert_eq!(source.drawn_curves(&timeline).iter().map(|curve| (curve.lane, curve.taken)).collect::<Vec<_>>(), vec![(0, false), (1, true)], "both lines are drawn, A1 flat");
        source.route("activate_focused_item", &mut preview, &mut timeline);
        assert!(!source.level_taken());
        assert_eq!(source.kept_curves(), [Vec::new(), vec![(40, 20)]], "kept for the clip");
        source.route("select_level_a2", &mut preview, &mut timeline);
        source.route("step_next_part", &mut preview, &mut timeline);
        source.route("clear_focus", &mut preview, &mut timeline);
        assert_eq!(source.kept_curves()[1], vec![(40, 20)], "Esc keeps what was kept");
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
        assert!(source.cut(&mut marks, None));
        assert_eq!((marks.source_in_frame, marks.source_out_frame), (Some(50), None));
        marks = marks.with_playhead(20);
        source.cut(&mut marks, None);
        assert_eq!((marks.source_in_frame, marks.source_out_frame), (Some(20), Some(50)), "the earlier is IN");
        marks = marks.with_playhead(70);
        source.cut(&mut marks, None);
        assert_eq!((marks.source_in_frame, marks.source_out_frame), (Some(70), None), "a new pair");
        // Two double clicked words: IN at the start of the first, OUT at the end of the later.
        let mut words = SourceTimeline::new();
        let mut marked = TimelineProjection::new(0, 100);
        words.cut_word(&mut marked, 10, 15);
        words.cut_word(&mut marked, 40, 48);
        assert_eq!((marked.source_in_frame, marked.source_out_frame), (Some(10), Some(48)));
        // The later word first: IN is still the smaller timecode, both words whole.
        words.cut_word(&mut marked, 40, 48);
        words.cut_word(&mut marked, 10, 15);
        assert_eq!((marked.source_in_frame, marked.source_out_frame), (Some(10), Some(48)));
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
        timeline.source_mark_focus = qnc_timeline_model::TimelineSourceMarkFocus::In; // drawn taken, the keys hold nothing
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
        source.sync(Some("b"), None, &mut SourcePreview::new(), &timeline);
        assert_eq!((source.kept(0).channel, source.kept(1).channel), (0, 1), "A1 on channel 1, A2 on channel 2");
    }
}

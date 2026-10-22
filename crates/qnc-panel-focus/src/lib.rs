//! Keyboard focus of an editorial surface (QNC v5 `story/focus.rs` `PanelFocus`):
//! the QNC keyboard map acts on the panel in focus, not on the whole application
//! (user rule 2026-09-25). Panels: the media pool, the source timeline (only I/O
//! and its playhead) and the segment panel (Wrap, markers, covers). Tab cycles
//! them; a click in a panel focuses it.
//!
//! This component routes one catalog `action_id` to the public pieces the focused
//! panel drives: the source preview (Broadcast Player), the Wrap view and the
//! program model. It owns no player, database or form, and knows no application.

use std::ops::ControlFlow;

use qnc_action_event::ActionEvent;
use qnc_program_segments::ProgramSegments;
use qnc_source_preview::SourcePreview;
use qnc_source_mark_focus::MarkKey;
use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_wrap_session::WrapSession;

pub const MODULE_ID: &str = "qnc.module.panel-focus";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A panel of an editorial surface that takes the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Panel {
    /// The tabs of the pool head (All, SubClip, B-roll, Segment).
    PoolTabs,
    Pool,
    /// v5: choosing a clip puts the keyboard on its source timeline.
    #[default]
    SourceTimeline,
    /// The action buttons in the head of the source dock.
    SourceButtons,
    Segments,
}

impl Panel {
    /// The name of the panel in a layout contract and in the focus walk.
    pub fn name(self) -> &'static str {
        match self {
            Self::PoolTabs => qnc_focus_walk::POOL_TABS,
            Self::Pool => qnc_focus_walk::CARDS,
            Self::SourceTimeline => qnc_focus_walk::SOURCE_TIMELINE,
            Self::SourceButtons => qnc_focus_walk::SOURCE_BUTTONS,
            Self::Segments => "segments",
        }
    }

    /// The panel a layout contract names in its `tab_order`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "pool_tabs" => Self::PoolTabs,
            "pool" => Self::Pool,
            "source_timeline" => Self::SourceTimeline,
            "source_buttons" => Self::SourceButtons,
            "segments" => Self::Segments,
            _ => return None,
        })
    }
}

/// The Tab walk of one form (user 2026-10-13: it belongs to the form, from its layout
/// contract): its panels in order, the action ids of its pool tabs and of its source
/// dock buttons, left to right.
#[derive(Debug, Clone)]
pub struct FocusLayout {
    pub order: Vec<Panel>,
    pub tabs: Vec<String>,
    pub buttons: Vec<String>,
}

impl FocusLayout {
    pub fn new((order, tabs, buttons): (Vec<&str>, Vec<&str>, Vec<&str>)) -> Self {
        let owned = |names: Vec<&str>| names.into_iter().map(str::to_string).collect();
        Self { order: order.into_iter().filter_map(Panel::from_name).collect(), tabs: owned(tabs), buttons: owned(buttons) }
    }
}

impl Default for FocusLayout {
    /// The walk without a layout: pool, source timeline, segments.
    fn default() -> Self {
        Self { order: vec![Panel::Pool, Panel::SourceTimeline, Panel::Segments], tabs: Vec::new(), buttons: Vec::new() }
    }
}

/// A key on the media pool: the card focus moves, or Enter opens the focused card (the
/// public card grid applies it).
pub use qnc_key_list::GridStep as PoolStep;

/// What a form draws of the focus: the panel, on the source dock buttons the action of
/// the one the arrows are on, and the last key on the media pool with its number (the
/// card grid applies each number once, however often the view is read).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusView {
    pub panel: Panel,
    pub button: Option<String>,
    pub pool_step: Option<(PoolStep, u64)>,
}

impl FocusView {
    /// The catalog scope that reads the keys first: the source timeline's own while it (or
    /// its buttons) has the keyboard (piece `qnc-source-timeline`).
    pub fn key_scope(&self) -> Option<&'static str> {
        matches!(self.panel, Panel::SourceTimeline | Panel::SourceButtons).then_some(qnc_source_timeline::KEY_SCOPE)
    }
}

/// Actions of the segment panel that also run from the other panels: they use
/// the source the user marked (v5 dispatches them whatever the focus).
const SOURCE_TO_PROGRAM: [&str; 6] = [
    "add_ton_segment",
    "add_off_segment",
    "quick_overwrite_cover",
    "overwrite_cover",
    // The switch Pokrivalice | Sync/B-roll sits on the source dock (user 2026-10-08).
    "cover_mode_covers",
    "cover_mode_sync",
];

/// What the answer of a key picture search moves.
#[derive(Debug, Clone, Copy)]
enum Jump {
    SourceMark,
    Program,
}

#[derive(Debug, Default)]
pub struct PanelFocus {
    panel: Panel,
    /// The panel of the last poll: leaving the segment panel drops what it took.
    last_panel: Panel,
    /// Ctrl+I or Ctrl+O took a source mark: the arrows move it, not the playhead
    /// (an I or O press only marks; the playhead keeps the arrows, v5).
    /// The one source timeline (piece `qnc-source-timeline`): its keys, the wheel and the
    /// A1/A2 lanes, the same in every form.
    source: qnc_source_timeline::SourceTimeline,
    /// Up/Down on a taken edit: the key picture search and what its answer moves.
    keyframes: qnc_keyframe_nav::KeyframeNav,
    jump: Option<Jump>,
    /// The one focus walk of every form (piece `qnc-focus-walk`): the panels in the form's
    /// order, the pool tabs and the source dock buttons the arrows walk, and the keys on the
    /// media pool as numbered steps for the card grid (a step is not taken on read: one arrow
    /// fires two catalog actions and every action reads the view again, live 2026-10-13;
    /// Enter that opened a card puts the keyboard on its source timeline).
    walk: qnc_focus_walk::FocusWalk,
    /// The catalog action a key on the pool tabs or a dock button stands for; the
    /// caller runs it as its own action (`take_pick`).
    pick: Option<String>,
}

/// The walk of a layout: its panels, the pool tabs row and the source dock buttons row.
fn walk_of(layout: &FocusLayout) -> qnc_focus_walk::FocusWalk {
    let order = layout.order.iter().map(|panel| panel.name()).collect();
    let (tabs, buttons) = (layout.tabs.iter().map(String::as_str).collect(), layout.buttons.iter().map(String::as_str).collect());
    qnc_focus_walk::FocusWalk::new((order, vec![(qnc_focus_walk::POOL_TABS, tabs), (qnc_focus_walk::SOURCE_BUTTONS, buttons)], Vec::new()))
}

impl PanelFocus {
    pub fn new() -> Self {
        Self { walk: walk_of(&FocusLayout::default()), ..Self::default() }
    }

    /// The focus of a form with its Tab walk; it starts on the first panel of the walk.
    pub fn with_layout(layout: FocusLayout) -> Self {
        let panel = layout.order.first().copied().unwrap_or_default();
        Self { panel, walk: walk_of(&layout), ..Self::default() }
    }

    /// The action a key on the pool tabs or a dock button stood for, once.
    pub fn take_pick(&mut self) -> Option<String> {
        self.pick.take()
    }

    /// A mark taken with Ctrl+I/O that the source timeline still has in focus.
    fn taken(&mut self, timeline: &TimelineProjection) -> bool {
        self.source.mark_taken(timeline)
    }

    /// The source timeline this form embeds.
    pub fn source_timeline(&self) -> &qnc_source_timeline::SourceTimeline {
        &self.source
    }

    pub fn source_timeline_mut(&mut self) -> &mut qnc_source_timeline::SourceTimeline {
        &mut self.source
    }

    pub fn panel(&self) -> Panel {
        self.panel
    }

    /// Applies the answer of an Up/Down key picture search to the edit it was asked
    /// for (the taken source mark, or the taken slot or marker); returns the panel.
    pub fn poll(
        &mut self,
        (preview, wrap, segments, timeline): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection),
    ) -> FocusView {
        // What the segment panel took is its own: left by a click, Tab or Enter, it is dropped.
        let last = std::mem::replace(&mut self.last_panel, self.panel);
        if last == Panel::Segments && self.panel != Panel::Segments {
            if qnc_segments_keys::left(preview, wrap, segments) && qnc_dev_diagnostics::player_diagnostics_enabled() {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("story-leave panel={:?} dropped the segment panel's taken edit", self.panel));
            }
        }
        // The media pool chosen: the enlarged slot closes and the source timeline is back (user
        // 2026-10-22).
        if last != self.panel && matches!(self.panel, Panel::Pool | Panel::PoolTabs) && segments.view().zoom.is_some() {
            segments.apply(qnc_program_segments::SegmentCommand::CloseZoom);
        }
        qnc_segments_keys::dropped(preview, wrap, segments); // another edit took over
        qnc_sync_session::poll(preview, wrap, segments); // an IN pressed while the source plays starts Sync
        if let Some(answer) = self.keyframes.take() {
            match (self.jump.take(), answer) {
                (Some(Jump::SourceMark), Ok(frames)) => {
                    mark_cue(preview, qnc_source_mark_focus::nudge(timeline, frames));
                }
                (Some(Jump::Program), Ok(frames)) => {
                    segments.jump_edit(frames);
                }
                (_, Err(message)) => segments.say(message),
                (None, Ok(_)) => {}
            }
        }
        let button = (self.panel == Panel::SourceButtons).then(|| self.walk.chosen_in(qnc_focus_walk::SOURCE_BUTTONS).map(str::to_string)).flatten();
        FocusView { panel: self.panel, button, pool_step: self.walk.last_step() }
    }

    /// Whether a key picture search runs (the caller repaints soon).
    pub fn busy(&self) -> bool {
        self.keyframes.busy()
    }

    /// A clip chosen in the media pool: the Source view; the keyboard stays on the pool
    /// (a click), or goes on to the source timeline when Enter opened the card (user
    /// 2026-10-13: the keys belong to the panel in use).
    pub fn chose_clip(&mut self, wrap: &mut WrapSession) -> bool {
        wrap.leave();
        self.panel = if self.walk.take_opening() { Panel::SourceTimeline } else { Panel::Pool };
        true
    }

    /// A clip chosen or a click on the source timeline: the keyboard goes to the
    /// source timeline and the Wrap view gives way to the Source view (v5).
    pub fn to_source(&mut self, wrap: &mut WrapSession) {
        self.panel = Panel::SourceTimeline;
        wrap.leave();
    }

    /// A click in a panel focuses it.
    pub fn set(&mut self, panel: Panel) {
        self.panel = panel;
    }

    /// Routes one system action, from any input device (user 2026-10-18), to the panel it
    /// belongs to: a key to the panel in focus, a pointer to the panel it was on. `None`: not
    /// a panel action (the application handles it); `Some(changed)`: taken here, done or ignored.
    pub fn route(
        &mut self,
        event: impl Into<ActionEvent>,
        pieces: (
            &mut SourcePreview,
            &mut WrapSession,
            &mut ProgramSegments,
            &mut TimelineProjection,
        ),
    ) -> Option<bool> {
        let event = event.into();
        let action_id = event.action_id.as_str();
        let panel = self.panel;
        let routed = match self.pointer(&event, (&mut *pieces.0, &mut *pieces.1, &mut *pieces.2, &mut *pieces.3)) {
            Some(answer) => answer,
            None => self.route_on_panel(action_id, (&mut *pieces.0, &mut *pieces.1, &mut *pieces.2, &mut *pieces.3)),
        };
        // Every key of the story in the player log (live 2026-10-16: IN/OUT not taken
        // after some action): the action, the panel it met, what came of it and the source
        // marks and playhead as the key found them.
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            let (preview, timeline) = (&*pieces.0, &*pieces.3);
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!(
                    "story-key action={action_id} value={:?} target={:?} at={:?} panel={panel:?}->{:?} routed={routed:?} in={:?} out={:?} playhead={:?} program={} part_sound={} marker={} edge={} message={:?}",
                    event.value,
                    event.target,
                    event.at,
                    self.panel,
                    timeline.source_in_frame,
                    timeline.source_out_frame,
                    timeline.playhead_frame,
                    preview.in_program(),
                    pieces.2.part_sound_taken(),
                    pieces.2.editing_marker(),
                    pieces.2.trimming_edge(),
                    pieces.2.view().message,
                ),
            );
        }
        routed
    }

    /// An event of a pointer (a click, a drag, the wheel) on a panel: the panel takes the
    /// keyboard (a click in a panel focuses it); what names its object or frame is answered
    /// here, the rest goes on as the same key on that panel. `None`: go on as a key.
    fn pointer(
        &mut self,
        event: &ActionEvent,
        (preview, wrap, segments, timeline): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection),
    ) -> Option<Option<bool>> {
        if event.is_at(qnc_segment_panel_name()) {
            self.panel = Panel::Segments;
            // A left click while something is taken on a Wrap row keeps it (as Enter, without
            // walking on), then the playhead goes where it was clicked (user rule 2026-10-20).
            if event.action_id == "segments_keep_edit" {
                let kept = qnc_segments_keys::confirm(preview, segments);
                let cued = event.unsigned().is_some_and(|frame| segments.apply(qnc_program_segments::SegmentCommand::Cue(frame)));
                return Some(Some(kept || cued));
            }
            // The marker, slot, cover, frame or channel the pointer named (piece
            // `qnc-program-commands`: one map for keys and pointers).
            return match qnc_program_segments::SegmentCommand::from_event(event)? {
                qnc_program_segments::SegmentCommand::PickChannel { lane, channel } => {
                    Some(Some(qnc_segments_keys::pick(lane, channel, (preview, wrap, segments))))
                }
                command => Some(Some(segments.apply(command))),
            };
        }
        if !event.is_at(qnc_source_timeline::PANEL) {
            return None;
        }
        // A click on the source timeline: the Source view, the playhead where it points (in
        // every form the same); a dragged IN/OUT or a channel picked with its value.
        let id = event.action_id.as_str();
        match id {
            "timeline_cue_frame" | "playlist_input_start" => {
                self.to_source(wrap);
                let intent = event.unsigned().filter(|_| id == "timeline_cue_frame").map_or(TimelineIntent::GoHome, TimelineIntent::CueFrame);
                Some(Some(self.source.cue(&intent, preview)))
            }
            // A left click while IN/OUT or A1/A2 is taken keeps it, then the playhead goes there.
            qnc_source_timeline::KEEP_MARK => {
                self.panel = Panel::SourceTimeline;
                Some(self.source.route_input(id, event.value, preview, timeline))
            }
            // A stopped Sync's OUT is the end of its cover: the mouse moves it as the arrows do.
            qnc_source_timeline::DRAG_OUT if qnc_sync_session::dragged(preview, segments, event.value) => Some(Some(true)),
            qnc_source_timeline::DRAG_IN | qnc_source_timeline::DRAG_OUT | qnc_source_timeline::CHOOSE_A1_CHANNEL | qnc_source_timeline::CHOOSE_A2_CHANNEL => {
                self.panel = Panel::SourceTimeline;
                Some(self.source.route_input(id, event.value, preview, timeline))
            }
            // Scissors (user 2026-10-18): the source timeline keeps the pair of cuts; a cut
            // marks IN, so it arms Sync/B-roll as I does.
            qnc_source_timeline::CUT => {
                self.panel = Panel::SourceTimeline;
                qnc_sync_session::armed_by_in(segments, preview);
                Some(self.source.route_input(id, None, preview, timeline))
            }
            _ => {
                self.panel = Panel::SourceTimeline;
                None
            }
        }
    }

    fn route_on_panel(
        &mut self,
        action_id: &str,
        (preview, wrap, segments, timeline): (
            &mut SourcePreview,
            &mut WrapSession,
            &mut ProgramSegments,
            &mut TimelineProjection,
        ),
    ) -> Option<bool> {
        let panel = self.panel;
        // The dock buttons are part of the source timeline panel (user 2026-10-13): its keys work there too.
        let on_source = matches!(panel, Panel::SourceTimeline | Panel::SourceButtons);
        let on_segments = panel == Panel::Segments;
        preview.hands_on(); // every key of the montage holds the background work off
        // The order below is the priority of the blocks: the first that takes a key answers it
        // (the module keeps only this order; each block does its own work).
        // 1. Whatever the focus: the monitor switch, a click that keeps a source mark, the wheel.
        match action_id {
            // Monitor Auto | On under the monitor: whatever the focus.
            qnc_source_preview::MONITOR_AUTO | qnc_source_preview::MONITOR_ON => return Some(preview.set_monitor_mode(action_id)),
            // A left click on the source timeline while IN/OUT is drawn taken keeps it (piece
            // `qnc-source-timeline`): never Enter, which walks the panels.
            qnc_source_timeline::KEEP_MARK => {
                self.panel = Panel::SourceTimeline;
                return self.source.route_input(action_id, None, preview, timeline);
            }
            // The wheel over a panel (user 2026-10-16, piece `qnc-wheel-jog`): that panel takes
            // the keyboard and the wheel is its left/right arrow.
            // Ctrl+wheel is Ctrl+left/right: the level of a taken A1/A2 sound, up louder.
            wheel if wheel.starts_with("wheel_") && wheel.contains("_ctrl_") => {
                self.panel = if wheel.starts_with("wheel_source") { Panel::SourceTimeline } else { Panel::Segments };
                // On the source timeline with no lane taken, Ctrl+wheel moves the taken IN/OUT or
                // segment edge, as on the source timeline of every form (audit 2026-10-20: in
                // Story it did nothing); with nothing taken it does nothing.
                if self.panel == Panel::SourceTimeline && !self.source.lane_taken() {
                    if !(self.taken(timeline) || segments.trimming_edge()) {
                        return Some(false);
                    }
                    let forward = wheel.ends_with("back") != qnc_wheel_jog::playhead_up_forward();
                    let arrow = if forward { "step_forward_frame" } else { "step_back_frame" };
                    return self.route_on_panel(arrow, (preview, wrap, segments, timeline));
                }
                let level = if wheel.ends_with("back") { "lane_level_down" } else { "lane_level_up" };
                return self.route_on_panel(level, (preview, wrap, segments, timeline));
            }
            // The wheel alone is the left/right arrow, its direction this computer's choice;
            // where those arrows turn a taken sound off/on, the wheel moves the playhead
            // (user 2026-10-16: the level never goes straight to the wheel, the playhead does).
            wheel if wheel.starts_with("wheel_") => {
                // A segment edge (Ctrl+Shift+I/O) is taken on the source timeline: the wheel
                // moves it wherever the pointer is (live 2026-10-16: over the segment panel the
                // wheel did nothing).
                self.panel = if wheel.starts_with("wheel_source") || segments.trimming_edge() { Panel::SourceTimeline } else { Panel::Segments };
                let forward = wheel.ends_with("back") != qnc_wheel_jog::playhead_up_forward();
                let frames = if forward { 1 } else { -1 };
                if self.panel == Panel::Segments && segments.part_sound_taken() {
                    return Some(wrap.step(frames));
                }
                if self.panel == Panel::SourceTimeline && self.source.lane_taken() {
                    return self.source.route(wheel, preview, timeline); // the playhead moves
                }
                let arrow = if forward { "step_forward_frame" } else { "step_back_frame" };
                return self.route_on_panel(arrow, (preview, wrap, segments, timeline));
            }
            _ => {}
        }
        // 2. The segment panel: the sound of the stored segment (A1) or cover (A2) under the
        // Wrap playhead (piece `qnc-segments-keys`).
        if on_segments {
            if let ControlFlow::Break(answer) = qnc_segments_keys::overlap(action_id, (preview, wrap, segments)) {
                return answer;
            }
            if let ControlFlow::Break(answer) = qnc_segments_keys::sound(action_id, (preview, wrap, segments)) {
                return answer;
            }
        }
        // 3. A1/A2 of the source timeline are its own (piece `qnc-source-timeline`): Ctrl+1,
        // Ctrl+2 take a lane there; while it is taken its keys belong to it.
        match action_id {
            "select_audio_a1" | "select_audio_a2" => {
                self.panel = Panel::SourceTimeline; // the lane is on the source timeline
                return self.source.route(action_id, preview, timeline);
            }
            lane if on_source && self.source.lane_taken() && LANE_KEYS.contains(&lane) => {
                return self.source.route(lane, preview, timeline);
            }
            _ => {}
        }
        // 4. Sync/B-roll: Space, and a stopped Sync's arrows, Enter, Delete, O (piece `qnc-sync-session`).
        if let ControlFlow::Break(answer) = qnc_sync_session::route(action_id, (preview, wrap, segments), panel != Panel::Segments) {
            return answer;
        }
        // 5. The pool tabs, the media pool and the source dock buttons (pieces `qnc-key-row`,
        // `qnc-key-list`).
        if let Some(answer) = self.rows_and_pool(action_id, (preview, wrap, segments, timeline)) {
            return answer;
        }
        // 6. The source timeline and the segment panel.
        match action_id {
            // Up/Down on a taken edit (user 2026-10-11): to the key picture before or after it,
            // half a second when every picture is one (intra). The answer comes in `poll`.
            "step_prev_part" | "step_next_part"
                if on_source && (self.taken(timeline) || (segments.trimming_edge() && qnc_source_mark_focus::is_taken(timeline))) =>
            {
                let (clip, frame) = (preview.view().clip_id.clone(), qnc_source_mark_focus::taken_picture(timeline));
                self.jump = Some(Jump::SourceMark);
                let forward = action_id == "step_next_part";
                return Some(clip.zip(frame).is_some_and(|(clip, frame)| self.keyframes.request(&clip, frame, forward, preview.still_locator())));
            }
            "step_prev_part" | "step_next_part" if on_segments && segments.keyframe_reference().is_some() => {
                let (clip, frame) = segments.keyframe_reference().unwrap_or_default();
                self.jump = Some(Jump::Program);
                return Some(self.keyframes.request(&clip, frame, action_id == "step_next_part", preview.still_locator()));
            }
            // Enter is the confirmation of the panel in focus only.
            "activate_focused_item" if on_source => {
                let confirmed = segments.confirm_source();
                // A taken mark is let go; with nothing to confirm Enter goes on round the walk.
                let released = self.taken(timeline) && self.source.route("clear_focus", preview, timeline).unwrap_or(false);
                let back = self.back_to_wrap(segments, wrap, timeline);
                return Some(confirmed || released || back || self.advance(preview, wrap, segments));
            }
            "activate_focused_item" if on_segments => {
                return Some(qnc_segments_keys::confirm(preview, segments) || self.advance(preview, wrap, segments));
            }
            "activate_focused_item" => return Some(false),
            // A click on a Wrap row keeps what is taken there; nothing taken, nothing happens.
            "segments_keep_edit" => return Some(qnc_segments_keys::confirm(preview, segments)),
            // A virtual shot (Shift+S or Add SubClip), not by an Enter that confirmed.
            "save_virtual_shot" if segments.source_waits_enter() || self.source.holds_enter() => return Some(false),
            // Down closes an open large wave off the segment panel (there down is the next segment).
            "step_next_part" if on_source && self.source.view().wave_over_video.is_some() => {
                return self.source.route(action_id, preview, timeline);
            }
            // Home on the source timeline: the start of the clip (the V label does the same).
            "playlist_input_start" if on_source => return Some(preview.cue(timeline.range_start_frame)),
            "focus_next" | "focus_prev" => {
                let next = cycle(panel, action_id == "focus_next", &self.walk());
                return Some(self.enter(next, preview, wrap, segments));
            }
            _ => {}
        }
        // I/O, Ctrl+I/O and Ctrl+Shift+I/O of the segment panel (piece `qnc-segments-keys`).
        if on_segments {
            if let ControlFlow::Break(answer) = qnc_segments_keys::marks(action_id, segments, &mut self.source) {
                return answer;
            }
        }
        Some(match action_id {
            // The source timeline marks IN/OUT, as in every form (one owner, piece
            // `qnc-source-panel`; the stills follow the monitor, `qnc-virtual-short-stills`). I/O mark
            // the chosen clip from the pool too (user 2026-10-16).
            "mark_in" | "mark_out" => {
                qnc_source_mark_focus::release(timeline); // a new mark lets go of a taken one
                if action_id == "mark_in" {
                    qnc_sync_session::armed_by_in(segments, preview); // v5: every IN arms Sync/B-roll
                }
                return self.source.route(action_id, preview, timeline);
            }
            // v5 navigate_adjacent_source_object: start, IN and OUT in order on the source.
            "navigate_prev_object" | "navigate_next_object" if on_source => return self.source.route(action_id, preview, timeline),
            // v5 mark_in_fit_duration: IN at the playhead, OUT the length of the slot.
            "mark_in_fit_duration" if on_source => match segments.fit_slot() {
                Some(frames) => {
                    self.source.set_mark_taken(false);
                    mark_cue(preview, qnc_source_mark_focus::fit(timeline, frames))
                }
                None => false,
            },
            // Escape drops a taken segment edge at once and goes back to Wrap.
            "clear_focus" if segments.trimming_edge() => {
                segments.apply_action(action_id);
                self.back_to_wrap(segments, wrap, timeline)
            }
            // v5 select_mark_in / select_mark_out: Ctrl+I/O take IN/OUT of the chosen clip from
            // the pool too (as I/O mark it there, user 2026-10-17); the keyboard goes to the
            // source timeline, where the arrows and the wheel move the taken mark.
            "select_mark_in" | "select_mark_out" => {
                self.panel = Panel::SourceTimeline;
                return self.source.route(action_id, preview, timeline);
            }
            "step_back_frame" | "step_forward_frame" if segments.trimming_edge() && qnc_source_mark_focus::is_taken(timeline) => {
                self.panel = Panel::SourceTimeline; // the edge is on the source timeline
                self.source.set_mark_taken(true); // a segment edge opened with its mark taken
                return self.source.route(action_id, preview, timeline);
            }
            "clear_focus" if self.taken(timeline) => return self.source.route(action_id, preview, timeline),
            // Escape gives up what is not saved (user 2026-10-21): after a taken mark, a Sync draft.
            "clear_focus" if segments.sync_drafting() => segments.discard_sync(),
            // Escape drops the edit of a segment wherever the keyboard is.
            "clear_focus" if on_source && segments.view().editing.is_some() => {
                segments.apply_action(action_id)
            }
            "step_back_frame" | "step_forward_frame" => match panel {
                Panel::Segments => qnc_segments_keys::step(action_id, wrap, segments),
                // The pool has no arrow keys: they step the source playhead (user rule
                // 2026-10-01: the arrows move the playhead one frame, no other key needed).
                Panel::SourceTimeline => return self.source.route(action_id, preview, timeline),
                Panel::Pool | Panel::PoolTabs | Panel::SourceButtons => false, // the keys act on the panel in focus only
            },
            action if SOURCE_TO_PROGRAM.contains(&action) => segments.apply_action(action),
            action if segments.handles(action) => on_segments && segments.apply_action(action),
            _ => return None,
        })
    }

    /// The pool tabs, the media pool and the source dock buttons: `Some(answer)` when the
    /// key is theirs.
    fn rows_and_pool(
        &mut self,
        action_id: &str,
        (preview, wrap, segments, timeline): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection),
    ) -> Option<Option<bool>> {
        let panel = self.panel;
        let arrow = matches!(action_id, "step_back_frame" | "step_forward_frame");
        let back = action_id == "step_back_frame";
        // A pool tab, by click or Alt+letter, puts the keyboard on the pool tabs.
        if self.walk.select_in(qnc_focus_walk::POOL_TABS, action_id) {
            self.panel = Panel::PoolTabs;
            return Some(None);
        }
        Some(match (panel, action_id) {
            // The arrows on the pool tabs choose the tab to the left or right.
            (Panel::PoolTabs, _) if arrow => {
                self.pick = self.walk.choose_in(qnc_focus_walk::POOL_TABS, back);
                return Some(None);
            }
            // The arrows on the dock buttons move between them, Enter presses one.
            (Panel::SourceButtons, _) if arrow => {
                self.walk.choose_in(qnc_focus_walk::SOURCE_BUTTONS, back);
                Some(true)
            }
            (Panel::SourceButtons, "activate_focused_item") => {
                let Some(id) = self.walk.chosen_in(qnc_focus_walk::SOURCE_BUTTONS).map(str::to_string) else { return Some(Some(false)) };
                if !segments.view().action_enabled(&id) {
                    return Some(Some(false));
                }
                // The button acts as its catalog action from the source timeline it heads.
                self.panel = Panel::SourceTimeline;
                let routed = self.route(&id, (preview, wrap, segments, timeline));
                if self.panel == Panel::SourceTimeline {
                    self.panel = Panel::SourceButtons;
                }
                if routed.is_none() {
                    self.pick = Some(id);
                }
                self.advance(preview, wrap, segments); // Enter goes on round the walk (user 2026-10-13)
                routed.map(|_| true)
            }
            // Enter on the pool tabs goes on to the pool.
            (Panel::PoolTabs, "activate_focused_item") => Some(self.advance(preview, wrap, segments)),
            // The media pool: the arrows move the card focus, Enter opens the card (and the
            // clip chosen puts the keyboard on its source timeline). The tick key is Ingest's.
            (Panel::Pool, key) if qnc_key_list::step_of(key).is_some_and(|step| step != PoolStep::Toggle) => {
                self.walk.press_list(qnc_focus_walk::CARDS, qnc_key_list::step_of(key).unwrap_or(PoolStep::Open), true);
                Some(true)
            }
            (Panel::PoolTabs | Panel::SourceButtons, "step_prev_part" | "step_next_part") => Some(false),
            _ => return None,
        })
    }

    /// The panels Tab and Enter reach: those of the layout that have something to take.
    fn walk(&self) -> Vec<Panel> {
        self.walk.reachable().into_iter().filter_map(Panel::from_name).collect()
    }

    /// Enter with nothing to confirm: on to the next panel of the walk.
    fn advance(&mut self, preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &ProgramSegments) -> bool {
        let next = cycle(self.panel, true, &self.walk());
        self.enter(next, preview, wrap, segments)
    }

    /// A segment edge written or dropped: the mark is let go and the keyboard and
    /// the Wrap playhead go back to the segment panel.
    fn back_to_wrap(&mut self, segments: &mut ProgramSegments, wrap: &mut WrapSession, timeline: &mut TimelineProjection) -> bool {
        let Some(frame) = segments.take_wrap_return() else {
            return false;
        };
        self.source.set_mark_taken(false);
        qnc_source_mark_focus::release(timeline);
        self.panel = Panel::Segments;
        wrap.seek(frame);
        true
    }

    /// Tab into a panel: the source timeline shows the source clip again (v5
    /// `activate_source_keyboard_panel`), the segment panel opens the program at
    /// the Wrap playhead.
    fn enter(&mut self, panel: Panel, preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &ProgramSegments) -> bool {
        self.panel = panel;
        match panel {
            Panel::SourceTimeline if wrap.is_active() => {
                wrap.leave();
                let frame = preview.view().timeline.playhead_frame.unwrap_or(0);
                preview.timeline_intent(&TimelineIntent::CueFrame(frame));
            }
            Panel::Segments if !wrap.is_active() && !segments.view().is_empty() => wrap.seek(wrap.playhead()),
            _ => {}
        }
        true
    }
}

/// The passive mark of the focused panel: a thin frame on top of it (user
/// decision 2026-09-25). It paints only; it takes no input.
/// The keys a taken A1/A2 lane of the source timeline takes.
const LANE_KEYS: &[&str] = &[
    "step_back_frame",
    "step_forward_frame",
    "step_prev_part",
    "step_next_part",
    "lane_level_down",
    "lane_level_up",
    "activate_focused_item",
    "clear_focus",
];

pub use qnc_ui_kit::paint_focus;

/// The next or previous panel of the walk, round; the first one from outside it.
pub fn cycle(panel: Panel, forward: bool, order: &[Panel]) -> Panel {
    let Some(index) = order.iter().position(|p| *p == panel) else {
        return order.first().copied().unwrap_or(panel);
    };
    let next = if forward {
        (index + 1) % order.len()
    } else {
        (index + order.len() - 1) % order.len()
    };
    order[next]
}

/// The panel name of the segment panel, as its block gives it with a pointer event.
fn qnc_segment_panel_name() -> &'static str {
    "segments"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pointer_event_belongs_to_the_panel_it_was_on() {
        let mut focus = PanelFocus::new();
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        let mut timeline = TimelineProjection { duration_frames: 100, playhead_frame: Some(0), ..Default::default() };
        focus.set(Panel::Segments);
        // A right click on the source timeline is Ctrl+I there, not the slot edge of the segment panel in focus.
        focus.route(ActionEvent::key("select_mark_in").at(qnc_source_timeline::PANEL), (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.panel(), Panel::SourceTimeline);
        focus.route(ActionEvent::key("timeline_cue_frame").with_value(5u64).at("segments"), (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.panel(), Panel::Segments, "a click on a Wrap row focuses the segment panel");
        focus.route(ActionEvent::key("timeline_cue_frame").with_value(9u64).at(qnc_source_timeline::PANEL), (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.panel(), Panel::SourceTimeline, "and one on the source timeline the source timeline");
    }

    #[test]
    fn tab_walks_the_panels_of_the_form_and_keys_on_tabs_and_buttons_become_their_actions() {
        let layout = FocusLayout::new((
            vec!["pool_tabs", "pool", "source_timeline", "source_buttons", "segments"],
            vec!["editorial_tab_all", "editorial_tab_virtual"],
            vec!["save_virtual_shot", "add_ton_segment"],
        ));
        let mut focus = PanelFocus::with_layout(layout);
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        let mut timeline = TimelineProjection::default();
        assert_eq!(focus.panel(), Panel::PoolTabs, "the walk starts on its first panel");
        for expected in [Panel::Pool, Panel::SourceTimeline, Panel::SourceButtons, Panel::Segments, Panel::PoolTabs] {
            focus.route("focus_next", (&mut preview, &mut wrap, &mut segments, &mut timeline));
            assert_eq!(focus.panel(), expected);
        }
        // Enter goes on round the walk: the pool tabs to the pool, where it opens a card.
        focus.route("activate_focused_item", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.panel(), Panel::Pool);
        focus.route("step_next_part", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline)).pool_step, Some((PoolStep::Down, 1)));
        focus.route("step_prev_segment", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline)).pool_step, Some((PoolStep::Down, 1)), "the second action of the same arrow keeps it");
        focus.set(Panel::SourceTimeline);
        focus.route("activate_focused_item", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.panel(), Panel::SourceButtons, "nothing to confirm on the source: on to its buttons");
        assert_eq!(focus.route("mark_in", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(true), "I still marks: the buttons are part of the source timeline panel");
        focus.set(Panel::PoolTabs);
        assert_eq!(focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline)), None);
        assert_eq!(focus.take_pick().as_deref(), Some("editorial_tab_virtual"), "the right arrow is the next tab");
        focus.set(Panel::SourceButtons);
        focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline)).button.as_deref(), Some("add_ton_segment"));
        assert_eq!(cycle(Panel::SourceTimeline, false, &[Panel::Pool, Panel::SourceTimeline]), Panel::Pool);
    }

    /// Locked (user rule 2026-10-20): Ctrl+wheel is Ctrl+arrows: a taken IN moves with it.
    #[test]
    fn ctrl_wheel_moves_a_taken_in_as_on_every_source_timeline() {
        let mut focus = PanelFocus::new();
        let mut preview = SourcePreview::default();
        let mut wrap = WrapSession::new();
        let mut segments = ProgramSegments::new();
        let mut timeline = TimelineProjection { duration_frames: 100, playhead_frame: Some(20), source_in_frame: Some(10), source_out_frame: Some(50), ..Default::default() };
        focus.set(Panel::SourceTimeline);
        assert_eq!(focus.route("wheel_source_ctrl_forward", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(false), "nothing taken: nothing moves");
        focus.route("select_mark_in", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        focus.route("wheel_source_ctrl_forward", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_ne!(timeline.source_in_frame, Some(10), "the taken IN moved a frame");
    }

    #[test]
    fn the_source_timeline_takes_i_o_and_the_segments_take_markers() {
        let mut focus = PanelFocus::new();
        let mut preview = SourcePreview::default();
        let mut wrap = WrapSession::new();
        let mut segments = ProgramSegments::new();
        focus.set(Panel::Segments);
        assert_eq!(
            focus.route("mark_in", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
            Some(false),
            "I does nothing on the segment panel"
        );
        focus.set(Panel::SourceTimeline);
        assert_eq!(
            focus.route("mark_in", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
            Some(true),
            "the source timeline marks IN (one owner)"
        );
        focus.set(Panel::Pool);
        assert_eq!(
            focus.route("mark_out", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
            Some(true),
            "a clip chosen in the pool is marked from the pool"
        );
        assert_eq!(
            focus.route("add_marker", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
            Some(false),
            "M belongs to the segment panel"
        );
        assert_eq!(
            focus.route(
                "editorial_tab_all",
                (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())
            ),
            None
        );
    }

    #[test]
    fn keys_act_on_the_panel_in_focus_only() {
        let mut focus = PanelFocus::new();
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        let mut timeline = TimelineProjection::default();
        focus.set(Panel::Pool);
        assert_eq!(focus.route("add_marker", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(false), "M is the segment panel's");
    }

    #[test]
    fn ctrl_2_takes_a2_on_the_source_timeline_from_any_panel() {
        let mut focus = PanelFocus::new();
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        let mut timeline = TimelineProjection::default();
        focus.set(Panel::Pool);
        // A clip without a known channel count: the lane is not taken, the keyboard is on the source timeline.
        assert_eq!(focus.route("select_audio_a2", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(false));
        assert_eq!(focus.panel(), Panel::SourceTimeline);
        assert!(!focus.source_timeline().lane_taken());
        assert!(!focus.source_timeline().view().message.is_empty(), "it says why");
    }

    #[test]
    fn the_arrows_move_the_playhead_until_ctrl_i_or_ctrl_o_takes_a_mark() {
        let mut focus = PanelFocus::new();
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        focus.set(Panel::SourceTimeline);
        // I marked IN: the timeline paints its focus on IN, the arrows stay the playhead's.
        let mut timeline = TimelineProjection { duration_frames: 100, playhead_frame: Some(0), source_in_frame: Some(10), ..Default::default() }.focus_source_in();
        focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(timeline.source_in_frame, Some(10), "the arrow stepped the playhead");
        focus.route("select_mark_in", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(timeline.source_in_frame, Some(11), "select_mark_in took IN");
        focus.route("clear_focus", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(timeline.source_in_frame, Some(11), "Escape gave the arrows back");
    }
}

/// A taken source mark moves the source playhead with it.
fn mark_cue(preview: &mut SourcePreview, key: MarkKey) -> bool {
    match key {
        MarkKey::Cue(frame) => preview.cue(frame),
        MarkKey::Changed => true,
        MarkKey::Refused => false,
    }
}

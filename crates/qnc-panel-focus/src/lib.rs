//! Keyboard focus of an editorial surface (QNC v5 `story/focus.rs` `PanelFocus`):
//! the QNC keyboard map acts on the panel in focus, not on the whole application
//! (user rule 2026-09-25). Panels: the media pool, the source timeline (only I/O
//! and its playhead) and the segment panel (Wrap, markers, covers). Tab cycles
//! them; a click in a panel focuses it.
//!
//! This component routes one catalog `action_id` to the public pieces the focused
//! panel drives: the source preview (Broadcast Player), the Wrap view and the
//! program model. It owns no player, database or form, and knows no application.

use eframe::egui;
use qnc_program_segments::{ProgramSegments, SyncSpace};
use qnc_source_preview::{SourcePreview, TransientCover};
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
pub use qnc_media_card::GridStep as PoolStep;

/// What a form draws of the focus: the panel, on the source dock buttons the action of
/// the one the arrows are on, and the last key on the media pool with its number (the
/// card grid applies each number once, however often the view is read).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FocusView {
    pub panel: Panel,
    pub button: Option<String>,
    pub pool_step: Option<(PoolStep, u64)>,
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
    /// Ctrl+I or Ctrl+O took a source mark: the arrows move it, not the playhead
    /// (an I or O press only marks; the playhead keeps the arrows, v5).
    mark_taken: bool,
    /// Up/Down on a taken edit: the key picture search and what its answer moves.
    keyframes: qnc_keyframe_nav::KeyframeNav,
    jump: Option<Jump>,
    layout: FocusLayout,
    /// The pool tab and the source dock button the arrows are on.
    tab: usize,
    button: usize,
    /// The catalog action a key on the pool tabs or a dock button stands for; the
    /// caller runs it as its own action (`take_pick`).
    pick: Option<String>,
    /// The last key on the media pool and its number, for the card grid. It is not taken
    /// on read: one arrow fires two catalog actions (storyboard and off scope) and every
    /// action reads the view again (live 2026-10-13: Up/Down were lost).
    pool_step: Option<(PoolStep, u64)>,
    /// Enter opened the focused card: the clip it chooses puts the keyboard on its
    /// source timeline; a click on a card keeps it on the pool.
    opening: bool,
}

impl PanelFocus {
    pub fn new() -> Self {
        Self::default()
    }

    /// The focus of a form with its Tab walk; it starts on the first panel of the walk.
    pub fn with_layout(layout: FocusLayout) -> Self {
        let panel = layout.order.first().copied().unwrap_or_default();
        Self { panel, layout, ..Self::default() }
    }

    /// The action a key on the pool tabs or a dock button stood for, once.
    pub fn take_pick(&mut self) -> Option<String> {
        self.pick.take()
    }

    /// A mark taken with Ctrl+I/O that the source timeline still has in focus.
    fn taken(&mut self, timeline: &TimelineProjection) -> bool {
        self.mark_taken &= qnc_source_mark_focus::is_taken(timeline);
        self.mark_taken
    }

    pub fn panel(&self) -> Panel {
        self.panel
    }

    /// Applies the answer of an Up/Down key picture search to the edit it was asked
    /// for (the taken source mark, or the taken slot or marker); returns the panel.
    pub fn poll(
        &mut self,
        (preview, _wrap, segments, timeline): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection),
    ) -> FocusView {
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
        let button = (self.panel == Panel::SourceButtons).then(|| self.layout.buttons.get(self.button).cloned()).flatten();
        FocusView { panel: self.panel, button, pool_step: self.pool_step }
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
        self.panel = if std::mem::take(&mut self.opening) { Panel::SourceTimeline } else { Panel::Pool };
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

    /// Routes one catalog action to the focused panel. `None`: not a panel action
    /// (the application handles it); `Some(changed)`: taken here, done or ignored.
    pub fn route(
        &mut self,
        action_id: &str,
        pieces: (
            &mut SourcePreview,
            &mut WrapSession,
            &mut ProgramSegments,
            &mut TimelineProjection,
        ),
    ) -> Option<bool> {
        let panel = self.panel;
        let routed = self.route_on_panel(action_id, (&mut *pieces.0, &mut *pieces.1, &mut *pieces.2, &mut *pieces.3));
        // Every key of the story in the player log (live 2026-10-16: IN/OUT not taken
        // after some action): the action, the panel it met, what came of it and the source
        // marks and playhead as the key found them.
        if qnc_dev_diagnostics::player_diagnostics_enabled() {
            let (preview, timeline) = (&*pieces.0, &*pieces.3);
            qnc_dev_diagnostics::log_line(
                qnc_dev_diagnostics::DiagnosticsStream::Player,
                format!(
                    "story-key action={action_id} panel={panel:?}->{:?} routed={routed:?} in={:?} out={:?} playhead={:?} program={} part_sound={}",
                    self.panel,
                    timeline.source_in_frame,
                    timeline.source_out_frame,
                    timeline.playhead_frame,
                    preview.in_program(),
                    pieces.2.part_sound_taken(),
                ),
            );
        }
        routed
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
        preview.hands_on(); // every key of the montage holds the background work off
        Some(match action_id {
            // Monitor Auto | On under the monitor: whatever the focus.
            qnc_source_preview::MONITOR_AUTO | qnc_source_preview::MONITOR_ON => preview.set_monitor_mode(action_id),
            // A1/A2 by keyboard (user rule 2026-10-01): Ctrl+1, Ctrl+2 take the lane; while it is
            // taken, its height is expanded over the video row and up/down (or a click) pick its
            // channel in the column (user 2026-10-14), left turns it off, right on, Ctrl+left/right
            // set its level (user 2026-10-15); all heard at once, Enter keeps, Escape lets go.
            // The wheel over a panel (user 2026-10-16, piece `qnc-wheel-jog`): that panel takes
            // the keyboard and the wheel is its left/right arrow.
            // Ctrl+wheel is Ctrl+left/right: the level of a taken A1/A2 sound, up louder.
            wheel if wheel.starts_with("wheel_") && wheel.contains("_ctrl_") => {
                self.panel = if wheel.starts_with("wheel_source") { Panel::SourceTimeline } else { Panel::Segments };
                let level = if wheel.ends_with("back") { "lane_level_down" } else { "lane_level_up" };
                return self.route_on_panel(level, (preview, wrap, segments, timeline));
            }
            // The wheel alone is the left/right arrow, its direction this computer's choice;
            // where those arrows turn a taken sound off/on, the wheel moves the playhead
            // (user 2026-10-16: the level never goes straight to the wheel, the playhead does).
            wheel if wheel.starts_with("wheel_") => {
                self.panel = if wheel.starts_with("wheel_source") { Panel::SourceTimeline } else { Panel::Segments };
                let forward = wheel.ends_with("back") != qnc_wheel_jog::playhead_up_forward();
                let frames = if forward { 1 } else { -1 };
                if self.panel == Panel::Segments && segments.part_sound_taken() {
                    return Some(wrap.step(frames));
                }
                if self.panel == Panel::SourceTimeline && segments.lane_is_taken() {
                    return Some(preview.step(frames));
                }
                let arrow = if forward { "step_forward_frame" } else { "step_back_frame" };
                return self.route_on_panel(arrow, (preview, wrap, segments, timeline));
            }
            // On the segment panel the same keys take the sound of the stored segment (A1) or
            // cover (A2) under the Wrap playhead (user 2026-10-16); Enter writes it.
            // The draft is heard in the program at once (user 2026-10-16), before Enter.
            "select_audio_a1" | "select_audio_a2" if panel == Panel::Segments => {
                segments.take_part_sound(u8::from(action_id == "select_audio_a2"), wrap.playhead())
            }
            "step_prev_part" | "step_next_part" if panel == Panel::Segments && segments.part_sound_taken() => {
                let changed = segments.part_sound_draft(if action_id == "step_prev_part" { -1 } else { 1 });
                preview.hear_program_draft(sound_draft(segments), stored_sound(segments), wrap.playhead());
                changed
            }
            "step_back_frame" | "step_forward_frame" if panel == Panel::Segments && segments.part_sound_taken() => {
                let changed = segments.part_sound_off(action_id == "step_back_frame");
                preview.hear_program_draft(sound_draft(segments), stored_sound(segments), wrap.playhead());
                changed
            }
            "lane_level_down" | "lane_level_up" if panel == Panel::Segments && segments.part_sound_taken() => {
                let changed = segments.part_sound_level(if action_id == "lane_level_down" { -1 } else { 1 });
                preview.hear_program_draft(sound_draft(segments), stored_sound(segments), wrap.playhead());
                changed
            }
            "clear_focus" | "close_player" if panel == Panel::Segments && segments.part_sound_taken() => {
                let released = segments.release_part_sound();
                preview.hear_program_draft(None, None, wrap.playhead()); // the stored sound again
                released
            }
            "select_audio_a1" | "select_audio_a2" => {
                self.panel = Panel::SourceTimeline; // the lane is on the source timeline
                segments.take_lane(u8::from(action_id == "select_audio_a2"))
            }
            "step_back_frame" | "step_forward_frame" if on_source && segments.lane_is_taken() => {
                let changed = segments.lane_off(action_id == "step_back_frame");
                preview.hear_channels(segments.audible_channels());
                changed
            }
            "lane_level_down" | "lane_level_up" if on_source && segments.lane_is_taken() => {
                let changed = segments.lane_level(if action_id == "lane_level_down" { -1 } else { 1 });
                preview.hear_channels(segments.audible_channels());
                changed
            }
            // A Sync (user 2026-10-13): stopped, the arrows move its end; Enter writes the
            // cover, Delete drops the draft, wherever the keyboard is.
            "step_back_frame" | "step_forward_frame" if segments.sync_drafting() && !preview.player_view().playing() => {
                let moved = segments.sync_nudge(if action_id == "step_back_frame" { -1 } else { 1 });
                // The monitor shows the cover's last picture at the new end (user 2026-10-16).
                if let Some(frame) = segments.sync_window_frame().filter(|_| moved) {
                    preview.show_program_frame(frame, false);
                }
                moved
            }
            "activate_focused_item" if segments.sync_drafting() => segments.confirm_source(),
            "delete_marker" if segments.sync_drafting() => segments.discard_sync(),
            // A pool tab, by click or Alt+letter, puts the keyboard on the pool tabs.
            tab if self.layout.tabs.iter().any(|id| id == tab) => {
                self.tab = self.layout.tabs.iter().position(|id| id == tab).unwrap_or(0);
                self.panel = Panel::PoolTabs;
                return None;
            }
            // The arrows on the pool tabs choose the tab to the left or right.
            "step_back_frame" | "step_forward_frame" if panel == Panel::PoolTabs => {
                let last = self.layout.tabs.len().saturating_sub(1);
                self.tab = if action_id == "step_back_frame" { self.tab.saturating_sub(1) } else { (self.tab + 1).min(last) };
                self.pick = self.layout.tabs.get(self.tab).cloned();
                return None;
            }
            // The arrows on the dock buttons move between them, Enter presses one.
            "step_back_frame" | "step_forward_frame" if panel == Panel::SourceButtons => {
                let last = self.layout.buttons.len().saturating_sub(1);
                self.button = if action_id == "step_back_frame" { self.button.saturating_sub(1) } else { (self.button + 1).min(last) };
                true
            }
            "activate_focused_item" if panel == Panel::SourceButtons => {
                let Some(id) = self.layout.buttons.get(self.button).cloned() else { return Some(false) };
                if !segments.view().action_enabled(&id) {
                    return Some(false);
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
                return routed.map(|_| true);
            }
            // Enter on the pool tabs goes on to the pool.
            "activate_focused_item" if panel == Panel::PoolTabs => self.advance(preview, wrap, segments),
            // The media pool: the arrows move the card focus, Enter opens the card (and the
            // clip chosen puts the keyboard on its source timeline).
            "step_back_frame" | "step_forward_frame" | "step_prev_part" | "step_next_part" | "page_prev" | "page_next" | "activate_focused_item" if panel == Panel::Pool => {
                let number = self.pool_step.map_or(1, |(_, number)| number + 1);
                self.pool_step = Some((match action_id {
                    "step_back_frame" => PoolStep::Left,
                    "step_forward_frame" => PoolStep::Right,
                    "step_prev_part" => PoolStep::Up,
                    "step_next_part" => PoolStep::Down,
                    "page_prev" => PoolStep::PageUp,
                    "page_next" => PoolStep::PageDown,
                    _ => {
                        self.opening = true;
                        PoolStep::Open
                    }
                }, number));
                true
            }
            "step_prev_part" | "step_next_part" if matches!(panel, Panel::PoolTabs | Panel::SourceButtons) => false,
            // A taken lane (user 2026-10-14): its channels stand in a column, up/down pick.
            // The draft is heard at once (user 2026-10-14); Enter only keeps it.
            "step_prev_part" | "step_next_part" if on_source && segments.lane_is_taken() => {
                let moved = segments.lane_draft(if action_id == "step_prev_part" { -1 } else { 1 });
                preview.hear_channels(segments.audible_channels());
                moved
            }
            // Up/Down on a taken edit (user 2026-10-11): to the key picture before or after it,
            // half a second when every picture is one (intra). The answer comes in `poll`.
            "step_prev_part" | "step_next_part"
                if on_source && (self.taken(timeline) || (segments.trimming_edge() && qnc_source_mark_focus::is_taken(timeline))) =>
            {
                let (clip, frame) = (preview.view().clip_id.clone(), qnc_source_mark_focus::taken_picture(timeline));
                self.jump = Some(Jump::SourceMark);
                let forward = action_id == "step_next_part";
                clip.zip(frame).is_some_and(|(clip, frame)| self.keyframes.request(&clip, frame, forward, preview.still_locator()))
            }
            "step_prev_part" | "step_next_part" if panel == Panel::Segments && segments.keyframe_reference().is_some() => {
                let (clip, frame) = segments.keyframe_reference().unwrap_or_default();
                self.jump = Some(Jump::Program);
                self.keyframes.request(&clip, frame, action_id == "step_next_part", preview.still_locator())
            }
            // Enter is the confirmation of the panel in focus only.
            "activate_focused_item" if on_source => {
                let lane = segments.lane_is_taken();
                let confirmed = segments.confirm_source();
                if lane {
                    preview.hear_channels(segments.audible_channels());
                }
                // A taken mark is let go; with nothing to confirm Enter goes on round the walk.
                let released = self.taken(timeline) && mark_cue(preview, qnc_source_mark_focus::release(timeline));
                self.mark_taken &= !released;
                let back = self.back_to_wrap(segments, wrap, timeline);
                confirmed || released || back || self.advance(preview, wrap, segments)
            }
            "activate_focused_item" if panel == Panel::Segments => {
                let sound = segments.part_sound_taken();
                let confirmed = segments.confirm_program();
                if sound && confirmed {
                    preview.forget_program_draft(); // written: the program opens from the story
                }
                confirmed || self.advance(preview, wrap, segments)
            }
            "activate_focused_item" => false,
            // A virtual shot (Shift+S or Add SubClip), not by an Enter that confirmed.
            "save_virtual_shot" if segments.source_waits_enter() => false,
            "clear_focus" | "close_player" if on_source && segments.lane_is_taken() => {
                let released = segments.release_lane();
                preview.hear_channels(segments.audible_channels()); // back to the kept channel
                released
            }
            // Down closes an open large wave off the segment panel (there down is the next segment).
            "step_next_part" if on_source && segments.view().wave_zoom != [0, 0] => segments.close_wave(),
            // Home on the source timeline: the start of the clip (the V label does the same).
            "playlist_input_start" if on_source => preview.cue(timeline.range_start_frame),
            "focus_next" | "focus_prev" => {
                let next = cycle(panel, action_id == "focus_next", &self.walk());
                self.enter(next, preview, wrap, segments)
            }
            "play_pause" => match segments.sync_space(wrap.is_active()) {
                SyncSpace::Start(sync) => {
                    wrap.hold(sync.window.0);
                    let a2 = segments.kept_lane(1);
                    let cover = TransientCover {
                        clip_id: sync.clip_id,
                        source_in: sync.source_in,
                        timebase: sync.timebase,
                        a2_source_channel: a2.channel,
                        a2_level_db: a2.level_db,
                        a2_off: a2.off,
                    };
                    preview.open_program_window(sync.window, cover)
                }
                SyncSpace::Blocked => true,
                SyncSpace::Play => preview.toggle_play(),
            },
            // O closes a running Sync slot wherever the keyboard is (v5).
            "mark_out" if on_source && segments.finish_sync() => true,
            // The application marks IN/OUT. I/O mark the chosen clip from the pool too (user 2026-10-16: a clip chosen in
            // the pool and played is marked without going to the source timeline); on the
            // segment panel they do nothing (its edges are Ctrl+I/O).
            "mark_in" | "mark_out" if panel == Panel::Segments => false,
            "mark_in" => {
                self.mark_taken = false;
                segments.arm_sync(); // v5: every IN arms Sync/B-roll
                return None;
            }
            "mark_out" => {
                self.mark_taken = false;
                return None;
            }
            // v5 navigate_adjacent_source_object: start, IN and OUT in order on the source.
            "navigate_prev_object" | "navigate_next_object" if on_source => {
                let key = qnc_source_mark_focus::adjacent(timeline, action_id == "navigate_prev_object");
                self.mark_taken = qnc_source_mark_focus::is_taken(timeline);
                mark_cue(preview, key)
            }
            // v5 mark_in_fit_duration: IN at the playhead, OUT the length of the slot.
            "mark_in_fit_duration" if on_source => match segments.fit_slot() {
                Some(frames) => {
                    self.mark_taken = false;
                    mark_cue(preview, qnc_source_mark_focus::fit(timeline, frames))
                }
                None => false,
            },
            // v5 select_mark_in / select_mark_out: Ctrl+I, Ctrl+O take the source IN, OUT; the
            // arrows move the taken mark, Escape gives the keys back to the playhead.
            // On the segment panel Ctrl+I, Ctrl+O take the start, the end of the slot under
            // the Wrap playhead (user 2026-10-13: slots are the montage); Ctrl+Shift+I, O
            // the start, the end of the segment, whose clip opens on that edge with the
            // mark taken (user 2026-10-10).
            "select_mark_in" | "select_mark_out" if panel == Panel::Segments => {
                segments.take_slot_edge_at_playhead(action_id == "select_mark_in")
            }
            "select_segment_start" | "select_segment_end" if panel == Panel::Segments => {
                self.mark_taken = segments.take_edge(action_id == "select_segment_start");
                self.mark_taken
            }
            // Escape drops a taken segment edge at once and goes back to Wrap.
            "clear_focus" | "close_player" if on_source && segments.trimming_edge() => {
                segments.apply_action(action_id);
                self.back_to_wrap(segments, wrap, timeline)
            }
            "select_mark_in" | "select_mark_out" if on_source => {
                self.panel = Panel::SourceTimeline; // the arrows go to the taken mark, not the buttons
                let key = qnc_source_mark_focus::take(timeline, action_id == "select_mark_in");
                self.mark_taken = key != MarkKey::Refused;
                mark_cue(preview, key)
            }
            "step_back_frame" | "step_forward_frame"
                if panel == Panel::SourceTimeline
                    && (self.taken(timeline) || (segments.trimming_edge() && qnc_source_mark_focus::is_taken(timeline))) =>
            {
                let frames = if action_id == "step_back_frame" { -1 } else { 1 };
                mark_cue(preview, qnc_source_mark_focus::nudge(timeline, frames))
            }
            "clear_focus" | "close_player" if self.taken(timeline) => {
                self.mark_taken = false;
                mark_cue(preview, qnc_source_mark_focus::release(timeline))
            }
            // Escape drops the edit of a segment wherever the keyboard is.
            "clear_focus" | "close_player" if on_source && segments.view().editing.is_some() => {
                segments.apply_action(action_id)
            }
            "step_back_frame" | "step_forward_frame" => {
                let frames = if action_id == "step_back_frame" {
                    -1
                } else {
                    1
                };
                match panel {
                    Panel::Segments if segments.handles(action_id) => {
                        segments.apply_action(action_id)
                    }
                    Panel::Segments => wrap.step(frames),
                    // The pool has no arrow keys: they step the source playhead (user rule
                    // 2026-10-01: the arrows move the playhead one frame, no other key needed).
                    Panel::SourceTimeline => preview.step(frames),
                    Panel::Pool | Panel::PoolTabs | Panel::SourceButtons => false, // the keys act on the panel in focus only
                }
            }
            action if SOURCE_TO_PROGRAM.contains(&action) => segments.apply_action(action),
            action if segments.handles(action) => {
                panel == Panel::Segments && segments.apply_action(action)
            }
            _ => return None,
        })
    }

    /// The panels Tab and Enter reach: those of the layout that have something to take.
    fn walk(&self) -> Vec<Panel> {
        let layout = &self.layout;
        let walk = layout.order.iter().copied().filter(|panel| match panel {
            Panel::PoolTabs => !layout.tabs.is_empty(),
            Panel::SourceButtons => !layout.buttons.is_empty(),
            _ => true,
        });
        walk.collect()
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
        self.mark_taken = false;
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
pub fn paint_focus(ui: &egui::Ui, rect: egui::Rect, focused: bool, color: egui::Color32) {
    if focused {
        let stroke = egui::Stroke::new(1.5, color);
        ui.painter()
            .rect_stroke(rect.shrink(1.0), 0.0, stroke, egui::StrokeKind::Inside);
    }
}

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

/// The catalog actions of the wheel over the source timeline or monitor (back, forward).
/// Then with Ctrl held (the level of a taken sound).
pub const WHEEL_SOURCE: qnc_wheel_jog::WheelActions<'static> = qnc_wheel_jog::SOURCE_ACTIONS;
/// The same over the segment panel (its Wrap rows), as over the source timeline.
pub const WHEEL_SEGMENTS: qnc_wheel_jog::WheelActions<'static> =
    (("wheel_segments_back", "wheel_segments_forward"), ("wheel_segments_ctrl_back", "wheel_segments_ctrl_forward"));

/// The catalog action of a Wrap row's wheel; `None` for every other command.
pub fn wheel_action(command: &qnc_program_segments::SegmentCommand) -> Option<&'static str> {
    match command {
        qnc_program_segments::SegmentCommand::Wheel { up, ctrl } => Some(qnc_wheel_jog::Wheel { up: *up, ctrl: *ctrl }.action(WHEEL_SEGMENTS)),
        _ => None,
    }
}

/// The sound drafted on the Wrap row, as the program plays it before Enter.
fn sound_draft(segments: &ProgramSegments) -> Option<qnc_source_preview::SoundDraft> {
    let (lane, id, draft) = segments.part_sound_draft_now()?;
    Some(qnc_source_preview::SoundDraft { lane, id, channel: draft.channel, level_db: draft.level_db, off: draft.off })
}

/// The stored sound of the part taken on the Wrap row.
fn stored_sound(segments: &ProgramSegments) -> Option<qnc_source_preview::SoundDraft> {
    let (lane, id, stored) = segments.part_sound_stored()?;
    Some(qnc_source_preview::SoundDraft { lane, id, channel: stored.channel, level_db: stored.level_db, off: stored.off })
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(focus.route("mark_in", (&mut preview, &mut wrap, &mut segments, &mut timeline)), None, "I still marks: the buttons are part of the source timeline panel");
        focus.set(Panel::PoolTabs);
        assert_eq!(focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline)), None);
        assert_eq!(focus.take_pick().as_deref(), Some("editorial_tab_virtual"), "the right arrow is the next tab");
        focus.set(Panel::SourceButtons);
        focus.route("step_forward_frame", (&mut preview, &mut wrap, &mut segments, &mut timeline));
        assert_eq!(focus.poll((&mut preview, &mut wrap, &mut segments, &mut timeline)).button.as_deref(), Some("add_ton_segment"));
        assert_eq!(cycle(Panel::SourceTimeline, false, &[Panel::Pool, Panel::SourceTimeline]), Panel::Pool);
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
            None,
            "the application marks IN"
        );
        focus.set(Panel::Pool);
        assert_eq!(
            focus.route("mark_out", (&mut preview, &mut wrap, &mut segments, &mut TimelineProjection::default())),
            None,
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
    fn select_audio_a2_up_and_enter_keep_the_channel_and_close_the_picker() {
        let mut focus = PanelFocus::new();
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        segments.set_source(qnc_program_segments::SourcePick::new(Some("c"), Some("C"), None, (None, 100), Some((50, 1))), None);
        segments.set_source_channels(Some(2));
        let mut timeline = TimelineProjection::default();
        for action in ["select_audio_a2", "step_prev_part", "activate_focused_item"] {
            focus.route(action, (&mut preview, &mut wrap, &mut segments, &mut timeline));
        }
        assert_eq!(segments.view().a2_choice, Some((0, 2)), "channel 1 kept on A2");
        assert_eq!(segments.view().lane_taken, None, "Enter closed the picker");
        focus.set(Panel::Segments);
        assert_eq!(focus.route("save_virtual_shot", (&mut preview, &mut wrap, &mut segments, &mut timeline)), Some(false), "not by the Enter that kept the channel");
        std::thread::sleep(std::time::Duration::from_millis(200));
        focus.set(Panel::SourceTimeline);
        assert_eq!(focus.route("save_virtual_shot", (&mut preview, &mut wrap, &mut segments, &mut timeline)), None, "the source timeline saves a short");
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

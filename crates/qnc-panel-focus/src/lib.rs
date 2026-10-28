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

mod panel;
mod route;
mod walk;

pub use panel::*;

/// Actions of the segment panel that also run from the other panels: they use
/// the source the user marked (v5 dispatches them whatever the focus).
const SOURCE_TO_PROGRAM: [&str; 7] = [
    "add_ton_segment",
    "add_off_segment",
    "quick_overwrite_cover",
    "overwrite_cover",
    // The switch Pokrivalice | Sync/B-roll sits on the source dock (user 2026-10-08).
    "cover_mode_covers",
    "cover_mode_sync",
    // Normalising the program's loudness in the preview, from any panel (user 2026-10-24).
    "toggle_loudness_normalize",
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
    /// The transcript of the clip on the preview (piece `qnc-transcript`, Media Assist Audio AI).
    transcript: qnc_transcript::Transcript,
    /// The shots of the clip on the preview (piece `qnc-clip-shots`, Video AI).
    shots: qnc_clip_shots::ClipShots,
    /// The script of the story (piece `qnc-script`, Media Assist Audio).
    script: qnc_script::Script,
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

    /// A clip and IN/OUT a chosen statement of the transcript or a usable part of the shot
    /// analysis opens (once).
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.transcript.take_source_request().or_else(|| self.shots.take_source_request()).or_else(|| self.script.take_source_request())
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
        (preview, wrap, segments, timeline, clips): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments, &mut TimelineProjection, &mut [qnc_clip_row::ClipRow]),
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
        preview.normalize_program(segments.normalization()); // the program's A1/A2 to the project's targets
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
        self.transcript.poll(preview, timeline, clips); // the pool's ticks are the transcript's
        self.shots.poll(preview);
        self.script.poll(preview, timeline, clips);
        FocusView { panel: self.panel, button, pool_step: self.walk.last_step(), transcript: self.transcript.view().clone(), shots: self.shots.view().clone(), script: self.script.view().clone() }
    }

    /// Whether a key picture search runs (the caller repaints soon).
    pub fn busy(&self) -> bool {
        self.keyframes.busy() || self.transcript.busy() || self.script.busy()
    }

    /// Whether a transcription of this process runs in the background (the caller repaints
    /// now and then for its progress).
    pub fn working(&self) -> bool {
        self.transcript.working()
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

/// A taken source mark moves the source playhead with it.
fn mark_cue(preview: &mut SourcePreview, key: MarkKey) -> bool {
    match key {
        MarkKey::Cue(frame) => preview.cue(frame),
        MarkKey::Changed => true,
        MarkKey::Refused => false,
    }
}

#[cfg(test)]
mod tests;

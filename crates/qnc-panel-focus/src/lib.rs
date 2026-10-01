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
    Pool,
    /// v5: choosing a clip puts the keyboard on its source timeline.
    #[default]
    SourceTimeline,
    Segments,
}

/// Actions of the segment panel that also run from the other panels: they use
/// the source the user marked (v5 dispatches them whatever the focus).
const SOURCE_TO_PROGRAM: [&str; 4] = [
    "add_ton_segment",
    "add_off_segment",
    "quick_overwrite_cover",
    "overwrite_cover",
];

#[derive(Debug, Default)]
pub struct PanelFocus {
    panel: Panel,
    /// Ctrl+I or Ctrl+O took a source mark: the arrows move it, not the playhead
    /// (an I or O press only marks; the playhead keeps the arrows, v5).
    mark_taken: bool,
}

impl PanelFocus {
    pub fn new() -> Self {
        Self::default()
    }

    /// A mark taken with Ctrl+I/O that the source timeline still has in focus.
    fn taken(&mut self, timeline: &TimelineProjection) -> bool {
        self.mark_taken &= qnc_source_mark_focus::is_taken(timeline);
        self.mark_taken
    }

    pub fn panel(&self) -> Panel {
        self.panel
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
        (preview, wrap, segments, timeline): (
            &mut SourcePreview,
            &mut WrapSession,
            &mut ProgramSegments,
            &mut TimelineProjection,
        ),
    ) -> Option<bool> {
        let panel = self.panel;
        Some(match action_id {
            "focus_next" | "focus_prev" => {
                let next = cycle(
                    panel,
                    action_id == "focus_next",
                    !segments.view().is_empty(),
                );
                self.enter(next, preview, wrap)
            }
            "play_pause" => match segments.sync_space(wrap.is_active()) {
                SyncSpace::Start(sync) => {
                    wrap.hold(sync.window.0);
                    let cover = TransientCover {
                        clip_id: sync.clip_id,
                        source_in: sync.source_in,
                        timebase: sync.timebase,
                        a2_source_channel: segments.heard_channels().1,
                    };
                    preview.open_program_window(sync.window, cover)
                }
                SyncSpace::Blocked => true,
                SyncSpace::Play => preview.toggle_play(),
            },
            // O closes a running Sync slot wherever the keyboard is (v5).
            "mark_out" if segments.finish_sync() => true,
            // The source timeline takes only I/O; the application marks them.
            "mark_in" | "mark_out" if panel != Panel::SourceTimeline => false,
            "mark_in" => {
                self.mark_taken = false;
                segments.arm_sync(); // v5: every IN arms Sync/B-roll
                return None;
            }
            "mark_out" => {
                self.mark_taken = false;
                return None;
            }
            // Enter belongs to a closed Sync slot or a marker draft wherever it runs.
            "activate_focused_item" if segments.sync_holds_enter() => {
                segments.apply_action(action_id)
            }
            // A1/A2 by keyboard (user rule 2026-10-01): Ctrl+1, Ctrl+2 take the lane; while it is
            // taken, left/right pick its channel, up/down show or hide its wave over the
            // video row, Enter keeps the channel (heard at once), Escape lets go.
            "select_audio_a1" | "select_audio_a2" if panel != Panel::Segments => {
                segments.take_lane(u8::from(action_id == "select_audio_a2"))
            }
            "step_back_frame" | "step_forward_frame" if segments.lane_is_taken() => {
                segments.lane_draft(if action_id == "step_back_frame" { -1 } else { 1 })
            }
            "step_prev_part" if segments.lane_is_taken() => segments.lane_zoom(1),
            "step_next_part" if segments.lane_is_taken() => segments.lane_zoom(-1),
            "activate_focused_item" if segments.lane_is_taken() => {
                let kept = segments.commit_lane();
                preview.hear_channels(segments.heard_channels());
                kept
            }
            "clear_focus" | "close_player" if segments.lane_is_taken() => segments.release_lane(),
            // v5 navigate_adjacent_source_object: start, IN and OUT in order on the source.
            "navigate_prev_object" | "navigate_next_object" if panel == Panel::SourceTimeline => {
                let key = qnc_source_mark_focus::adjacent(timeline, action_id == "navigate_prev_object");
                self.mark_taken = qnc_source_mark_focus::is_taken(timeline);
                mark_cue(preview, key)
            }
            // v5 mark_in_fit_duration: IN at the playhead, OUT the length of the slot.
            "mark_in_fit_duration" if panel == Panel::SourceTimeline => match segments.fit_slot() {
                Some(frames) => {
                    self.mark_taken = false;
                    mark_cue(preview, qnc_source_mark_focus::fit(timeline, frames))
                }
                None => false,
            },
            // v5 select_mark_in / select_mark_out: Ctrl+I, Ctrl+O take the source IN, OUT; the
            // arrows move the taken mark, Escape gives the keys back to the playhead.
            "select_mark_in" | "select_mark_out" if panel == Panel::SourceTimeline => {
                let key = qnc_source_mark_focus::take(timeline, action_id == "select_mark_in");
                self.mark_taken = key != MarkKey::Refused;
                mark_cue(preview, key)
            }
            "step_back_frame" | "step_forward_frame"
                if panel == Panel::SourceTimeline && self.taken(timeline) =>
            {
                let frames = if action_id == "step_back_frame" { -1 } else { 1 };
                mark_cue(preview, qnc_source_mark_focus::nudge(timeline, frames))
            }
            "clear_focus" | "close_player" if self.taken(timeline) => {
                self.mark_taken = false;
                mark_cue(preview, qnc_source_mark_focus::release(timeline))
            }
            // Escape drops the edit of a segment wherever the keyboard is.
            "clear_focus" | "close_player" if segments.view().editing.is_some() => {
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
                    Panel::SourceTimeline | Panel::Pool => preview.step(frames),
                }
            }
            action if SOURCE_TO_PROGRAM.contains(&action) => segments.apply_action(action),
            action if segments.handles(action) => {
                panel == Panel::Segments && segments.apply_action(action)
            }
            _ => return None,
        })
    }

    /// Tab into a panel: the source timeline shows the source clip again (v5
    /// `activate_source_keyboard_panel`), the segment panel opens the program at
    /// the Wrap playhead.
    fn enter(&mut self, panel: Panel, preview: &mut SourcePreview, wrap: &mut WrapSession) -> bool {
        self.panel = panel;
        match panel {
            Panel::SourceTimeline if wrap.is_active() => {
                wrap.leave();
                let frame = preview.view().timeline.playhead_frame.unwrap_or(0);
                preview.timeline_intent(&TimelineIntent::CueFrame(frame));
            }
            Panel::Segments if !wrap.is_active() => wrap.seek(wrap.playhead()),
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

/// v5 `next_panel_focus`: pool, source timeline, segments (when there are any).
pub fn cycle(panel: Panel, forward: bool, has_segments: bool) -> Panel {
    let order: &[Panel] = if has_segments {
        &[Panel::Pool, Panel::SourceTimeline, Panel::Segments]
    } else {
        &[Panel::Pool, Panel::SourceTimeline]
    };
    let index = order.iter().position(|p| *p == panel).unwrap_or(0);
    let next = if forward {
        (index + 1) % order.len()
    } else {
        (index + order.len() - 1) % order.len()
    };
    order[next]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_cycles_the_panels_like_v5() {
        assert_eq!(cycle(Panel::Pool, true, true), Panel::SourceTimeline);
        assert_eq!(cycle(Panel::SourceTimeline, true, true), Panel::Segments);
        assert_eq!(cycle(Panel::Segments, true, true), Panel::Pool);
        assert_eq!(cycle(Panel::Pool, false, true), Panel::Segments);
        assert_eq!(
            cycle(Panel::SourceTimeline, true, false),
            Panel::Pool,
            "no segment panel without a story"
        );
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

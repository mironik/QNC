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
use qnc_timeline::TimelineIntent;
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
}

impl PanelFocus {
    pub fn new() -> Self {
        Self::default()
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
        (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments),
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
                segments.arm_sync(); // v5: every IN arms Sync/B-roll
                return None;
            }
            "mark_out" => return None,
            // Enter belongs to a closed Sync slot or a marker draft wherever it runs.
            "activate_focused_item" if segments.sync_holds_enter() => {
                segments.apply_action(action_id)
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
                    Panel::SourceTimeline => preview.step(frames),
                    Panel::Pool => false,
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
            focus.route("mark_in", (&mut preview, &mut wrap, &mut segments)),
            Some(false),
            "I does nothing on the segment panel"
        );
        focus.set(Panel::SourceTimeline);
        assert_eq!(
            focus.route("mark_in", (&mut preview, &mut wrap, &mut segments)),
            None,
            "the application marks IN"
        );
        assert_eq!(
            focus.route("add_marker", (&mut preview, &mut wrap, &mut segments)),
            Some(false),
            "M belongs to the segment panel"
        );
        assert_eq!(
            focus.route(
                "editorial_tab_all",
                (&mut preview, &mut wrap, &mut segments)
            ),
            None
        );
    }
}

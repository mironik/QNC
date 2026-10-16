//! The mouse wheel over a panel as catalog actions (user 2026-10-16: the wheel does what
//! the left/right arrows do, by the panel under the pointer). A mouse notch is one step;
//! a touchpad's fine scrolling is added up, a step every `POINTS_PER_STEP`. A fast spin
//! gives one action a frame and the rest in the next frames, so the player gets its steps
//! in order. Over the panel the scroll is taken, so a list under it does not move.
//! No form state, no database, no player.

use eframe::egui::{self, Rect, Ui};

pub const MODULE_ID: &str = "qnc.module.wheel-jog";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Touchpad points of one step.
pub const POINTS_PER_STEP: f32 = 40.0;

/// Steps not yet given and the touchpad points not yet a step, kept by the panel's id.
#[derive(Clone, Copy, Default)]
struct Pending {
    steps: i32,
    points: f32,
}

/// The wheel steps of one frame from its scroll events: notches (`Line`, `Page`) count
/// one each, points add up. Down (toward the user) and right are forward.
fn steps_of(events: &[egui::Event], points: &mut f32) -> i32 {
    let mut steps = 0;
    for event in events {
        let egui::Event::MouseWheel { unit, delta, .. } = event else { continue };
        let amount = -delta.y + delta.x;
        match unit {
            egui::MouseWheelUnit::Point => *points += amount,
            _ => steps += amount.signum() as i32 * amount.abs().round().max(1.0) as i32,
        }
    }
    let whole = (*points / POINTS_PER_STEP).trunc();
    *points -= whole * POINTS_PER_STEP;
    steps + whole as i32
}

/// The catalog action of this frame's wheel over `rect` (`back` or `forward`), when the
/// wheel steps there now (`enabled`); `None` otherwise. Over the panel the scroll is
/// taken. `id` names the panel's pending steps.
pub fn action(ui: &Ui, rect: Rect, enabled: bool, id: &str, (back, forward): (&str, &str)) -> Option<String> {
    let key = egui::Id::new(("qnc-wheel-jog", id));
    let ctx = ui.ctx();
    let mut pending: Pending = ctx.data(|data| data.get_temp(key)).unwrap_or_default();
    let over = enabled && ctx.pointer_hover_pos().is_some_and(|at| rect.contains(at));
    if over {
        let events = ctx.input(|input| input.events.clone());
        pending.steps += steps_of(&events, &mut pending.points);
        ctx.input_mut(|input| {
            input.smooth_scroll_delta = egui::Vec2::ZERO;
            input.raw_scroll_delta = egui::Vec2::ZERO;
        });
    } else {
        pending = Pending::default(); // the pointer left: nothing carries over
    }
    let step = pending.steps.signum();
    pending.steps -= step;
    ctx.data_mut(|data| data.insert_temp(key, pending));
    if pending.steps != 0 {
        ctx.request_repaint(); // the rest of a fast spin, one a frame
    }
    match step {
        -1 => Some(back.to_string()),
        1 => Some(forward.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(unit: egui::MouseWheelUnit, y: f32) -> egui::Event {
        egui::Event::MouseWheel { unit, delta: egui::vec2(0.0, y), modifiers: egui::Modifiers::NONE }
    }

    #[test]
    fn a_notch_is_one_step_down_is_forward() {
        let mut points = 0.0;
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Line, -1.0)], &mut points), 1);
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Line, 1.0), wheel(egui::MouseWheelUnit::Line, 1.0)], &mut points), -2);
    }

    #[test]
    fn a_touchpad_adds_up_its_points() {
        let mut points = 0.0;
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Point, -15.0)], &mut points), 0);
        assert_eq!(steps_of(&[wheel(egui::MouseWheelUnit::Point, -30.0)], &mut points), 1);
        assert!((points - 5.0).abs() < 0.01, "the rest is kept");
    }
}

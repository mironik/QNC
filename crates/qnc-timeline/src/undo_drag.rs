//! Undo and redo with the mouse (user 2026-10-29: "left-right drag is for the markers, up-down
//! for undo-redo"): on a row that allows it (`TimelineLayerFlags::undo_drag`, the Wrap rows),
//! a left-button drag that goes mostly up or down is not a scrub nor a marker drag but undo
//! steps (up, away from the user) or redo steps (down), one every `PIXELS_PER_STEP`. Which it
//! is is decided once, when the pointer has gone `DECIDE_PIXELS` from where it was pressed;
//! the timeline only says the step, the mouse map says the action.

use eframe::egui::{self, Pos2, Rect, Ui};

/// How far the pointer goes before the drag is decided.
const DECIDE_PIXELS: f32 = 10.0;
/// The pointer's move along of one undo or redo.
const PIXELS_PER_STEP: f32 = 40.0;

#[derive(Debug, Clone, Copy)]
struct Drag {
    /// Where the button went down.
    from: Pos2,
    /// `None` undecided, `Some(true)` up/down (undo), `Some(false)` across (left alone).
    along: Option<bool>,
    /// The height at which the last step was given.
    stepped_at: f32,
}

/// Whether a move of `delta` from the press is decided, and whether it is up/down.
fn decide(delta: egui::Vec2) -> Option<bool> {
    (delta.length() >= DECIDE_PIXELS).then(|| delta.y.abs() > 2.0 * delta.x.abs())
}

/// The steps a drag gives as the pointer goes from `stepped_at` to `y`: one at a time, the
/// rest in the next frames. `Some(false)` undo (up), `Some(true)` redo (down).
fn step(stepped_at: &mut f32, y: f32) -> Option<bool> {
    let moved = y - *stepped_at;
    if moved.abs() < PIXELS_PER_STEP {
        return None;
    }
    *stepped_at += PIXELS_PER_STEP * moved.signum();
    Some(moved > 0.0)
}

/// This frame of the mouse over `rect`: whether an up/down drag holds it (the row's own drags
/// are then not given) and its step, if any.
pub(crate) fn take(ui: &Ui, rect: Rect, key: egui::Id) -> (bool, Option<bool>) {
    let ctx = ui.ctx();
    let (pressed, down, at) = ctx.input(|input| {
        (input.pointer.primary_pressed(), input.pointer.primary_down(), input.pointer.interact_pos())
    });
    let mut drag: Option<Drag> = ctx.data(|data| data.get_temp(key));
    if pressed {
        drag = at.filter(|at| rect.contains(*at)).map(|from| Drag { from, along: None, stepped_at: from.y });
    }
    let (Some(mut held), true, Some(at)) = (drag, down, at) else {
        ctx.data_mut(|data| data.remove::<Drag>(key));
        return (false, None);
    };
    if held.along.is_none() {
        held.along = decide(at - held.from);
    }
    let along = held.along == Some(true);
    let given = if along { step(&mut held.stepped_at, at.y) } else { None };
    if along && (at.y - held.stepped_at).abs() >= PIXELS_PER_STEP {
        ctx.request_repaint();
    }
    ctx.data_mut(|data| data.insert_temp(key, held));
    (along, given)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_mostly_up_or_down_is_undo_steps_one_every_forty_pixels() {
        assert_eq!(decide(egui::vec2(3.0, 4.0)), None, "not far enough yet");
        assert_eq!(decide(egui::vec2(2.0, -12.0)), Some(true), "up: undo");
        assert_eq!(decide(egui::vec2(12.0, 3.0)), Some(false), "across: the markers");
        let mut at = 100.0;
        assert_eq!(step(&mut at, 70.0), None, "30 px up: no step yet");
        assert_eq!(step(&mut at, 15.0), Some(false), "up: undo");
        assert_eq!(step(&mut at, 15.0), Some(false), "the rest in the next frame");
        assert_eq!(step(&mut at, 15.0), None);
        assert_eq!(step(&mut at, 70.0), Some(true), "down: redo");
    }
}

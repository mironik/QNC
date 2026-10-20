//! When a form paints. A form asks this piece after its application polled, instead of
//! deciding itself (audit 2026-10-10: the rule lived in the forms' `app.rs`).
//!
//! - A change paints at once, except while the player plays: then the player's new
//!   picture wakes the paint (the source rate); painting at the screen rate took the
//!   graphics card from the HDMI output (live 2026-10-08).
//! - A cadence the application asks for paints later, only when this paint had no new
//!   picture (scheduling it after a wake would postpone the next source frame).
//! - Once a player exists, its pictures and states wake the paint.

use std::time::Duration;

use eframe::egui::Context;

/// A wake the player calls on every new picture or state.
pub type Wake = Box<dyn Fn() + Send + Sync + 'static>;

#[derive(Debug, Default)]
pub struct PaintPace {
    bound: bool,
}

impl PaintPace {
    pub fn new() -> Self {
        Self::default()
    }

    /// Hands the player a wake once a player exists (`bind` registers it with the player).
    pub fn bind_player(&mut self, ctx: &Context, player_exists: bool, bind: impl FnOnce(Wake)) {
        if self.bound || !player_exists {
            return;
        }
        let repaint = ctx.clone();
        bind(Box::new(move || repaint.request_repaint()));
        self.bound = true;
    }

    /// After the application polled: whether it `changed`, whether the player `playing`,
    /// and the cadence it asks for.
    pub fn after_poll(&self, ctx: &Context, changed: bool, playing: bool, cadence: Option<Duration>) {
        if let Some(when) = paint_when(changed, playing, cadence) {
            match when {
                Paint::Now => ctx.request_repaint(),
                Paint::After(delay) => ctx.request_repaint_after(delay),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Paint {
    Now,
    After(Duration),
}

fn paint_when(changed: bool, playing: bool, cadence: Option<Duration>) -> Option<Paint> {
    match (changed, playing) {
        // While the player plays only its new pictures paint: the source rate (25 for PAL,
        // 30 for NTSC, 50 for 50p), not more (user 2026-10-20: 61 paints a second for 50p).
        (_, true) => None,
        (true, false) => Some(Paint::Now),
        (false, false) => cadence.map(Paint::After),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_paints_at_once_unless_the_player_plays() {
        let cadence = Some(Duration::from_millis(40));
        assert_eq!(paint_when(true, false, cadence), Some(Paint::Now));
        assert_eq!(paint_when(true, true, cadence), None, "the player's picture paints it");
        assert_eq!(paint_when(true, true, None), None, "the player's picture wakes it");
        assert_eq!(paint_when(false, false, None), None);
    }
}

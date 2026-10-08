//! Diagnostics only: how often the desktop paints and who asked for each paint.
//! An idle desktop should hardly paint; every paint costs the processor the player
//! needs. Written once a second with player diagnostics on.

use std::{collections::BTreeMap, time::Instant};

#[derive(Default)]
pub(crate) struct PaintRate {
    since: Option<Instant>,
    frames: u32,
    causes: BTreeMap<String, u32>,
}

impl PaintRate {
    pub(crate) fn count(&mut self, ctx: &eframe::egui::Context, tab: &str) {
        if !qnc_dev_diagnostics::player_diagnostics_enabled() {
            return;
        }
        let since = *self.since.get_or_insert_with(Instant::now);
        self.frames += 1;
        for cause in ctx.repaint_causes() {
            *self.causes.entry(cause.to_string()).or_default() += 1;
        }
        if since.elapsed().as_secs_f32() < 1.0 {
            return;
        }
        let mut causes: Vec<_> = std::mem::take(&mut self.causes).into_iter().collect();
        causes.sort_by(|a, b| b.1.cmp(&a.1));
        let causes: Vec<String> = causes.iter().take(4).map(|(c, n)| format!("{n}x {c}")).collect();
        qnc_dev_diagnostics::log_line(
            qnc_dev_diagnostics::DiagnosticsStream::Player,
            format!("ui-paint-rate frames={} tab={tab} causes=[{}]", self.frames, causes.join("; ")),
        );
        (self.since, self.frames) = (Some(Instant::now()), 0);
    }
}

//! The clip the media pool points at (user 2026-10-20: "watch the focus on the media pool;
//! the filmstrip is read into memory if it exists, so a click shows it at once"). The card
//! grid tells which card the mouse is over, else the card the keyboard is on; the form host
//! takes it once per frame; whoever reads ahead gives each new clip once (the preview, docs/99). It
//! reads nothing itself and knows no form.

use eframe::egui;

pub const MODULE_ID: &str = "qnc.module.pool-pointer";

fn key() -> egui::Id {
    egui::Id::new("qnc-pool-pointer")
}

/// The card grid: this frame the pool points at `clip_id`.
pub fn point(ctx: &egui::Context, clip_id: &str) {
    ctx.data_mut(|data| data.insert_temp(key(), clip_id.to_string()));
}

/// The form host: what the pool pointed at in the frame it last drew (asked once).
pub fn take(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|data| data.remove_temp::<String>(key()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_grid_pointed_at_is_taken_once() {
        let ctx = egui::Context::default();
        point(&ctx, "clip-1");
        assert_eq!(take(&ctx).as_deref(), Some("clip-1"));
        assert_eq!(take(&ctx), None);
    }
}

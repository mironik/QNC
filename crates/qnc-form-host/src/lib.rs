//! The host of a form on the desktop (user 2026-10-20, rule 11: the Ingest and Story forms
//! each ran the same loop in their `app.rs`). Once per frame before the form paints: the
//! player's wake, the application's poll, when to paint (piece `qnc-paint-pace`) and the
//! keys of the frame as catalog actions (piece `qnc-key-intents`), each dispatched to the
//! application. It knows no application: one implements `FormApplication`.

use std::time::Duration;

use eframe::egui::Context;
use qnc_keyboard_shortcut::ShortcutCatalog;
use qnc_paint_pace::{PaintPace, Wake};

pub const MODULE_ID: &str = "qnc.module.form-host";

/// What the host asks of a form's application.
pub trait FormApplication {
    type Intent;
    /// A player exists (its pictures and states then wake the paint).
    fn has_player(&self) -> bool;
    fn on_player_change(&self, wake: Wake);
    /// Reads what changed; whether the view changed.
    fn poll(&mut self) -> bool;
    fn playing(&self) -> bool;
    /// The cadence the application asks for.
    fn next_repaint_delay(&self) -> Option<Duration>;
    /// The catalog scope of the panel with the keyboard.
    fn key_scope(&self) -> Option<&'static str>;
    fn intent_of(action_id: String) -> Self::Intent;
    /// Runs an intent; whether the view changed.
    fn dispatch(&mut self, intent: Self::Intent) -> bool;
}

/// The keys of a form: its catalog, its scopes and its play action.
pub struct FormKeys<'a> {
    pub catalog: &'a ShortcutCatalog,
    pub scopes: &'a [&'a str],
    pub play_action: &'a str,
}

#[derive(Debug, Default)]
pub struct FormHost {
    pace: PaintPace,
}

impl FormHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Before the form paints: the player's wake, the poll, when to paint, the keys.
    pub fn before_paint<A: FormApplication>(&mut self, ctx: &Context, application: &mut A, keys: &FormKeys<'_>) {
        let app = &*application;
        self.pace.bind_player(ctx, app.has_player(), |wake| app.on_player_change(wake));
        let changed = application.poll();
        self.pace.after_poll(ctx, changed, application.playing(), application.next_repaint_delay());
        for action_id in qnc_key_intents::action_ids_in(ctx, keys.catalog, application.key_scope(), keys.scopes, keys.play_action) {
            dispatch(ctx, application, A::intent_of(action_id));
        }
    }
}

/// Runs an intent of the form (a click its panels answered, a key); paints again when the
/// view changed.
pub fn dispatch<A: FormApplication>(ctx: &Context, application: &mut A, intent: A::Intent) {
    if application.dispatch(intent) {
        ctx.request_repaint();
    }
}

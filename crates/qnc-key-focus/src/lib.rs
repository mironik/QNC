//! The keys belong to the QNC keyboard catalog (user rule 2026-10-13: Tab walks the panels
//! of a form, Alt+ jumps). egui gives keyboard focus to a button, card or tab on Tab and
//! then every catalog key is reserved for that widget (Space, I/O, arrows stop working,
//! further Tabs walk widgets up to the footer). Called once a frame before the keys are
//! read, this takes the focus back from every widget except a text field, where typing
//! needs it. It draws nothing, reads no database and knows no form.

use eframe::egui;

pub const MODULE_ID: &str = "qnc.module.key-focus";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Takes keyboard focus away from a widget that is not a text field; returns whether it did.
pub fn keep_keys(ctx: &egui::Context) -> bool {
    let Some(focused) = ctx.memory(|memory| memory.focused()) else {
        return false;
    };
    if egui::text_edit::TextEditState::load(ctx, focused).is_some() {
        return false; // typing keeps its field
    }
    ctx.memory_mut(|memory| memory.surrender_focus(focused));
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Key;

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, ui: impl FnMut(&mut egui::Ui)) {
        let input = egui::RawInput { events, ..Default::default() };
        let mut ui = ui;
        let _ = ctx.run(input, |ctx| {
            keep_keys(ctx);
            egui::CentralPanel::default().show(ctx, |inner| ui(inner));
        });
    }

    fn tab() -> egui::Event {
        egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn a_button_does_not_keep_the_keys_but_a_text_field_does() {
        let ctx = egui::Context::default();
        let mut text = String::new();
        // Tab on a board of buttons: egui would focus the first one; the keys stay free.
        for _ in 0..3 {
            frame(&ctx, vec![tab()], |ui| {
                let _ = ui.button("A");
                let _ = ui.button("B");
            });
            frame(&ctx, vec![], |ui| {
                let _ = ui.button("A");
                let _ = ui.button("B");
            });
            assert!(ctx.memory(|memory| memory.focused()).is_none(), "no button keeps focus");
        }
        // A text field keeps the focus it was given.
        let mut id = None;
        frame(&ctx, vec![], |ui| {
            let response = ui.text_edit_singleline(&mut text);
            response.request_focus();
            id = Some(response.id);
        });
        frame(&ctx, vec![], |ui| {
            let _ = ui.text_edit_singleline(&mut text);
        });
        assert_eq!(ctx.memory(|memory| memory.focused()), id, "the text field keeps it");
    }
}

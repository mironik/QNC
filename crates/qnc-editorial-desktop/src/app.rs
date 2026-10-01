//! The editorial application object shared by the standalone executable and the
//! shell adapter of every group: the form plus the read-only component.

use std::path::PathBuf;

use eframe::egui::{self, CentralPanel, Frame};
use qnc_editorial_application::{action_ids, EditorialApplication, EditorialIntent};

use crate::EditorialForm;

const EDITORIAL_SHORTCUT_SCOPES: [&str; 2] = ["storyboard", "off"];

pub struct EditorialApp {
    form: EditorialForm,
    application: EditorialApplication,
    player_repaint_bound: bool,
}

impl EditorialApp {
    pub fn new(group: &str, root: PathBuf) -> Result<Self, String> {
        Ok(Self {
            form: EditorialForm::new(group)?,
            application: EditorialApplication::new(root),
            player_repaint_bound: false,
        })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut qnc_board::Frame<'_>) {
        if !self.player_repaint_bound && self.application.has_player() {
            let repaint = ctx.clone();
            self.application.notify_on_player_change(move || {
                repaint.request_repaint();
            });
            self.player_repaint_bound = true;
        }
        let changed = self.application.poll();
        if changed {
            ctx.request_repaint();
        } else if let Some(delay) = self.application.next_repaint_delay() {
            // Cadence only when this paint had no new picture; scheduling it
            // after a notify wake would postpone the next source frame.
            ctx.request_repaint_after(delay);
        }
        self.dispatch_keyboard_shortcuts(ctx);
        self.form.apply_theme(ctx);
        let intent = self.form.show_desktop(ui, frame, self.application.view());
        if let Some(intent) = intent {
            self.dispatch(ctx, intent);
        }
    }

    pub fn footer_status(&self) -> &str {
        self.application.footer_status()
    }

    /// The shell shows or hides this surface.
    pub fn set_active(&mut self, active: bool) {
        self.application.set_active(active);
    }

    fn dispatch(&mut self, ctx: &egui::Context, intent: EditorialIntent) {
        if self.application.dispatch(intent) {
            ctx.request_repaint();
        }
    }

    fn dispatch_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let ids = qnc_key_intents::action_ids(ctx, self.form.shortcuts(), &EDITORIAL_SHORTCUT_SCOPES, action_ids::PLAY_PAUSE);
        for action_id in ids {
            self.dispatch(ctx, EditorialIntent::action(action_id));
        }
    }
}

impl eframe::App for EditorialApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| self.show_desktop(ctx, ui, &mut qnc_board::Frame::bare()));
    }
}

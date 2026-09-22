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

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
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
        let intent = self.form.show_desktop(ui, self.application.view());
        if let Some(intent) = intent {
            self.dispatch(ctx, intent);
        }
    }

    pub fn footer_status(&self) -> &str {
        self.application.footer_status()
    }

    /// The surface became visible again: reread the active project cheaply.
    pub fn on_activated(&mut self) {
        self.application.refresh();
    }

    fn dispatch(&mut self, ctx: &egui::Context, intent: EditorialIntent) {
        if self.application.dispatch(intent) {
            ctx.request_repaint();
        }
    }

    fn dispatch_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let play_presses = qnc_keyboard_shortcut::consume_egui_action_presses(
            ctx,
            self.form.shortcuts(),
            "storyboard",
            action_ids::PLAY_PAUSE,
        );
        for _ in 0..play_presses {
            self.dispatch(ctx, EditorialIntent::action(action_ids::PLAY_PAUSE));
        }

        for event in qnc_keyboard_shortcut::egui_shortcut_events(ctx) {
            let mut handled_actions = Vec::new();
            for scope in EDITORIAL_SHORTCUT_SCOPES {
                let actions = self
                    .form
                    .shortcuts()
                    .action_ids_for_event(scope, &event)
                    .into_iter()
                    .filter_map(editorial_shortcut_action)
                    .collect::<Vec<_>>();
                for action_id in actions {
                    if handled_actions.contains(&action_id) {
                        continue;
                    }
                    handled_actions.push(action_id);
                    self.dispatch(ctx, EditorialIntent::action(action_id));
                }
            }
        }
    }
}

impl eframe::App for EditorialApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| self.show_desktop(ctx, ui));
    }
}

fn editorial_shortcut_action(action_id: &str) -> Option<&'static str> {
    match action_id {
        action_ids::PLAY_PAUSE => Some(action_ids::PLAY_PAUSE),
        action_ids::STEP_BACK_FRAME => Some(action_ids::STEP_BACK_FRAME),
        action_ids::STEP_FORWARD_FRAME => Some(action_ids::STEP_FORWARD_FRAME),
        action_ids::MARK_IN => Some(action_ids::MARK_IN),
        action_ids::MARK_OUT => Some(action_ids::MARK_OUT),
        action_ids::SAVE_VIRTUAL_SHOT => Some(action_ids::SAVE_VIRTUAL_SHOT),
        _ => None,
    }
}

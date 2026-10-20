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
    host: qnc_form_host::FormHost,
}

/// The editorial application as the form host sees it.
struct Hosted<'a>(&'a mut EditorialApplication);

impl qnc_form_host::FormApplication for Hosted<'_> {
    type Intent = EditorialIntent;
    fn has_player(&self) -> bool { self.0.has_player() }
    fn on_player_change(&self, wake: qnc_paint_pace::Wake) { self.0.notify_on_player_change(wake) }
    fn poll(&mut self) -> bool { self.0.poll() }
    fn playing(&self) -> bool { self.0.view().preview.playing }
    fn next_repaint_delay(&self) -> Option<std::time::Duration> { self.0.next_repaint_delay() }
    fn key_scope(&self) -> Option<&'static str> { self.0.view().focus.key_scope() }
    fn intent_of(action_id: String) -> EditorialIntent { EditorialIntent::action(&action_id) }
    fn dispatch(&mut self, intent: EditorialIntent) -> bool { self.0.dispatch(intent) }
}

impl EditorialApp {
    pub fn new(group: &str, root: PathBuf) -> Result<Self, String> {
        let form = EditorialForm::new(group)?;
        let application = EditorialApplication::new(root, form.focus_layout());
        Ok(Self { form, application, host: qnc_form_host::FormHost::new() })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut qnc_board::Frame<'_>) {
        let keys = qnc_form_host::FormKeys { catalog: self.form.shortcuts(), scopes: &EDITORIAL_SHORTCUT_SCOPES, play_action: action_ids::PLAY_PAUSE };
        self.host.before_paint(ctx, &mut Hosted(&mut self.application), &keys);
        self.form.apply_theme(ctx);
        if let Some(intent) = self.form.show_desktop(ui, frame, self.application.view()) {
            qnc_form_host::dispatch(ctx, &mut Hosted(&mut self.application), intent);
        }
    }

    pub fn footer_status(&self) -> &str {
        self.application.footer_status()
    }

    /// The shell shows or hides this surface.
    pub fn set_active(&mut self, active: bool) {
        self.application.set_active(active);
    }
}

impl eframe::App for EditorialApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        CentralPanel::default()
            .frame(Frame::NONE)
            .show(ctx, |ui| self.show_desktop(ctx, ui, &mut qnc_board::Frame::bare()));
    }
}

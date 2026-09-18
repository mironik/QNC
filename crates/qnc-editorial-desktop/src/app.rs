//! The editorial application object shared by the standalone executable and the
//! shell adapter of every group. It holds the form and an (empty) view; the
//! group functions fill the view later.

use eframe::egui::{self, CentralPanel, Frame};

use crate::{EditorialForm, EditorialIntent, EditorialView};

pub struct EditorialApp {
    form: EditorialForm,
    view: EditorialView,
    footer: String,
}

impl EditorialApp {
    pub fn new(group: &str) -> Result<Self, String> {
        Ok(Self {
            form: EditorialForm::new(group)?,
            view: EditorialView::default(),
            footer: String::new(),
        })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        self.form.apply_theme(ctx);
        if let Some(intent) = self.form.show_desktop(ui, &self.view) {
            self.handle(intent);
        }
    }

    pub fn footer_status(&self) -> &str {
        &self.footer
    }

    /// Only the timeline cue is handled here: it moves the shown playhead. The
    /// group functions (player, transcript, ...) come later.
    fn handle(&mut self, intent: EditorialIntent) {
        if let EditorialIntent::Timeline(qnc_timeline::TimelineIntent::CueFrame(frame)) = intent {
            self.view.timeline = self.view.timeline.with_playhead(frame);
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

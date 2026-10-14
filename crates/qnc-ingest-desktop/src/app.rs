use std::path::PathBuf;

use eframe::egui::{self, CentralPanel, Frame};
use qnc_ingest_application::{action_ids, IngestApplication, IngestIntent};

use crate::{
    layout_contract::IngestContracts,
    theme::{self, Theme},
};

pub struct IngestApp {
    contracts: IngestContracts,
    application: IngestApplication,
    pace: qnc_paint_pace::PaintPace,
}

impl IngestApp {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let contracts = IngestContracts::load()?;
        let application = IngestApplication::with_store_root(&root)?.with_focus(contracts.focus_names());
        Ok(Self { contracts, application, pace: qnc_paint_pace::PaintPace::new() })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut qnc_board::Frame<'_>) {
        let application = &self.application;
        let player_exists = application.view().playback.preparing || application.view().playback.reply.is_some();
        self.pace.bind_player(ctx, player_exists, |wake| application.notify_on_player_change(wake));
        let changed = self.application.poll();
        let playing = self.application.view().playback.playing();
        self.pace.after_poll(ctx, changed, playing, self.application.next_repaint_delay());
        self.dispatch_keyboard_shortcuts(ctx);
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        // The view is borrowed, not copied: it holds every clip and the timeline data.
        let intent = qnc_ingest_blocks::render_desktop(ui, frame, &self.contracts, &theme, self.application.view());
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

    /// Uvezi has started the import: the shell may open the next application.
    pub fn take_navigation_request(&mut self) -> bool {
        self.application.take_navigation_request()
    }

    pub fn navigation_sequence(&self) -> Result<Vec<qnc_ingest_application::SequenceStep>, String> {
        self.application.navigation_sequence()
    }

    fn dispatch(&mut self, ctx: &egui::Context, intent: IngestIntent) {
        let result = self.application.dispatch(intent);
        if result.request_repaint {
            ctx.request_repaint();
        }
    }

    fn dispatch_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let catalog = &self.contracts.shortcuts;
        for action_id in qnc_key_intents::action_ids(ctx, catalog, &["ingest"], action_ids::PLAY_PAUSE) {
            self.dispatch(ctx, IngestIntent::empty(action_id));
        }
    }
}

impl eframe::App for IngestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        CentralPanel::default()
            .frame(Frame::NONE.fill(theme.bg))
            .show(ctx, |ui| self.show_desktop(ctx, ui, &mut qnc_board::Frame::bare()));
    }
}

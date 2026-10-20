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
    host: qnc_form_host::FormHost,
}

/// The Ingest application as the form host sees it.
struct Hosted<'a>(&'a mut IngestApplication);

impl qnc_form_host::FormApplication for Hosted<'_> {
    type Intent = IngestIntent;
    fn has_player(&self) -> bool { self.0.has_player() }
    fn on_player_change(&self, wake: qnc_paint_pace::Wake) { self.0.notify_on_player_change(wake) }
    fn poll(&mut self) -> bool { self.0.poll() }
    fn playing(&self) -> bool { self.0.view().playback.playing() }
    fn next_repaint_delay(&self) -> Option<std::time::Duration> { self.0.next_repaint_delay() }
    fn key_scope(&self) -> Option<&'static str> { self.0.view().focus.key_scope() }
    fn intent_of(action_id: String) -> IngestIntent { IngestIntent::empty(action_id) }
    fn dispatch(&mut self, intent: IngestIntent) -> bool { self.0.dispatch(intent).request_repaint }
}

impl IngestApp {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        let contracts = IngestContracts::load()?;
        let application = IngestApplication::with_store_root(&root)?.with_focus(contracts.focus_names());
        Ok(Self { contracts, application, host: qnc_form_host::FormHost::new() })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, frame: &mut qnc_board::Frame<'_>) {
        let keys = qnc_form_host::FormKeys { catalog: &self.contracts.shortcuts, scopes: &["ingest"], play_action: action_ids::PLAY_PAUSE };
        self.host.before_paint(ctx, &mut Hosted(&mut self.application), &keys);
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        // The view is borrowed, not copied: it holds every clip and the timeline data.
        if let Some(intent) = qnc_ingest_blocks::render_desktop(ui, frame, &self.contracts, &theme, self.application.view()) {
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

    /// Uvezi has started the import: the shell may open the next application.
    pub fn take_navigation_request(&mut self) -> bool {
        self.application.take_navigation_request()
    }

    pub fn navigation_sequence(&self) -> Result<Vec<qnc_ingest_application::SequenceStep>, String> {
        self.application.navigation_sequence()
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

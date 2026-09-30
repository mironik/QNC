use eframe::egui;
use qnc_project_blocks::{layout_contract::AppContracts, theme::Theme};
use qnc_project_session::ProjectSession;

/// Project on the desktop board: the contracts, the session and its blocks. The form
/// only passes what the blocks ask for to the session.
pub struct ProjectApp {
    contracts: AppContracts,
    session: ProjectSession,
}

impl ProjectApp {
    pub(crate) fn new(contracts: AppContracts, session: ProjectSession) -> Self {
        Self { contracts, session }
    }

    pub fn footer_status(&self) -> &str {
        self.session.footer_status()
    }

    pub fn take_navigation_trigger(&mut self) -> bool {
        self.session.take_navigation_trigger()
    }

    pub fn navigation_sequence(
        &self,
    ) -> Result<Vec<qnc_project_application::ProjectNavigationStep>, String> {
        self.session.navigation_sequence()
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        if self.session.component.applications.poll() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        for intent in qnc_project_blocks::show(ctx, ui, &self.contracts, &mut self.session) {
            self.session.dispatch(intent);
        }
    }
}

impl eframe::App for ProjectApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let t = Theme::from_contract(&self.contracts.shell.colors);
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(t.bg))
            .show(ctx, |ui| self.show_desktop(ctx, ui));
    }
}

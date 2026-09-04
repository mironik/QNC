use std::path::PathBuf;

use eframe::egui::{self, CentralPanel, Frame};
use qnc_ingest_components::{IngestComponent, IngestIntent};

use crate::{
    layout_contract::IngestContracts,
    theme::{self, Theme},
    widgets,
};

pub struct IngestApp {
    root: PathBuf,
    contracts: IngestContracts,
    component: IngestComponent,
}

impl IngestApp {
    pub fn new(root: PathBuf) -> Result<Self, String> {
        Ok(Self {
            root,
            contracts: IngestContracts::load()?,
            component: IngestComponent::new(),
        })
    }

    pub fn show_desktop(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        ui.data_mut(|data| data.insert_temp(egui::Id::new("qnc_ingest_root"), self.root.clone()));
        let view = self.component.view().clone();
        if let Some(intent) = widgets::render_desktop(ui, &self.contracts, &theme, &view) {
            self.dispatch(ctx, intent);
        }
    }

    fn dispatch(&mut self, ctx: &egui::Context, intent: IngestIntent) {
        let result = self.component.dispatch(intent);
        if result.request_repaint {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for IngestApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let theme = Theme::from_contract(&self.contracts.shell);
        theme::apply(ctx, &theme);
        CentralPanel::default()
            .frame(Frame::NONE.fill(theme.bg))
            .show(ctx, |ui| self.show_desktop(ctx, ui));
    }
}

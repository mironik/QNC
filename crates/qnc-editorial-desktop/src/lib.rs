//! Editorial form base for Media Assist groups and Story: the Ingest layout
//! and UI copied 1:1 (shell, preview monitor, pool head, source dock with the
//! timeline), without the right clip grid and the directory browser. Passive:
//! the host fills `EditorialView` and receives `EditorialIntent`s.

mod layout_contract;
mod theme;
mod view;
mod widgets;

use eframe::egui;

pub use layout_contract::{check_contracts_message, EditorialContracts};
pub use view::{action_ids, EditorialIntent, EditorialView, MonitorFrame, Poster};

/// One editorial form for a group (e, g, l or o).
pub struct EditorialForm {
    contracts: EditorialContracts,
    theme: theme::Theme,
}

impl EditorialForm {
    pub fn new(group: &str) -> Result<Self, String> {
        let contracts = EditorialContracts::load(group)?;
        let theme = theme::Theme::from_contract(&contracts.shell);
        Ok(Self { contracts, theme })
    }

    pub fn group(&self) -> &str {
        &self.contracts.group
    }

    /// Applies fonts and visuals of the contract theme.
    pub fn apply_theme(&self, ctx: &egui::Context) {
        theme::apply(ctx, &self.theme);
    }

    /// Paints the whole form into `ui`; returns the intent of this frame.
    pub fn show_desktop(&self, ui: &mut egui::Ui, view: &EditorialView) -> Option<EditorialIntent> {
        widgets::render_desktop(ui, &self.contracts, &self.theme, view)
    }
}

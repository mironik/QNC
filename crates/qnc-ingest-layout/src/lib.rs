//! The model of the Ingest layout contract (moved unchanged out of the Ingest form,
//! user rule 2026-09-30: forms are boards of public pieces) and the Ingest places on
//! the one desktop board.

mod contracts;
mod model;

pub use contracts::{check_contracts_message, IngestContracts};
pub use model::*;
pub use qnc_desktop_theme as theme;
pub use qnc_desktop_theme::{ShellColors, ShellLayoutContract, ShellThemeMetrics};

/// A colour of the layout contract (`[r, g, b]`).
pub fn rgb([r, g, b]: [u8; 3]) -> eframe::egui::Color32 {
    eframe::egui::Color32::from_rgb(r, g, b)
}

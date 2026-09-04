mod app;
mod layout_contract;
mod theme;
mod widgets;

use std::path::PathBuf;

pub use app::IngestApp;
pub use layout_contract::check_contracts_message;

pub fn create_ingest_app(root: PathBuf) -> Result<IngestApp, String> {
    IngestApp::new(root)
}

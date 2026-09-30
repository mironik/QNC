mod app;
pub(crate) use qnc_ingest_layout as layout_contract;
pub(crate) use qnc_ingest_layout::theme;
mod widgets;

use std::path::PathBuf;

pub use app::IngestApp;
pub use layout_contract::check_contracts_message;

pub fn create_ingest_app(root: PathBuf) -> Result<IngestApp, String> {
    IngestApp::new(root)
}

//! The Ingest content database as one source adapter for the universal source preview.
//!
//! The preview itself is the neutral `qnc-source-preview`; this crate only says where
//! Ingest keeps its clips and which sources carry the media. Other applications must
//! provide their own source adapter over the same universal preview.

mod content;
mod panel;

pub use content::preview_context;
pub use panel::{Chosen, IngestPreviewSource, OpenFrom, PreviewView, Shown};
pub use qnc_source_preview::MonitorState;

pub const MODULE_ID: &str = "qnc.module.ingest-preview-source";
pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests;

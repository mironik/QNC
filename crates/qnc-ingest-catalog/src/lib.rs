//! The Ingest catalog of the active project: its rows on the clip list a form shows, one read
//! of it, that read off the caller's thread, and the project folder its posters are in.

mod folder;
mod loader;
mod read;
mod rows;

pub use folder::{project_folder, project_folder_for_settings};
pub use loader::CatalogLoader;
pub use read::{load, CatalogSource, LoadedCatalog};
pub use rows::{apply_catalog_rows, AppliedCatalogClips, CatalogClipItem, CatalogClipRow, CatalogThumbStatus, ThumbnailRequest};

pub const MODULE_ID: &str = "qnc.module.ingest-catalog";
pub const VERSION: &str = "0.1.0";

#[cfg(test)]
mod tests;

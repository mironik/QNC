//! The clip list reducer for Ingest Select events.
//!
//! This component owns the list mutation rules: incoming clips, existing marks,
//! save status, removed clips and Select completion text. It has no form, player,
//! database or source browser dependency.

use qnc_ingest_select as selection;

pub const MODULE_ID: &str = "qnc.module.ingest-clip-list";
pub const VERSION: &str = "0.1.0";

pub trait ClipListItem: Sized {
    fn clip_id(&self) -> &str;
    fn name(&self) -> &str;
    fn selected(&self) -> bool;
    fn metadata_revision(&self) -> u32;
    fn set_selected(&mut self, selected: bool);
    fn set_previously_seen(&mut self, seen: bool);
    fn set_save_failed(&mut self, failed: bool);
    fn from_selected_clip(clip: selection::SelectedClip) -> Self;
}

mod events;
mod row;

pub use events::{apply_select_event, SelectEventOutcome, SelectEventState};

#[cfg(test)]
mod tests;

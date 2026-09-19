//! Data the editorial form paints and the intents it sends back. Neutral: no
//! player, database or store type leaks through it.

pub use qnc_source_preview::{MonitorFrame, PreviewView};

/// One row of the clip list (summary only).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorialClip {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
}

#[derive(Clone, Default)]
pub struct EditorialView {
    pub clips: Vec<EditorialClip>,
    /// True while the project catalog is being read.
    pub loading: bool,
    /// Catalog status or controlled error text.
    pub message: String,
    /// The source preview: monitor picture, timeline, artifacts, chosen clip.
    pub preview: PreviewView,
}

impl EditorialView {
    /// Id of the clip in the monitor, if one is chosen.
    pub fn chosen_clip_id(&self) -> Option<&str> {
        self.preview.clip_id.as_deref()
    }

    /// Name of the clip in the monitor, if one is chosen.
    pub fn current_clip_label(&self) -> Option<&str> {
        let id = self.chosen_clip_id()?;
        self.clips
            .iter()
            .find(|clip| clip.clip_id == id)
            .map(|clip| clip.name.as_str())
    }
}

/// Neutral intent. `Action` carries an `action_id` from
/// `contracts/qnc-keyboard-shortcuts.json`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorialIntent {
    PreviewClip(String),
    Action(&'static str),
    Timeline(qnc_timeline::TimelineIntent),
}

pub mod action_ids {
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STEP_BACK_FRAME: &str = "step_back_frame";
    pub const STEP_FORWARD_FRAME: &str = "step_forward_frame";
}

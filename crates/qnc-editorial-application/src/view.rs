//! Data the editorial form paints and the intents it sends back. Neutral: no
//! player, database or store type leaks through it.

pub use qnc_source_preview::{MonitorFrame, PreviewView};
pub use qnc_virtual_short_cards::VirtualShortCard as EditorialShort;

/// Pool tab. Virtual lists short shots only, as in v5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LibraryTab {
    #[default]
    All,
    Virtual,
}

/// One row of the clip list (summary only).
#[derive(Debug, Clone, PartialEq)]
pub struct EditorialClip {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub duration_frames: u64,
    /// Import finished.
    pub imported: bool,
    /// Where the poster is (project folder or source), as the project database says.
    pub thumb_uri: Option<String>,
    /// The poster once it is loaded; the form paints a placeholder until then.
    pub thumb_image: Option<std::sync::Arc<qnc_image_assets::RgbaImage>>,
    /// The catalog record status; the status dots are made from it by the public card module.
    pub import_status: String,
    pub imported_media_uri: String,
}

#[derive(Clone, Default)]
pub struct EditorialView {
    pub library_tab: LibraryTab,
    pub clips: Vec<EditorialClip>,
    /// Short virtual shots for the Virtual tab, oldest first.
    pub shorts: Vec<EditorialShort>,
    /// The short card that is selected on the Virtual tab.
    pub chosen_shot_id: Option<String>,
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
    /// Open the parent clip and show this short's IN/OUT.
    PreviewShort(String),
    SwitchLibraryTab(LibraryTab),
    Action(String),
    Timeline(qnc_timeline::TimelineIntent),
}

impl EditorialIntent {
    pub fn action(action_id: impl Into<String>) -> Self {
        Self::Action(action_id.into())
    }
}

pub mod action_ids {
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STEP_BACK_FRAME: &str = "step_back_frame";
    pub const STEP_FORWARD_FRAME: &str = "step_forward_frame";
    pub const MARK_IN: &str = "mark_in";
    pub const MARK_OUT: &str = "mark_out";
    pub const SAVE_VIRTUAL_SHOT: &str = "save_virtual_shot";
}

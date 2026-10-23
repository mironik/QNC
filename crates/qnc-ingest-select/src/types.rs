use crate::*;

#[derive(Debug)]
pub enum Event {
    Status(String),
    Clip(SelectedClip),
    Existing(BTreeSet<String>),
    Saved {
        revisions: Vec<(String, u32)>,
        error: Option<String>,
    },
    Removed(Vec<String>),
    Warning(String),
    Finished(std::result::Result<Summary, String>),
}

#[derive(Debug)]
pub struct Summary {
    pub unchanged: usize,
    pub processed: usize,
    pub removed: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectSaveState {
    Pending,
    Failed,
    #[default]
    Saved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectThumbStatus {
    Ready,
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedClip {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub selected: bool,
    pub imported: bool,
    pub previously_seen: bool,
    pub metadata_revision: u32,
    pub save_state: SelectSaveState,
    pub thumb_uri: Option<String>,
    pub thumb_status: SelectThumbStatus,
    pub thumb_image: Option<Arc<qnc_image_assets::RgbaImage>>,
}


/// The project a Select writes into: its content (clips) and the intermediary of its
/// database for the media records and the source index (v5: all in the project).
#[derive(Debug, Clone)]
pub struct SelectTarget {
    pub content: ContentTarget,
    pub records: qnc_db_broker::ProjectDbTarget,
    /// The project whose posters Select brings in when it has published (user 2026-10-18:
    /// Select runs the blocks; the posters are copied or made at once).
    pub posters: Option<qnc_missing_posters::PosterProject>,
}

impl SelectTarget {
    /// Both from the active project settings: the same project database binding.
    pub fn for_project(
        reader: &qnc_work_settings::SettingsReader,
        settings: &qnc_work_settings::WorkSettings,
    ) -> std::result::Result<Self, String> {
        Ok(Self {
            content: ContentTarget::for_project(reader, settings)?,
            records: qnc_db_broker::ProjectDbTarget::for_project(reader, settings)?,
            posters: Some(qnc_missing_posters::PosterProject::new(reader.clone(), settings.clone())),
        })
    }
}

impl Default for SelectedClip {
    fn default() -> Self {
        Self {
            clip_id: String::new(),
            name: String::new(),
            duration_seconds: 0.0,
            selected: false,
            imported: false,
            previously_seen: false,
            metadata_revision: 0,
            save_state: SelectSaveState::Saved,
            thumb_uri: None,
            thumb_status: SelectThumbStatus::Missing,
            thumb_image: None,
        }
    }
}

#[cfg(test)]
use std::sync::mpsc;
use std::{path::Path, time::Duration};

use qnc_ingest_catalog as catalog;
#[cfg(test)]
use qnc_ingest_select as selection;
use qnc_ingest_select::selection_config;
use qnc_ingest_store::IngestStore;
pub use qnc_ingest_work_plan::{IngestMedia, IngestWorkPlan, PlaybackInput};
use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_timeline_assets::SourceTimelineAssets;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod clip_filter_tests;

mod actions;
mod browse;
mod clip_state;
mod clip_view;
mod dispatch;
mod dispatch_result;
mod intent;
mod navigation;
mod playback;
mod playback_guard;
mod poll;
mod settings;
mod source_kind;
mod state;
mod pieces;
mod thumbnails;
mod timeline_artifacts;
mod timeline_projection;
mod view_model;
#[cfg(test)]
mod work_settings_tests;

pub mod action_ids;
pub use clip_state::{ClipFilter, SaveState, ThumbStatus};
pub use clip_view::ClipView;
pub use dispatch_result::IngestDispatchResult;
pub use intent::{IngestIntent, IngestPayload};
pub use qnc_source_browse::BrowseLocationEntry as LocationEntry;
pub use navigation::SequenceStep;
pub use source_kind::SourceKind;
pub use timeline_projection::timeline_intent_to_ingest_intent;
pub use view_model::IngestViewModel;

pub(crate) use playback_guard::playback_guard_message;
pub(crate) use source_kind::source_kind_id;

#[derive(Debug, Default)]
pub struct IngestApplication {
    preview: qnc_ingest_preview_source::IngestPreviewSource,
    view: IngestViewModel,
    dispatch_log: Vec<String>,
    store: Option<IngestStore>,
    /// The active project: work plan, catalog, project database targets (piece `qnc-ingest-project`).
    project: qnc_ingest_project::IngestProject,
    /// Uvezi: the selection written once, then the background woken (piece `qnc-ingest-import-run`).
    import: qnc_ingest_import_run::ImportRun,
    navigation_requested: bool,
    posters: qnc_clip_posters::ClipPosters,
    pending_source: Option<String>,
    selection_config: Option<selection_config::SelectionConfig>,
    selection_config_error: Option<String>,
    browse: qnc_source_browse::SourceBrowse,
    /// One Select of a source (piece `qnc-ingest-select-run`).
    select: qnc_ingest_select_run::SelectRun,
    root: Option<std::path::PathBuf>,
    /// The keyboard walk over the panels (the form's layout contract gives the order).
    walk: qnc_focus_walk::FocusWalk,
    /// The pieces of the form (way B: a new possibility is a new piece, `qnc-ingest-pieces`).
    pieces: qnc_ingest_pieces::IngestPieces,
}

impl IngestApplication {
    pub fn new() -> Self {
        Self::default()
    }

    /// The Tab walk of the form: its panels, pool tabs and dock buttons (layout contract).
    pub fn with_focus(mut self, names: qnc_focus_walk::FocusNames<'_>) -> Self {
        self.walk = qnc_focus_walk::FocusWalk::new(names);
        self
    }

    /// Replaces the cameras Select can read (composition root only).
    pub fn with_camera_registry(mut self, registry: qnc_camera_adapter::CameraRegistry) -> Self {
        self.select = std::mem::take(&mut self.select).with_cameras(registry);
        self
    }

    pub fn with_store_root(root: impl AsRef<Path>) -> Result<Self, String> {
        let mut component = Self::new().with_camera_registry(qnc_ingest_cameras::registry(root.as_ref())?);
        component.root = Some(root.as_ref().to_path_buf());
        component.preview.source_preview_mut().wake_worker_from(root.as_ref());
        component.store = Some(IngestStore::open(root.as_ref())?);
        if let Some(mut browser) = component.refresh_sources() {
            let state = browser.roots("local").map(Into::into);
            component.browse.connect(browser);
            component.apply_source_browser_result(state);
        }
        // Active project comes from the DB, not from a disk scan or Project call.
        component.project = qnc_ingest_project::IngestProject::from_root(root.as_ref());
        match component.project.status().error.clone() {
            Some(error) => component.settings_failed(error),
            None => _ = component.load_work_settings(None),
        }
        Ok(component)
    }
}

impl Drop for IngestApplication {
    fn drop(&mut self) {
        self.stop_player();
        self.cancel_thumbnail_load();
        self.import.cancel();
        self.select.cancel();
    }
}

#[cfg(test)]
mod app_tests;

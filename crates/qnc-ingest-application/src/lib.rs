#[cfg(test)]
use std::sync::mpsc;
use std::{
    path::Path,
    time::Duration,
};

use qnc_dir_browser::{BrowserState, DirectoryBrowserSession};
use qnc_ingest_catalog as catalog;
use qnc_ingest_select as selection;
use qnc_ingest_select::selection_config;
use qnc_ingest_store::{content::CatalogStats, IngestStore, SourceSelectionRecord};
pub use qnc_ingest_work_plan::{IngestMedia, IngestWorkPlan, PlaybackInput};
use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_timeline_assets::SourceTimelineAssets;
use qnc_work_settings::SettingsReader;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod clip_filter_tests;
mod playback;
mod playback_guard;
mod timeline_artifacts;
#[cfg(test)]
mod work_settings_tests;

pub mod action_ids {
    pub const INGEST_SOURCE_KIND_LOCAL: &str = "ingest_source_kind_local";
    pub const INGEST_SOURCE_KIND_LAN: &str = "ingest_source_kind_lan";
    pub const INGEST_SOURCE_KIND_INTERNET: &str = "ingest_source_kind_internet";
    pub const INGEST_DIR_UP: &str = "ingest_dir_up";
    pub const INGEST_DIR_ROOTS: &str = "ingest_dir_roots";
    pub const INGEST_DIR_OPEN: &str = "ingest_dir_open";
    pub const INGEST_DIR_CONFIRM: &str = "ingest_dir_confirm";
    pub const INGEST_DIR_CANCEL: &str = "ingest_dir_cancel";
    pub const INGEST_PREVIEW_FOCUS: &str = "ingest_preview_focus";
    pub const INGEST_CLIP_TOGGLE: &str = "ingest_clip_toggle";
    pub const INGEST_SELECT_ALL: &str = "ingest_select_all";
    pub const INGEST_CLEAR_SELECTION: &str = "ingest_clear_selection";
    pub const INGEST_IMPORT_SELECTED: &str = "ingest_import_selected";
    pub const INGEST_RELOAD: &str = "ingest_reload";
    pub const INGEST_SET_CLIP_FILTER: &str = "ingest_set_clip_filter";
    pub const INGEST_SET_ARCHIVE: &str = "ingest_set_archive";
    pub const INGEST_SET_AI_MINING: &str = "ingest_set_ai_mining";
    pub const INGEST_APPROVE_PROXY_POSTERS: &str = "ingest_approve_proxy_posters";
    pub const INGEST_CUE_FRAME: &str = "ingest_cue_frame";
    pub const INGEST_TOGGLE_AUDIO_LANE: &str = "ingest_toggle_audio_lane";
    pub const PLAY_PAUSE: &str = "play_pause";
    pub const STEP_BACK_FRAME: &str = "step_back_frame";
    pub const STEP_FORWARD_FRAME: &str = "step_forward_frame";

    pub const ALL: &[&str] = &[
        INGEST_SOURCE_KIND_LOCAL,
        INGEST_SOURCE_KIND_LAN,
        INGEST_SOURCE_KIND_INTERNET,
        INGEST_DIR_UP,
        INGEST_DIR_ROOTS,
        INGEST_DIR_OPEN,
        INGEST_DIR_CONFIRM,
        INGEST_DIR_CANCEL,
        INGEST_PREVIEW_FOCUS,
        INGEST_CLIP_TOGGLE,
        INGEST_SELECT_ALL,
        INGEST_CLEAR_SELECTION,
        INGEST_IMPORT_SELECTED,
        INGEST_RELOAD,
        INGEST_SET_CLIP_FILTER,
        INGEST_SET_ARCHIVE,
        INGEST_SET_AI_MINING,
        INGEST_APPROVE_PROXY_POSTERS,
        INGEST_CUE_FRAME,
        INGEST_TOGGLE_AUDIO_LANE,
        PLAY_PAUSE,
        STEP_BACK_FRAME,
        STEP_FORWARD_FRAME,
    ];
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    #[default]
    Local,
    Lan,
    Internet,
}

impl SourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Računalo",
            Self::Lan => "LAN",
            Self::Internet => "Internet",
        }
    }

    pub fn action_id(self) -> &'static str {
        match self {
            Self::Local => action_ids::INGEST_SOURCE_KIND_LOCAL,
            Self::Lan => action_ids::INGEST_SOURCE_KIND_LAN,
            Self::Internet => action_ids::INGEST_SOURCE_KIND_INTERNET,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LocationEntry {
    pub name: String,
    pub qnc_uri: String,
    pub serial_number: String,
    pub volume_name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipView {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub selected: bool,
    pub imported: bool,
    pub previously_seen: bool,
    pub metadata_revision: u32,
    pub save_state: SaveState,
    pub thumb_uri: Option<String>,
    pub thumb_status: ThumbStatus,
    #[serde(skip)]
    pub thumb_image: Option<std::sync::Arc<qnc_image_assets::RgbaImage>>,
}

impl Default for ClipView {
    fn default() -> Self {
        Self {
            clip_id: String::new(),
            name: String::new(),
            duration_seconds: 0.0,
            selected: false,
            imported: false,
            previously_seen: false,
            metadata_revision: 0,
            save_state: SaveState::Saved,
            thumb_uri: None,
            thumb_status: ThumbStatus::Missing,
            thumb_image: None,
        }
    }
}

impl From<selection::SelectedClip> for ClipView {
    fn from(clip: selection::SelectedClip) -> Self {
        Self {
            clip_id: clip.clip_id,
            name: clip.name,
            duration_seconds: clip.duration_seconds,
            selected: clip.selected,
            imported: clip.imported,
            previously_seen: clip.previously_seen,
            metadata_revision: clip.metadata_revision,
            save_state: match clip.save_state {
                selection::SelectSaveState::Pending => SaveState::Pending,
                selection::SelectSaveState::Failed => SaveState::Failed,
                selection::SelectSaveState::Saved => SaveState::Saved,
            },
            thumb_uri: clip.thumb_uri,
            thumb_status: match clip.thumb_status {
                selection::SelectThumbStatus::Ready => ThumbStatus::Ready,
                selection::SelectThumbStatus::Pending => ThumbStatus::Pending,
                selection::SelectThumbStatus::Missing => ThumbStatus::Missing,
            },
            thumb_image: clip.thumb_image,
        }
    }
}

impl From<catalog::CatalogClipRow> for ClipView {
    fn from(clip: catalog::CatalogClipRow) -> Self {
        Self {
            clip_id: clip.clip_id,
            name: clip.name,
            duration_seconds: clip.duration_seconds,
            selected: clip.selected,
            imported: clip.imported,
            previously_seen: clip.previously_seen,
            metadata_revision: clip.metadata_revision,
            save_state: SaveState::Saved,
            thumb_uri: clip.thumb_uri,
            thumb_status: match clip.thumb_status {
                catalog::CatalogThumbStatus::Pending => ThumbStatus::Pending,
                catalog::CatalogThumbStatus::Missing => ThumbStatus::Missing,
            },
            thumb_image: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SaveState {
    Pending,
    Failed,
    #[default]
    Saved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThumbStatus {
    Ready,
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ClipFilter {
    New,
    #[default]
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestViewModel {
    #[serde(skip)]
    pub playback: qnc_player_client::View,
    #[serde(skip)]
    pub timeline: TimelineProjection,
    #[serde(skip)]
    pub timeline_assets: SourceTimelineAssets,
    pub source_kind: SourceKind,
    pub browser_roots: bool,
    pub browser_path_label: String,
    pub browser_current_uri: Option<String>,
    pub browser_parent_available: bool,
    pub browser_entries: Vec<LocationEntry>,
    pub browser_busy: bool,
    pub browser_error: Option<String>,
    pub selected_source_uri: Option<String>,
    pub selected_source_name: String,
    pub selected_source_serial_number: String,
    pub selected_source_volume_name: String,
    pub clips: Vec<ClipView>,
    pub clip_filter: ClipFilter,
    pub preview_clip_id: Option<String>,
    pub archive_original: bool,
    pub archive_original_available: bool,
    pub ai_mining: bool,
    pub command_busy: bool,
    pub select_warning_count: usize,
    pub work_settings_loading: bool,
    pub work_settings_ready: bool,
    pub work_settings_error: Option<String>,
    pub message: String,
}

impl Default for IngestViewModel {
    fn default() -> Self {
        Self {
            source_kind: SourceKind::Local,
            playback: Default::default(),
            timeline: Default::default(),
            timeline_assets: SourceTimelineAssets::empty(),
            browser_roots: true,
            browser_path_label: String::new(),
            browser_current_uri: None,
            browser_parent_available: false,
            browser_entries: Vec::new(),
            browser_busy: false,
            browser_error: None,
            selected_source_uri: None,
            selected_source_name: String::new(),
            selected_source_serial_number: String::new(),
            selected_source_volume_name: String::new(),
            clips: Vec::new(),
            clip_filter: ClipFilter::All,
            preview_clip_id: None,
            archive_original: false,
            archive_original_available: false,
            ai_mining: false,
            command_busy: false,
            select_warning_count: 0,
            work_settings_loading: false,
            work_settings_ready: false,
            work_settings_error: None,
            message: "Odaberi izvor.".to_string(),
        }
    }
}

pub fn timeline_intent_to_ingest_intent(intent: TimelineIntent) -> Option<IngestIntent> {
    match intent {
        TimelineIntent::CueFrame(frame) => Some(IngestIntent::new(
            action_ids::INGEST_CUE_FRAME,
            IngestPayload::Frame(frame.min(i64::MAX as u64) as i64),
        )),
        TimelineIntent::ToggleAudioExpand(_) => None,
        TimelineIntent::SelectVirtual { .. }
        | TimelineIntent::SelectCover { .. }
        | TimelineIntent::SelectMarkerSlot { .. }
        | TimelineIntent::SelectMarker { .. } => None,
        TimelineIntent::None => None,
    }
}

fn playback_timeline_projection(playback: &qnc_player_client::View) -> TimelineProjection {
    qnc_player_timeline::projection_from_player_reply(playback.reply.as_ref())
}

impl IngestViewModel {
    pub fn visible_clips(&self) -> impl Iterator<Item = &ClipView> {
        self.clips
            .iter()
            .filter(|clip| self.clip_filter == ClipFilter::All || !clip.previously_seen)
    }

    pub fn total_count(&self) -> usize {
        self.clips.len()
    }

    pub fn selected_count(&self) -> usize {
        self.clips.iter().filter(|clip| clip.selected).count()
    }

    pub fn imported_count(&self) -> usize {
        self.clips.iter().filter(|clip| clip.imported).count()
    }

    pub fn pending_count(&self) -> usize {
        self.total_count().saturating_sub(self.imported_count())
    }

    pub fn current_clip_label(&self) -> &str {
        self.preview_clip_id
            .as_deref()
            .and_then(|clip_id| {
                self.clips
                    .iter()
                    .find(|clip| clip.clip_id == clip_id)
                    .map(|clip| clip.name.as_str())
            })
            .unwrap_or("Odaberi klip")
    }

    pub fn status_label(&self) -> String {
        if self.select_warning_count > 0 && !self.command_busy {
            return format!(
                "{} klipova; {} upozorenja",
                self.clips.len(),
                self.select_warning_count
            );
        }
        if self.command_busy {
            return format!("Select: {} klipova", self.clips.len());
        }
        let imported = self.imported_count();
        let pending = self.pending_count();
        let selected = self.selected_count();
        let total = self.total_count();
        if pending > 0 {
            format!("{imported} uvezeno · {pending} nije uvezeno · {selected}/{total}")
        } else {
            format!("{imported} uvezeno · {selected}/{total}")
        }
    }

    pub fn proxy_poster_approval_count(&self) -> usize {
        self.clips
            .iter()
            .filter(|clip| clip.selected && matches!(clip.thumb_status, ThumbStatus::Missing))
            .count()
    }

    pub fn timeline_filmstrip_background(
        &self,
    ) -> Option<&qnc_timeline_assets::FilmstripBackground> {
        self.timeline_assets.filmstrip_background()
    }

    pub fn timeline_a1_peaks(&self) -> &[f32] {
        self.timeline_assets.a1_peaks()
    }

    pub fn timeline_a2_peaks(&self) -> &[f32] {
        self.timeline_assets.a2_peaks()
    }

    pub fn timeline_a3_peaks(&self) -> &[f32] {
        self.timeline_assets.a3_peaks()
    }

    pub fn timeline_a4_peaks(&self) -> &[f32] {
        self.timeline_assets.a4_peaks()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum IngestPayload {
    #[default]
    None,
    SourceKind(SourceKind),
    LocationUri(String),
    ClipId(String),
    Bool(bool),
    Frame(i64),
    AudioLane(String),
    ClipFilter(ClipFilter),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestIntent {
    pub action_id: String,
    pub payload: IngestPayload,
}

impl IngestIntent {
    pub fn new(action_id: impl Into<String>, payload: IngestPayload) -> Self {
        Self {
            action_id: action_id.into(),
            payload,
        }
    }

    pub fn empty(action_id: impl Into<String>) -> Self {
        Self::new(action_id, IngestPayload::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestDispatchResult {
    pub accepted: bool,
    pub message: Option<String>,
    pub request_repaint: bool,
}

impl IngestDispatchResult {
    fn accepted(message: Option<String>, request_repaint: bool) -> Self {
        Self {
            accepted: true,
            message,
            request_repaint,
        }
    }

    fn rejected(message: impl Into<String>) -> Self {
        Self {
            accepted: false,
            message: Some(message.into()),
            request_repaint: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct IngestApplication {
    preview: qnc_source_preview::SourcePreview,
    view: IngestViewModel,
    dispatch_log: Vec<String>,
    source_browser: DirectoryBrowserSession,
    store: Option<IngestStore>,
    settings_reader: Option<SettingsReader>,
    catalog_loader: catalog::CatalogLoader,
    catalog_target: Option<qnc_ingest_store::content::ContentTarget>,
    selection_writer: qnc_ingest_selection_write::SelectionWriter,
    thumbnail_loader: qnc_media_thumbnail::ThumbnailBatchService,
    artifacts: qnc_timeline_artifacts::Artifacts,
    catalog_stats: Option<CatalogStats>,
    work_plan: Option<IngestWorkPlan>,
    pending_source: Option<String>,
    selection_config: Option<selection_config::SelectionConfig>,
    selection_config_error: Option<String>,
    browse: qnc_source_browse::SourceBrowse,
    selection_session: selection::SelectSession,
    importer: qnc_ingest_import_worker::Importer,
    camera_registry: std::sync::Arc<qnc_camera_adapter::CameraRegistry>,
    selection_warnings: usize,
    selection_last_warning: Option<String>,
}

impl IngestApplication {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the cameras Select can read (composition root only).
    pub fn with_camera_registry(mut self, registry: qnc_camera_adapter::CameraRegistry) -> Self {
        self.camera_registry = std::sync::Arc::new(registry);
        self
    }

    pub fn with_store_root(root: impl AsRef<Path>) -> Result<Self, String> {
        let mut component = Self::new().with_camera_registry(qnc_ingest_cameras::registry()?);
        component.store = Some(IngestStore::open(root.as_ref())?);
        match selection_config::SelectionConfig::load(root.as_ref()).and_then(|config| {
            let browser = config.browser()?;
            Ok((config, browser))
        }) {
            Ok((config, mut browser)) => {
                let state = browser.roots("local");
                component.selection_config = Some(config);
                component.browse.connect(browser);
                component.apply_source_browser_result(state);
            }
            Err(error) => component.selection_config_error = Some(error.to_string()),
        }
        match SettingsReader::from_root(root.as_ref()) {
            Ok(reader) => {
                component.settings_reader = Some(reader);
                // Active project comes from the DB, not from a disk scan or Project call.
                component.load_work_settings(None);
            }
            Err(error) => component.settings_failed(error.to_string()),
        }
        Ok(component)
    }

    pub fn work_plan(&self) -> Option<&IngestWorkPlan> {
        self.work_plan
            .as_ref()
            .filter(|_| self.view.work_settings_ready)
    }

    pub fn footer_status(&self) -> &str {
        if self.view.work_settings_loading {
            "Ucitavanje projekta..."
        } else if let Some(error) = self.view.work_settings_error.as_deref() {
            error
        } else if let Some(plan) = self.work_plan() {
            &plan.settings.project_name
        } else {
            "Projekt nije ucitan."
        }
    }

    fn settings_failed(&mut self, error: String) {
        self.stop_player();
        self.cancel_thumbnail_load();
        self.work_plan = None;
        self.catalog_target = None;
        self.catalog_stats = None;
        self.view.clips.clear();
        self.view.timeline = Default::default();
        self.reset_timeline_artifacts();
        self.view.clip_filter = ClipFilter::All;
        self.view.preview_clip_id = None;
        self.pending_source = None;
        self.catalog_loader.cancel();
        self.view.work_settings_loading = false;
        self.view.work_settings_ready = false;
        self.view.work_settings_error = Some(error);
        self.view.ai_mining = false;
    }

    pub fn refresh_active_project(&mut self) -> IngestDispatchResult {
        self.load_work_settings_inner(None, true)
    }

    fn load_work_settings(&mut self, pending_source: Option<String>) -> IngestDispatchResult {
        self.load_work_settings_inner(pending_source, false)
    }

    fn load_work_settings_inner(
        &mut self,
        pending_source: Option<String>,
        retain_loaded_workspace: bool,
    ) -> IngestDispatchResult {
        if self.catalog_loader.is_busy()
            || self.selection_writer.is_busy()
            || self.selection_session.has_pending_work()
        {
            if retain_loaded_workspace && pending_source.is_none() && self.catalog_loader.is_busy() {
                return IngestDispatchResult::accepted(None, true);
            }
            return IngestDispatchResult::rejected("Citanje radnih postavki je u tijeku.");
        }
        let Some(reader) = self.settings_reader.clone() else {
            self.settings_failed("Nema konfiguriranog citaca radnih postavki.".into());
            return IngestDispatchResult::rejected("Nema konfiguriranog citaca radnih postavki.");
        };
        let keep_current_view =
            retain_loaded_workspace && pending_source.is_none() && self.work_plan.is_some();
        let retained_workspace = keep_current_view
            .then(|| {
                self.work_plan
                    .as_ref()
                    .map(|p| p.settings.workspace_db_uri.clone())
            })
            .flatten();
        let retained_stats = keep_current_view
            .then(|| self.catalog_stats.clone())
            .flatten();
        self.pending_source = pending_source;
        if !keep_current_view {
            self.stop_player();
            self.cancel_thumbnail_load();
            self.view.work_settings_loading = true;
            self.view.work_settings_ready = false;
        }
        self.view.work_settings_error = None;
        if let Err(error) = self.catalog_loader.start(reader, retained_workspace, retained_stats) {
            self.settings_failed(error);
        }
        IngestDispatchResult::accepted(None, true)
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = self.poll_settings();
        if self.preview.poll() {
            self.sync_playback_view();
            let message = self.preview.take_message();
            if !message.is_empty() {
                self.view.message = message;
            }
            changed = true;
        }
        self.apply_playback_guard();
        if !self.playback_guard_active() {
            changed |= self.poll_thumbnails();
        }
        self.apply_playback_guard();
        if !self.playback_guard_active() && self.artifacts.sync_deferred() {
            self.sync_timeline_artifact_content_db();
            changed = true;
        }
        changed |= self.poll_timeline_artifacts();
        if let Some(result) = self.selection_writer.poll() {
            match result {
                Ok(applied) => {
                    for clip in &mut self.view.clips {
                        if applied.clip_ids.contains(&clip.clip_id) {
                            clip.selected = applied.selected;
                        }
                    }
                    self.view.message = self.view.status_label();
                }
                Err(error) => self.view.message = error,
            }
            changed = true;
        }
        if let Some(result) = self.browse.poll() {
            self.apply_source_browser_result(result);
            changed = true;
        }
        let mut import_finished = false;
        for notice in self.importer.poll(64) {
            changed = true;
            self.view.message = notice.message;
            if notice.finished {
                self.view.command_busy = false;
                import_finished = true;
            }
        }
        if import_finished {
            // Re-read the catalog: the imported clips changed state in the database.
            self.load_work_settings(None);
        }
        for event in self.selection_session.poll(64) {
            changed = true;
            match event {
                selection::Event::Status(message) => self.view.message = message,
                selection::Event::Warning(message) => {
                    self.selection_warnings += 1;
                    self.view.select_warning_count = self.selection_warnings;
                    self.selection_last_warning = Some(message.clone());
                    self.view.message = message;
                }
                selection::Event::Clip(clip) => {
                    let mut clip = ClipView::from(clip);
                    if let Some(existing) = self
                        .view
                        .clips
                        .iter_mut()
                        .find(|c| c.clip_id == clip.clip_id)
                    {
                        clip.selected = existing.selected;
                        *existing = clip;
                    } else {
                        let index = self.view.clips.partition_point(|c| c.name < clip.name);
                        self.view.clips.insert(index, clip);
                    }
                }
                selection::Event::Existing(ids) => {
                    for clip in &mut self.view.clips {
                        if ids.contains(&clip.clip_id) {
                            clip.previously_seen = true;
                        }
                    }
                }
                selection::Event::Saved { revisions, error } => {
                    for clip in &mut self.view.clips {
                        if revisions.iter().any(|(id, revision)| {
                            id == &clip.clip_id && *revision == clip.metadata_revision
                        }) {
                            clip.save_state = if error.is_some() {
                                SaveState::Failed
                            } else {
                                SaveState::Saved
                            };
                        }
                    }
                    if let Some(error) = error {
                        self.selection_warnings += 1;
                        self.view.select_warning_count = self.selection_warnings;
                        self.selection_last_warning = Some(error.clone());
                        self.view.message = error;
                    }
                }
                selection::Event::Removed(ids) => {
                    self.view.clips.retain(|c| !ids.contains(&c.clip_id));
                    self.remove_timeline_artifact_clips(&ids);
                    if self
                        .view
                        .preview_clip_id
                        .as_ref()
                        .is_some_and(|id| ids.contains(id))
                    {
                        self.view.preview_clip_id = None;
                        self.stop_player();
                    }
                }
                selection::Event::Finished(result) => {
                    self.view.command_busy = false;
                    let finished_ok = result.is_ok();
                    self.view.message = match result {
                        Ok(summary) if self.selection_warnings == 0 => format!(
                            "Select: {} postojećih; {} obrađenih; {} uklonjenih ({:.1} s).",
                            summary.unchanged,
                            summary.processed,
                            summary.removed,
                            summary.elapsed_ms as f64 / 1000.0
                        ),
                        Ok(_) => format!(
                            "Select: {} klipova; {} upozorenja. {}",
                            self.view.clips.len(),
                            self.selection_warnings,
                            self.selection_last_warning.as_deref().unwrap_or_default()
                        ),
                        Err(error) => {
                            self.view.select_warning_count += 1;
                            error
                        }
                    };
                    if finished_ok {
                        self.sync_timeline_artifact_content_db();
                    }
                }
            }
        }
        changed
    }

    fn poll_settings(&mut self) -> bool {
        let Some(result) = self.catalog_loader.poll() else {
            return false;
        };
        self.view.work_settings_loading = false;
        match result {
            Ok(loaded) => {
                let plan = loaded.plan;
                let catalog_was_loaded = loaded.clips.is_some();
                // A new project must never inherit the preceding project's selection/preview.
                if self.work_plan.as_ref().map(|p| &p.settings.project_id)
                    != Some(&plan.settings.project_id)
                {
                    self.cancel_thumbnail_load();
                    self.view.clips.clear();
                    self.view.clip_filter = ClipFilter::All;
                    self.view.preview_clip_id = None;
                    self.reset_timeline_artifacts();
                    self.stop_player();
                    self.view.selected_source_uri = None;
                    self.view.selected_source_name.clear();
                    self.view.selected_source_serial_number.clear();
                    self.view.selected_source_volume_name.clear();
                    self.catalog_stats = None;
                }
                self.view.ai_mining = plan.settings.ai_enabled();
                self.view.archive_original_available = false;
                self.view.archive_original = false;
                self.view.work_settings_ready = true;
                self.catalog_target = Some(loaded.target);
                self.catalog_stats = Some(loaded.stats);
                if let Some(clips) = loaded.clips {
                    let clips = clips.into_iter().map(ClipView::from).collect::<Vec<_>>();
                    let thumbnails = clips
                        .iter()
                        .filter_map(|clip| {
                            clip.thumb_uri
                                .as_ref()
                                .map(|uri| (clip.clip_id.clone(), uri.clone()))
                        })
                        .collect::<Vec<_>>();
                    self.view.clips = clips;
                    self.start_thumbnail_load(thumbnails);
                }
                if self.pending_source.is_none() && catalog_was_loaded {
                    if let Some(source) = loaded.source {
                        self.view.selected_source_uri = Some(source.uri);
                        self.view.selected_source_name = source.name;
                        self.view.selected_source_serial_number = source.serial_number;
                        self.view.selected_source_volume_name = source.volume_name;
                    } else {
                        self.view.selected_source_uri = None;
                        self.view.selected_source_name.clear();
                        self.view.selected_source_serial_number.clear();
                        self.view.selected_source_volume_name.clear();
                    }
                }
                self.work_plan = Some(plan);
                self.refresh_timeline_artifact_context();
                if catalog_was_loaded {
                    self.sync_timeline_artifact_content_db();
                }
                if let Some(uri) = self.pending_source.take() {
                    if self.playback_guard_active() {
                        self.view.message = playback_guard_message().to_string();
                    } else {
                        self.confirm_source_selection(uri);
                    }
                }
            }
            Err(error) => self.settings_failed(error),
        }
        true
    }

    pub fn view(&self) -> &IngestViewModel {
        &self.view
    }

    pub fn has_pending_work(&self) -> bool {
        self.view.work_settings_loading
            || self.view.command_busy
            || self.view.browser_busy
            || self.catalog_loader.is_busy()
            || self.selection_writer.is_busy()
            || self.thumbnail_loader.has_pending_work()
            || self.selection_session.has_pending_work()
            || self.artifacts.has_pending_work()
            || self.preview.play_when_ready()
    }
    pub fn has_player(&self) -> bool {
        self.view.playback.preparing || self.view.playback.reply.is_some()
    }

    pub fn needs_player_poll(&self) -> bool {
        self.preview.play_when_ready() || self.view.playback.preparing || self.view.playback.playing()
    }

    fn playback_guard_active(&self) -> bool {
        playback_guard::PlaybackGuard::active(self.preview.play_when_ready(), &self.view)
    }

    fn apply_playback_guard(&mut self) {
        let active = self.playback_guard_active();
        self.set_timeline_artifact_playback_priority(active);
        if active {
            self.cancel_thumbnail_load();
            self.cancel_source_work_for_playback();
        }
    }

    fn cancel_source_work_for_playback(&mut self) {
        if self.selection_session.has_pending_work() {
            self.selection_session.cancel();
            self.view.command_busy = false;
            self.view.message = playback_guard_message().to_string();
        }
        if self.browse.cancel() {
            self.view.browser_busy = false;
            self.view.browser_error = Some(playback_guard_message().to_string());
        }
        self.pending_source = None;
    }

    fn playback_guard_rejected(&mut self) -> IngestDispatchResult {
        let message = playback_guard_message();
        self.view.message = message.to_string();
        let mut result = IngestDispatchResult::rejected(message);
        result.request_repaint = true;
        result
    }

    fn playback_guard_blocks_action(action_id: &str) -> bool {
        playback_guard::PlaybackGuard::blocks_action(action_id)
    }

    pub fn next_repaint_delay(&self) -> Option<Duration> {
        if self.needs_player_poll() {
            return self.view.playback.source_frame_interval();
        }
        if self.has_pending_work() {
            return Some(Duration::from_millis(100));
        }
        None
    }

    fn select_clips(&mut self, ids: Vec<String>, selected: bool) -> IngestDispatchResult {
        if self
            .view
            .clips
            .iter()
            .any(|c| ids.contains(&c.clip_id) && c.save_state != SaveState::Saved)
        {
            return IngestDispatchResult::rejected("Klip jos nije spremljen u bazu.");
        }
        if self.selection_writer.is_busy() || self.view.work_settings_loading {
            return IngestDispatchResult::rejected("DB odabir je u tijeku.");
        }
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected("Projektni katalog nije dostupan.");
        };
        match self.selection_writer.start(target, ids, selected) {
            Ok(()) => IngestDispatchResult::accepted(None, true),
            Err(error) => IngestDispatchResult::rejected(error),
        }
    }

    pub fn dispatch_log(&self) -> &[String] {
        &self.dispatch_log
    }

    pub fn dispatch(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        self.dispatch_log.push(intent.action_id.clone());

        if self.playback_guard_active() && Self::playback_guard_blocks_action(&intent.action_id) {
            return self.playback_guard_rejected();
        }

        let source_action = matches!(
            intent.action_id.as_str(),
            action_ids::INGEST_RELOAD
                | action_ids::INGEST_SOURCE_KIND_LOCAL
                | action_ids::INGEST_SOURCE_KIND_LAN
                | action_ids::INGEST_SOURCE_KIND_INTERNET
                | action_ids::INGEST_DIR_ROOTS
                | action_ids::INGEST_DIR_UP
                | action_ids::INGEST_DIR_OPEN
                | action_ids::INGEST_DIR_CONFIRM
                | action_ids::INGEST_DIR_CANCEL
        );
        if source_action
            && (self.selection_writer.is_busy()
                || self.view.command_busy
                || self.view.browser_busy
                || (self.pending_source.is_some()
                    && intent.action_id != action_ids::INGEST_DIR_CANCEL))
        {
            return IngestDispatchResult::rejected("Obrada odabranog izvora je u tijeku.");
        }
        if self.browse.is_connected() {
            match intent.action_id.as_str() {
                action_ids::INGEST_SOURCE_KIND_LOCAL => {
                    return self.browse_registered(SourceKind::Local, None);
                }
                action_ids::INGEST_SOURCE_KIND_LAN => {
                    return self.browse_registered(SourceKind::Lan, None);
                }
                action_ids::INGEST_SOURCE_KIND_INTERNET => {
                    return self.browse_registered(SourceKind::Internet, None);
                }
                action_ids::INGEST_DIR_ROOTS | action_ids::INGEST_DIR_CANCEL => {
                    return self.browse_registered(self.view.source_kind, None);
                }
                action_ids::INGEST_DIR_UP => {
                    return self.browse_registered(self.view.source_kind, Some(None));
                }
                action_ids::INGEST_DIR_OPEN => {
                    if let IngestPayload::LocationUri(uri) = &intent.payload {
                        self.capture_selected_source_metadata(uri);
                        return self
                            .browse_registered(self.view.source_kind, Some(Some(uri.clone())));
                    }
                }
                _ => {}
            }
        }

        match intent.action_id.as_str() {
            action_ids::INGEST_RELOAD => self.load_work_settings(None),
            action_ids::INGEST_SET_CLIP_FILTER => match intent.payload {
                IngestPayload::ClipFilter(filter) => {
                    self.view.clip_filter = filter;
                    IngestDispatchResult::accepted(None, true)
                }
                _ => IngestDispatchResult::rejected("Nedostaje filter klipova."),
            },
            action_ids::INGEST_SOURCE_KIND_LOCAL => {
                self.view.source_kind = SourceKind::Local;
                let result = self.source_browser.load_roots();
                self.apply_source_browser_result(result)
            }
            action_ids::INGEST_SOURCE_KIND_LAN => {
                self.view.source_kind = SourceKind::Lan;
                self.clear_source_browser_state();
                self.view.message = "LAN izvor nije povezan u ovom rezu.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_SOURCE_KIND_INTERNET => {
                self.view.source_kind = SourceKind::Internet;
                self.clear_source_browser_state();
                self.view.message = "Internet izvor nije povezan u ovom rezu.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            action_ids::INGEST_DIR_ROOTS => {
                if self.view.source_kind == SourceKind::Local {
                    let result = self.source_browser.load_roots();
                    self.apply_source_browser_result(result)
                } else {
                    IngestDispatchResult::accepted(None, true)
                }
            }
            action_ids::INGEST_DIR_UP => {
                if self.view.source_kind == SourceKind::Local {
                    let result = self.source_browser.open_parent();
                    self.apply_source_browser_result(result)
                } else {
                    IngestDispatchResult::accepted(None, true)
                }
            }
            action_ids::INGEST_DIR_OPEN => match intent.payload {
                IngestPayload::LocationUri(uri) if self.view.source_kind == SourceKind::Local => {
                    self.pending_source = None;
                    self.capture_selected_source_metadata(&uri);
                    let result = self.source_browser.open_uri(&uri);
                    self.apply_source_browser_result(result)
                }
                IngestPayload::LocationUri(_) => IngestDispatchResult::accepted(None, true),
                _ => IngestDispatchResult::rejected("Nedostaje QNC lokacijski URI."),
            },
            action_ids::INGEST_DIR_CONFIRM => match intent.payload {
                IngestPayload::LocationUri(uri) => {
                    if self.browse.selected(&uri).is_none()
                    {
                        let error = self
                            .selection_config_error
                            .clone()
                            .unwrap_or_else(|| "Odaberi disk ili mapu u browseru.".into());
                        self.view.message = error.clone();
                        return IngestDispatchResult::rejected(error);
                    }
                    self.load_work_settings(Some(uri))
                }
                _ => IngestDispatchResult::rejected("Nedostaje QNC lokacijski URI."),
            },
            action_ids::INGEST_DIR_CANCEL => self.cancel_source_browser(),
            action_ids::INGEST_SELECT_ALL => self.select_clips(
                self.view
                    .visible_clips()
                    .map(|c| c.clip_id.clone())
                    .collect(),
                true,
            ),
            action_ids::INGEST_CLEAR_SELECTION => self.select_clips(
                self.view
                    .visible_clips()
                    .map(|c| c.clip_id.clone())
                    .collect(),
                false,
            ),
            action_ids::INGEST_CLIP_TOGGLE => match intent.payload {
                IngestPayload::ClipId(clip_id) => {
                    if let Some(clip) = self
                        .view
                        .clips
                        .iter_mut()
                        .find(|clip| clip.clip_id == clip_id)
                    {
                        let selected = !clip.selected;
                        self.select_clips(vec![clip_id], selected)
                    } else {
                        IngestDispatchResult::rejected("Clip nije pronađen.")
                    }
                }
                _ => IngestDispatchResult::rejected("Nedostaje clip_id."),
            },
            action_ids::INGEST_PREVIEW_FOCUS => match intent.payload {
                IngestPayload::ClipId(clip_id) => self.prepare_preview(clip_id),
                _ => IngestDispatchResult::rejected("Nedostaje clip_id."),
            },
            action_ids::INGEST_SET_ARCHIVE => match intent.payload {
                IngestPayload::Bool(value) => {
                    self.view.archive_original = value;
                    IngestDispatchResult::accepted(None, true)
                }
                _ => IngestDispatchResult::rejected("Nedostaje bool vrijednost."),
            },
            action_ids::INGEST_SET_AI_MINING => IngestDispatchResult::rejected(
                "AI postavka dolazi iz baze; nema lokalnog overridea.",
            ),
            action_ids::INGEST_IMPORT_SELECTED => self.start_import(),
            action_ids::PLAY_PAUSE
            | action_ids::STEP_BACK_FRAME
            | action_ids::STEP_FORWARD_FRAME
            | action_ids::INGEST_CUE_FRAME => self.player_action(intent),
            _ => IngestDispatchResult::accepted(
                Some("Akcija je zapisana, komponenta za izvršenje još nije spojena.".to_string()),
                true,
            ),
        }
    }

    fn clear_source_browser_state(&mut self) {
        self.pending_source = None;
        self.view.browser_roots = true;
        self.view.browser_path_label.clear();
        self.view.browser_current_uri = None;
        self.view.browser_parent_available = false;
        self.view.browser_entries.clear();
        self.view.browser_busy = false;
        self.view.browser_error = None;
        self.view.selected_source_uri = None;
        self.view.selected_source_name.clear();
        self.view.selected_source_serial_number.clear();
        self.view.selected_source_volume_name.clear();
    }

    fn browse_registered(
        &mut self,
        kind: SourceKind,
        target: Option<Option<String>>,
    ) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if !self.browse.is_connected() {
            return IngestDispatchResult::rejected("Izvor nije povezan.");
        }
        self.view.source_kind = kind;
        let step = match target {
            None => {
                self.clear_source_browser_state();
                qnc_source_browse::Step::Roots(
                    match kind {
                        SourceKind::Internet => "intranet",
                        _ => source_kind_id(kind),
                    }
                    .into(),
                )
            }
            Some(None) => qnc_source_browse::Step::Parent,
            Some(Some(uri)) => qnc_source_browse::Step::Open(uri),
        };
        self.view.browser_busy = true;
        match self.browse.start(step) {
            Ok(()) => IngestDispatchResult::accepted(None, true),
            Err(error) => {
                self.view.browser_busy = false;
                IngestDispatchResult::rejected(error)
            }
        }
    }

    /// Queues the selected clips (the selection lives in the database) and starts
    /// the import in the background. What is copied and where is decided by the
    /// project settings in the work plan; the media is read through the transport
    /// of its source (local, LAN or intranet) and copied into the project folder of
    /// this machine.
    fn start_import(&mut self) -> IngestDispatchResult {
        let Some(plan) = self.work_plan.clone() else {
            return IngestDispatchResult::rejected("Radne postavke nisu dostupne.");
        };
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        let (Some(config), Some(target), Some(reader)) = (
            self.selection_config.as_ref(),
            self.catalog_target.clone(),
            self.settings_reader.as_ref(),
        ) else {
            return IngestDispatchResult::rejected("Uvoz nije dostupan: nema konfiguracije ili kataloga.");
        };
        match self
            .importer
            .start(reader, plan, config.sources.clone(), target)
        {
            Ok(()) => {
                self.view.command_busy = true;
                self.view.message = "Uvoz je pokrenut.".into();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => IngestDispatchResult::rejected(error),
        }
    }

    fn start_selection(&mut self, uri: &str) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        let Some(selected) = self.browse.selected(uri)
        else {
            return IngestDispatchResult::rejected("Odabrani izvor vise nije dostupan.");
        };
        let Some(config) = self.selection_config.clone() else {
            return IngestDispatchResult::rejected("Nema Select konfiguracije.");
        };
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected("Projektni katalog nije dostupan.");
        };
        match self
            .selection_session
            .start(config, selected, target, self.camera_registry.clone()) {
            Ok(()) => {
                self.selection_warnings = 0;
                self.view.select_warning_count = 0;
                self.selection_last_warning = None;
                self.view.command_busy = true;
                self.view.message = "Select je pokrenut.".into();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.message = error.to_string();
                IngestDispatchResult::rejected(error.to_string())
            }
        }
    }

    fn cancel_source_browser(&mut self) -> IngestDispatchResult {
        self.view.source_kind = SourceKind::Local;
        self.clear_source_browser_state();
        let result = self.source_browser.load_roots();
        let mut dispatch = self.apply_source_browser_result(result);
        self.view.message = "Odabir izvora je otkazan.".to_string();
        dispatch.message = None;
        dispatch
    }

    fn apply_source_browser_result(
        &mut self,
        result: Result<BrowserState, String>,
    ) -> IngestDispatchResult {
        match result {
            Ok(state) => {
                self.view.browser_roots = state.roots;
                self.view.browser_path_label = state.path_label;
                self.view.browser_current_uri = state.current_uri;
                self.view.browser_parent_available = state.parent_available;
                self.view.browser_entries = state
                    .entries
                    .into_iter()
                    .map(|entry| LocationEntry {
                        name: entry.name,
                        qnc_uri: entry.qnc_uri,
                        serial_number: entry.serial_number,
                        volume_name: entry.volume_name,
                    })
                    .collect();
                self.view.browser_busy = false;
                self.view.browser_error = None;
                self.view.message = "Odaberi lokalni izvor.".to_string();
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => {
                self.view.browser_busy = false;
                self.view.browser_error = Some(error.clone());
                self.view.message = error.clone();
                IngestDispatchResult::rejected(error)
            }
        }
    }

    fn capture_selected_source_metadata(&mut self, uri: &str) {
        if let Some(entry) = self
            .view
            .browser_entries
            .iter()
            .find(|entry| entry.qnc_uri == uri)
        {
            if !entry.serial_number.trim().is_empty() || !entry.volume_name.trim().is_empty() {
                self.view.selected_source_name = entry.name.clone();
                self.view.selected_source_serial_number = entry.serial_number.clone();
                self.view.selected_source_volume_name = entry.volume_name.clone();
            } else if self.view.selected_source_name.trim().is_empty() {
                self.view.selected_source_name = entry.name.clone();
            }
        }
    }

    fn start_thumbnail_load(&mut self, clips: Vec<(String, String)>) {
        self.cancel_thumbnail_load();
        if self.playback_guard_active() {
            return;
        }
        if clips.is_empty() {
            return;
        }
        let Some(config) = self.selection_config.clone() else {
            return;
        };
        let sources = config
            .sources
            .iter()
            .filter_map(|source| source.reader().ok())
            .collect::<Vec<_>>();
        if sources.is_empty() {
            return;
        }
        let requests = clips
            .into_iter()
            .map(|(clip_id, uri)| qnc_media_thumbnail::ThumbnailRequest {
                item_id: clip_id,
                uri,
            })
            .collect::<Vec<_>>();
        if let Err(error) = self.thumbnail_loader.start(sources, requests) {
            self.view.message = error;
        }
    }

    fn cancel_thumbnail_load(&mut self) {
        self.thumbnail_loader.cancel();
    }

    fn poll_thumbnails(&mut self) -> bool {
        let mut changed = false;
        for event in self.thumbnail_loader.poll(16) {
            match event {
                qnc_media_thumbnail::ThumbnailEvent::Ready {
                    item_id,
                    uri,
                    image,
                } => {
                    if let Some(clip) = self.view.clips.iter_mut().find(|clip| {
                        clip.clip_id == item_id && clip.thumb_uri.as_deref() == Some(uri.as_str())
                    }) {
                        clip.thumb_image = Some(image);
                        clip.thumb_status = ThumbStatus::Ready;
                        changed = true;
                    }
                }
                qnc_media_thumbnail::ThumbnailEvent::Finished => break,
            }
        }
        changed
    }

    fn confirm_source_selection(&mut self, uri: String) -> IngestDispatchResult {
        if self.playback_guard_active() {
            return self.playback_guard_rejected();
        }
        if self.work_plan.is_none() {
            return IngestDispatchResult::rejected("Radne postavke nisu dostupne.");
        }
        if qnc_contracts::parse_qnc_uri(&uri).is_err() {
            return IngestDispatchResult::rejected("Odabir izvora nije QNC URI.");
        }
        let display_name = if !self.view.selected_source_name.trim().is_empty() {
            self.view.selected_source_name.clone()
        } else if !self.view.browser_path_label.trim().is_empty() {
            self.view.browser_path_label.clone()
        } else {
            uri.clone()
        };
        let private_local_path = self.source_browser.path_for_uri(&uri);
        let record = SourceSelectionRecord {
            source_uri: uri.clone(),
            source_kind: source_kind_id(self.view.source_kind).to_string(),
            display_name,
            serial_number: self.view.selected_source_serial_number.clone(),
            volume_name: self.view.selected_source_volume_name.clone(),
            private_local_path,
        };

        if let Some(store) = self.store.as_mut() {
            match store.record_source_selection(&record) {
                Ok(session) => {
                    self.view.selected_source_uri = Some(uri.clone());
                    self.view.message = format!("Izvor je odabran: {}", session.selected_at_utc);
                    self.start_selection(&uri)
                }
                Err(error) => {
                    self.view.message = error.clone();
                    IngestDispatchResult::rejected(error)
                }
            }
        } else {
            self.view.selected_source_uri = Some(uri.clone());
            self.view.message = "Izvor je odabran.".to_string();
            self.start_selection(&uri)
        }
    }
}

impl Drop for IngestApplication {
    fn drop(&mut self) {
        self.stop_player();
        self.cancel_thumbnail_load();
        self.selection_writer.cancel();
        self.selection_session.cancel();
    }
}

fn source_kind_id(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Local => "local",
        SourceKind::Lan => "lan",
        SourceKind::Internet => "internet",
    }
}

fn playback_guard_message() -> &'static str {
    playback_guard::PlaybackGuard::message()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeSet, fs, path::Path};

    #[test]
    fn action_ids_are_unique() {
        let unique = action_ids::ALL.iter().copied().collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), action_ids::ALL.len());
    }

    #[test]
    fn component_starts_with_passive_empty_view() {
        let component = IngestApplication::new();
        assert_eq!(component.view().source_kind, SourceKind::Local);
        assert!(component.view().browser_roots);
        assert!(component.view().browser_entries.is_empty());
        assert!(component.view().clips.is_empty());
        assert_eq!(component.dispatch_log().len(), 0);
    }

    fn load_local_roots(component: &mut IngestApplication) {
        component.dispatch(IngestIntent::empty(action_ids::INGEST_DIR_ROOTS));
    }

    #[test]
    fn playback_requests_without_player_do_not_create_local_state() {
        let mut component = IngestApplication::default();
        component.view.preview_clip_id = Some("existing-preview".into());
        component.view.clip_filter = ClipFilter::New;
        let mut expected = component.view().clone();
        expected.message = "Broadcast Player nije povezan.".into();
        let mut requests = vec![
            IngestIntent::empty(action_ids::PLAY_PAUSE),
            IngestIntent::empty(action_ids::PLAY_PAUSE),
            IngestIntent::empty(action_ids::STEP_BACK_FRAME),
            IngestIntent::empty(action_ids::STEP_FORWARD_FRAME),
        ];
        for frame in [i64::MIN, 0, 25, i64::MAX] {
            requests.push(IngestIntent::new(
                action_ids::INGEST_CUE_FRAME,
                IngestPayload::Frame(frame),
            ));
        }
        for intent in requests {
            let result = component.dispatch(intent);
            assert!(!result.accepted);
            assert_eq!(result.message.as_deref(), Some(expected.message.as_str()));
            assert!(result.request_repaint);
            assert_eq!(component.view(), &expected);
            assert!(!component.has_pending_work());
        }
        let view = serde_json::to_value(component.view()).unwrap();
        assert!(view.get("playing").is_none());
        assert!(view.get("cue_frame").is_none());
    }

    #[test]
    fn playback_guard_blocks_background_source_work() {
        let mut component = IngestApplication::default();
        component.view.playback.preparing = true;

        component.start_thumbnail_load(vec![(
            "clip-1".into(),
            "qnc://local/source/card/Clip/0001.jpg".into(),
        )]);
        assert!(!component.thumbnail_loader.has_pending_work());

        let result = component.dispatch(IngestIntent::empty(action_ids::INGEST_RELOAD));
        assert!(!result.accepted);
        assert!(result.request_repaint);
        assert_eq!(result.message.as_deref(), Some(playback_guard_message()));
        assert_eq!(component.view().message, playback_guard_message());
    }

    #[test]
    fn clip_selection_is_not_blocked_by_the_playback_guard() {
        for (preparing, play_when_ready) in [(true, false), (false, true)] {
            let mut component = IngestApplication::default();
            component.view.playback.preparing = preparing;
            if play_when_ready {
                crate::playback::queue_play(&mut component);
            }
            assert!(component.playback_guard_active());

            let requests = [
                IngestIntent::empty(action_ids::INGEST_SELECT_ALL),
                IngestIntent::empty(action_ids::INGEST_CLEAR_SELECTION),
                IngestIntent::new(
                    action_ids::INGEST_CLIP_TOGGLE,
                    IngestPayload::ClipId("missing".into()),
                ),
            ];
            for intent in requests {
                let result = component.dispatch(intent);
                // Rejected for its own reason (no catalog / unknown clip), never by the guard.
                assert_ne!(result.message.as_deref(), Some(playback_guard_message()));
                assert_ne!(component.view().message, playback_guard_message());
            }
        }
    }

    #[test]
    fn playback_guard_defers_timeline_artifact_sync() {
        let mut component = IngestApplication::default();
        component.view.playback.preparing = true;

        component.sync_timeline_artifact_content_db();

        assert!(component.artifacts.sync_deferred());
        assert!(!component.artifacts.has_pending_work());
    }

    #[test]
    fn local_browser_exposes_qnc_uri_not_raw_path() {
        let mut component = IngestApplication::new();
        load_local_roots(&mut component);
        for entry in &component.view().browser_entries {
            assert!(entry.qnc_uri.starts_with("qnc://local/source/"));
            assert!(!qnc_contracts::looks_like_raw_os_path(&entry.qnc_uri));
        }
    }

    #[test]
    fn switching_source_kind_clears_stale_local_browser_state() {
        let mut component = IngestApplication::new();
        load_local_roots(&mut component);
        let first_uri = component
            .view()
            .browser_entries
            .first()
            .map(|entry| entry.qnc_uri.clone())
            .expect("local root");

        component.dispatch(IngestIntent::new(
            action_ids::INGEST_DIR_OPEN,
            IngestPayload::LocationUri(first_uri),
        ));
        assert!(!component.view().browser_path_label.is_empty());

        component.dispatch(IngestIntent::empty(action_ids::INGEST_SOURCE_KIND_LAN));

        assert_eq!(component.view().source_kind, SourceKind::Lan);
        assert!(component.view().browser_path_label.is_empty());
        assert!(component.view().browser_current_uri.is_none());
        assert!(component.view().browser_entries.is_empty());
        assert!(component.view().selected_source_name.is_empty());
        assert!(component.view().selected_source_serial_number.is_empty());
        assert!(component.view().selected_source_volume_name.is_empty());
    }

    #[test]
    fn cancel_source_browser_returns_to_local_roots() {
        let mut component = IngestApplication::new();
        load_local_roots(&mut component);
        let first_uri = component
            .view()
            .browser_entries
            .first()
            .map(|entry| entry.qnc_uri.clone())
            .expect("local root");

        component.dispatch(IngestIntent::new(
            action_ids::INGEST_DIR_OPEN,
            IngestPayload::LocationUri(first_uri),
        ));
        component.view.selected_source_name = "stale".to_string();
        component.view.selected_source_serial_number = "serial".to_string();
        component.view.selected_source_volume_name = "volume".to_string();

        component.dispatch(IngestIntent::empty(action_ids::INGEST_DIR_CANCEL));

        assert_eq!(component.view().source_kind, SourceKind::Local);
        assert!(component.view().browser_roots);
        assert!(component.view().browser_current_uri.is_none());
        assert!(!component.view().browser_entries.is_empty());
        assert!(component.view().selected_source_name.is_empty());
        assert!(component.view().selected_source_serial_number.is_empty());
        assert!(component.view().selected_source_volume_name.is_empty());
        assert_eq!(component.view().message, "Odabir izvora je otkazan.");
    }

    #[test]
    fn ingest_ui_crate_has_no_active_dependencies() {
        let ui_src = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("crate dir parent")
            .join("qnc-ingest-desktop")
            .join("src");
        let files = rust_files(&ui_src);
        assert!(!files.is_empty(), "expected ingest desktop sources");

        let forbidden = [
            "std::fs",
            "rusqlite",
            "std::process::Command",
            "Command::new",
            "ffprobe",
            "ffmpeg",
            "qnc_source_scanner",
            "qnc_media_probe",
            "qnc_filmstrip::",
            "qnc_wave::",
            "qnc_broadcast_player",
            "qnc_project_store",
            "pick_directory",
            "list_directory",
        ];

        for path in files {
            let text = fs::read_to_string(&path).expect("read ingest ui source");
            let scrubbed = strip_line_comments(&text);
            for needle in forbidden {
                assert!(
                    !scrubbed.contains(needle),
                    "{} must not contain active dependency marker {needle}",
                    path.display()
                );
            }
        }
    }

    fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).expect("read source dir") {
            let entry = entry.expect("source entry");
            let path = entry.path();
            if path.is_dir() {
                out.extend(rust_files(&path));
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
        out
    }

    fn strip_line_comments(text: &str) -> String {
        text.lines()
            .map(|line| line.split_once("//").map(|(code, _)| code).unwrap_or(line))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

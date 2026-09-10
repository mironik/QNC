use std::{
    path::Path,
    sync::mpsc::{self, Receiver, TryRecvError},
};

use qnc_dir_browser::{BrowserState, DirectoryBrowserSession};
use qnc_ingest_store::{content::CatalogStats, IngestStore, SourceSelectionRecord};
use qnc_work_settings::SettingsReader;
use serde::{Deserialize, Serialize};

mod catalog;
#[cfg(test)]
mod clip_filter_tests;
mod playback;
mod selection;
mod selection_config;
mod work_plan;
#[cfg(test)]
mod work_settings_tests;
pub use work_plan::{IngestMedia, IngestWorkPlan, PlaybackInput};

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
pub struct IngestComponent {
    player: Option<qnc_player_client::Player>,
    view: IngestViewModel,
    dispatch_log: Vec<String>,
    source_browser: DirectoryBrowserSession,
    store: Option<IngestStore>,
    settings_reader: Option<SettingsReader>,
    settings_result: Option<Receiver<Result<catalog::Loaded, String>>>,
    catalog_target: Option<qnc_ingest_store::content::ContentTarget>,
    catalog_result: Option<Receiver<Result<(Vec<String>, bool), String>>>,
    catalog_thread: Option<std::thread::JoinHandle<()>>,
    thumbnail_result: Option<Receiver<catalog::ThumbnailEvent>>,
    thumbnail_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    catalog_stats: Option<CatalogStats>,
    play_when_ready: bool,
    work_plan: Option<IngestWorkPlan>,
    pending_source: Option<String>,
    selection_config: Option<selection_config::SelectionConfig>,
    selection_config_error: Option<String>,
    transport_browser: Option<qnc_dir_browser::TransportBrowserSession>,
    browser_result: Option<
        Receiver<(
            qnc_dir_browser::TransportBrowserSession,
            Result<BrowserState, String>,
        )>,
    >,
    selection_result: Option<Receiver<selection::Event>>,
    selection_cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    selection_thread: Option<std::thread::JoinHandle<()>>,
    selection_warnings: usize,
    selection_last_warning: Option<String>,
}

impl IngestComponent {
    pub fn new() -> Self {
        let mut component = Self::default();
        let result = component.source_browser.load_roots();
        component.apply_source_browser_result(result);
        component
    }

    pub fn with_store_root(root: impl AsRef<Path>) -> Result<Self, String> {
        let mut component = Self::new();
        component.store = Some(IngestStore::open(root.as_ref())?);
        match selection_config::SelectionConfig::load(root.as_ref()).and_then(|config| {
            let browser = config.browser()?;
            Ok((config, browser))
        }) {
            Ok((config, mut browser)) => {
                let state = browser.roots("local");
                component.selection_config = Some(config);
                component.transport_browser = Some(browser);
                component.apply_source_browser_result(state);
            }
            Err(error) => component.selection_config_error = Some(error.to_string()),
        }
        match SettingsReader::from_root(root.as_ref()) {
            Ok(reader) => {
                component.settings_reader = Some(reader);
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
        self.view.clip_filter = ClipFilter::All;
        self.view.preview_clip_id = None;
        self.pending_source = None;
        self.settings_result = None;
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
        if self.settings_result.is_some()
            || self.catalog_result.is_some()
            || self.selection_result.is_some()
        {
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
        let (send, receive) = mpsc::sync_channel(1);
        let config = self.selection_config.clone();
        match std::thread::Builder::new()
            .name("ingest-work-settings".into())
            .spawn(move || {
                let result = catalog::load(
                    &reader,
                    config.as_ref(),
                    retained_workspace.as_deref(),
                    retained_stats.as_ref(),
                );
                let _ = send.send(result);
            }) {
            Ok(_) => self.settings_result = Some(receive),
            Err(_) => self.settings_failed("Nije moguce pokrenuti citanje radnih postavki.".into()),
        }
        IngestDispatchResult::accepted(None, true)
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = self.poll_settings();
        changed |= self.poll_thumbnails();
        if let Some(player) = &self.player {
            let playback = player.view();
            if self.view.playback != playback {
                if let Some(error) = &playback.error {
                    self.view.message = error.clone();
                    self.play_when_ready = false;
                }
                self.view.playback = playback;
                changed = true;
            }
        }
        if self.play_when_ready {
            if self.view.playback.error.is_some() {
                self.play_when_ready = false;
            } else if self.view.playback.can_start_playback() && !self.view.playback.playing() {
                if let Some(player) = &self.player {
                    match player.send(qnc_player_client::Action::TogglePlayPause) {
                        Ok(()) => self.play_when_ready = false,
                        Err(error) if error == "Player se priprema." => {}
                        Err(error) => {
                            self.play_when_ready = false;
                            self.view.message = error;
                        }
                    }
                    changed = true;
                }
            } else if self.view.playback.playing() {
                self.play_when_ready = false;
            }
        }
        if let Some(receiver) = &self.catalog_result {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Disconnected) => Some(Err("DB odabir je prekinut.".into())),
                Err(TryRecvError::Empty) => None,
            };
            if let Some(result) = result {
                self.catalog_result = None;
                if let Some(thread) = self.catalog_thread.take() {
                    let _ = thread.join();
                }
                match result {
                    Ok((ids, selected)) => {
                        for clip in &mut self.view.clips {
                            if ids.contains(&clip.clip_id) {
                                clip.selected = selected;
                            }
                        }
                        self.view.message = self.view.status_label();
                    }
                    Err(error) => self.view.message = error,
                }
                changed = true;
            }
        }
        if let Some(receiver) = &self.browser_result {
            match receiver.try_recv() {
                Ok((browser, result)) => {
                    self.browser_result = None;
                    self.transport_browser = Some(browser);
                    self.apply_source_browser_result(result);
                    changed = true;
                }
                Err(TryRecvError::Disconnected) => {
                    self.browser_result = None;
                    self.apply_source_browser_result(Err("Citanje izvora je prekinuto.".into()));
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        for _ in 0..64 {
            let Some(receiver) = &self.selection_result else {
                break;
            };
            let event = match receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    selection::Event::Finished(Err("Select proces je prekinut.".into()))
                }
            };
            changed = true;
            match event {
                selection::Event::Status(message) => self.view.message = message,
                selection::Event::Warning(message) => {
                    self.selection_warnings += 1;
                    self.view.select_warning_count = self.selection_warnings;
                    self.selection_last_warning = Some(message.clone());
                    self.view.message = message;
                }
                selection::Event::Clip(mut clip) => {
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
                    self.selection_result = None;
                    self.selection_cancel = None;
                    if let Some(thread) = self.selection_thread.take() {
                        let _ = thread.join();
                    }
                    self.view.command_busy = false;
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
                }
            }
        }
        changed
    }

    fn poll_settings(&mut self) -> bool {
        let Some(receiver) = self.settings_result.as_ref() else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err("Citanje radnih postavki je prekinuto.".into()),
        };
        self.settings_result = None;
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
                    if let Some((uri, name, serial, volume)) = loaded.source {
                        self.view.selected_source_uri = Some(uri);
                        self.view.selected_source_name = name;
                        self.view.selected_source_serial_number = serial;
                        self.view.selected_source_volume_name = volume;
                    } else {
                        self.view.selected_source_uri = None;
                        self.view.selected_source_name.clear();
                        self.view.selected_source_serial_number.clear();
                        self.view.selected_source_volume_name.clear();
                    }
                }
                self.work_plan = Some(plan);
                if let Some(uri) = self.pending_source.take() {
                    self.confirm_source_selection(uri);
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
            || self.settings_result.is_some()
            || self.catalog_result.is_some()
            || self.thumbnail_result.is_some()
            || self.play_when_ready
    }
    pub fn has_player(&self) -> bool {
        self.view.playback.preparing || self.view.playback.reply.is_some()
    }

    pub fn needs_player_poll(&self) -> bool {
        self.play_when_ready || self.view.playback.preparing || self.view.playback.playing()
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
        if self.catalog_result.is_some() || self.view.work_settings_loading {
            return IngestDispatchResult::rejected("DB odabir je u tijeku.");
        }
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected("Projektni katalog nije dostupan.");
        };
        let (send, receive) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("ingest-db-selection".into())
            .spawn(move || {
                let result = target
                    .open(qnc_ingest_store::content::Access::ReadWrite)
                    .and_then(|mut db| db.select(ids.clone(), selected))
                    .map(|()| (ids, selected));
                let _ = send.send(result);
            }) {
            Ok(thread) => {
                self.catalog_result = Some(receive);
                self.catalog_thread = Some(thread);
                IngestDispatchResult::accepted(None, true)
            }
            Err(error) => IngestDispatchResult::rejected(error.to_string()),
        }
    }

    pub fn dispatch_log(&self) -> &[String] {
        &self.dispatch_log
    }

    pub fn dispatch(&mut self, intent: IngestIntent) -> IngestDispatchResult {
        self.dispatch_log.push(intent.action_id.clone());

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
            && (self.catalog_result.is_some()
                || self.view.command_busy
                || self.view.browser_busy
                || (self.pending_source.is_some()
                    && intent.action_id != action_ids::INGEST_DIR_CANCEL))
        {
            return IngestDispatchResult::rejected("Obrada odabranog izvora je u tijeku.");
        }
        if self.transport_browser.is_some() {
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
                    if self
                        .transport_browser
                        .as_ref()
                        .and_then(|b| b.selected(&uri))
                        .is_none()
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
            action_ids::INGEST_IMPORT_SELECTED => {
                IngestDispatchResult::rejected(if self.work_plan.is_none() {
                    "Radne postavke nisu dostupne."
                } else {
                    "Media import jos nije implementiran."
                })
            }
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
        let Some(mut browser) = self.transport_browser.clone() else {
            return IngestDispatchResult::rejected("Izvor nije povezan.");
        };
        self.view.source_kind = kind;
        if target.is_none() {
            self.clear_source_browser_state();
        }
        self.view.browser_busy = true;
        let (send, receive) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("ingest-browser".into())
            .spawn(move || {
                let result = match target {
                    None => browser.roots(match kind {
                        SourceKind::Internet => "intranet",
                        _ => source_kind_id(kind),
                    }),
                    Some(None) => browser.parent(),
                    Some(Some(uri)) => browser.open(&uri),
                };
                let _ = send.send((browser, result));
            }) {
            Ok(_) => self.browser_result = Some(receive),
            Err(error) => {
                self.view.browser_busy = false;
                return IngestDispatchResult::rejected(error.to_string());
            }
        }
        IngestDispatchResult::accepted(None, true)
    }

    fn start_selection(&mut self, uri: &str) -> IngestDispatchResult {
        let Some(selected) = self
            .transport_browser
            .as_ref()
            .and_then(|b| b.selected(uri))
        else {
            return IngestDispatchResult::rejected("Odabrani izvor vise nije dostupan.");
        };
        let Some(config) = self.selection_config.clone() else {
            return IngestDispatchResult::rejected("Nema Select konfiguracije.");
        };
        let Some(target) = self.catalog_target.clone() else {
            return IngestDispatchResult::rejected("Projektni katalog nije dostupan.");
        };
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        let (send, receive) = mpsc::sync_channel(32);
        match std::thread::Builder::new()
            .name("ingest-select".into())
            .spawn(move || selection::run(config, selected, target, send, worker_cancel))
        {
            Ok(thread) => {
                self.selection_thread = Some(thread);
                self.selection_cancel = Some(cancel);
                self.selection_result = Some(receive);
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
        if clips.is_empty() {
            return;
        }
        let Some(config) = self.selection_config.clone() else {
            return;
        };
        let (send, receive) = mpsc::sync_channel(32);
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_cancel = cancel.clone();
        if std::thread::Builder::new()
            .name("ingest-thumbnail-load".into())
            .spawn(move || catalog::load_thumbnails(config, clips, send, worker_cancel))
            .is_ok()
        {
            self.thumbnail_result = Some(receive);
            self.thumbnail_cancel = Some(cancel);
        }
    }

    fn cancel_thumbnail_load(&mut self) {
        if let Some(cancel) = self.thumbnail_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.thumbnail_result = None;
    }

    fn poll_thumbnails(&mut self) -> bool {
        let mut changed = false;
        for _ in 0..16 {
            let Some(receiver) = self.thumbnail_result.as_ref() else {
                break;
            };
            let event = match receiver.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => catalog::ThumbnailEvent::Finished,
            };
            match event {
                catalog::ThumbnailEvent::Ready {
                    clip_id,
                    uri,
                    image,
                } => {
                    if let Some(clip) = self.view.clips.iter_mut().find(|clip| {
                        clip.clip_id == clip_id && clip.thumb_uri.as_deref() == Some(uri.as_str())
                    }) {
                        clip.thumb_image = Some(image);
                        clip.thumb_status = ThumbStatus::Ready;
                        changed = true;
                    }
                }
                catalog::ThumbnailEvent::Finished => {
                    self.thumbnail_result = None;
                    self.thumbnail_cancel = None;
                    break;
                }
            }
        }
        changed
    }

    fn confirm_source_selection(&mut self, uri: String) -> IngestDispatchResult {
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

impl Drop for IngestComponent {
    fn drop(&mut self) {
        self.cancel_thumbnail_load();
        self.catalog_result = None;
        if let Some(thread) = self.catalog_thread.take() {
            let _ = thread.join();
        }
        if let Some(cancel) = self.selection_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.selection_result = None;
        if let Some(thread) = self.selection_thread.take() {
            let _ = thread.join();
        }
    }
}

fn source_kind_id(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Local => "local",
        SourceKind::Lan => "lan",
        SourceKind::Internet => "internet",
    }
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
        let component = IngestComponent::new();
        assert_eq!(component.view().source_kind, SourceKind::Local);
        assert!(component.view().browser_roots);
        assert!(component
            .view()
            .browser_entries
            .iter()
            .all(|entry| qnc_contracts::parse_qnc_uri(&entry.qnc_uri).is_ok()));
        assert!(component.view().clips.is_empty());
        assert_eq!(component.dispatch_log().len(), 0);
    }

    #[test]
    fn playback_requests_without_player_do_not_create_local_state() {
        let mut component = IngestComponent::default();
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
    fn local_browser_exposes_qnc_uri_not_raw_path() {
        let component = IngestComponent::new();
        for entry in &component.view().browser_entries {
            assert!(entry.qnc_uri.starts_with("qnc://local/source/"));
            assert!(!qnc_contracts::looks_like_raw_os_path(&entry.qnc_uri));
        }
    }

    #[test]
    fn switching_source_kind_clears_stale_local_browser_state() {
        let mut component = IngestComponent::new();
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
        let mut component = IngestComponent::new();
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

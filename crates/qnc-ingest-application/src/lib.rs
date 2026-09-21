#[cfg(test)]
use std::sync::mpsc;
use std::{path::Path, time::Duration};

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

mod actions;
mod browse;
mod clip_state;
mod clip_view;
mod dispatch;
mod dispatch_result;
mod intent;
mod location_entry;
mod navigation;
mod playback;
mod playback_guard;
mod poll;
mod settings;
mod source_kind;
mod state;
mod thumbnails;
mod timeline_artifacts;
mod timeline_projection;
mod view_model;
#[cfg(test)]
mod work_settings_tests;
mod worker_launch;

pub mod action_ids;
pub use clip_state::{ClipFilter, SaveState, ThumbStatus};
pub use clip_view::ClipView;
pub use dispatch_result::IngestDispatchResult;
pub use intent::{IngestIntent, IngestPayload};
pub use location_entry::LocationEntry;
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
    settings_reader: Option<SettingsReader>,
    catalog_loader: catalog::CatalogLoader,
    catalog_target: Option<qnc_ingest_store::content::ContentTarget>,
    artifact_target: Option<qnc_content_store::ContentTarget>,
    selection_writer: qnc_ingest_selection_write::SelectionWriter,
    navigation_requested: bool,
    thumbnail_loader: qnc_media_thumbnail::ThumbnailBatchService,
    artifacts: qnc_content_artifacts::ProjectArtifacts,
    catalog_stats: Option<CatalogStats>,
    work_plan: Option<IngestWorkPlan>,
    pending_source: Option<String>,
    selection_config: Option<selection_config::SelectionConfig>,
    selection_config_error: Option<String>,
    browse: qnc_source_browse::SourceBrowse,
    selection_session: selection::SelectSession,
    root: Option<std::path::PathBuf>,
    worker: worker_launch::WorkerLauncher,
    runtime: qnc_ingest_runtime::PlaybackReporter,
    camera_registry: std::sync::Arc<qnc_camera_adapter::CameraRegistry>,
    selection_events: qnc_ingest_clip_list::SelectEventState,
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
        component.root = Some(root.as_ref().to_path_buf());
        component.artifacts.set_host_root(root.as_ref());
        component.store = Some(IngestStore::open(root.as_ref())?);
        match selection_config::SelectionConfig::load(root.as_ref()).and_then(|config| {
            let browser = config.browser()?;
            Ok((config, browser))
        }) {
            Ok((config, mut browser)) => {
                let state = browser.roots("local").map(Into::into);
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
}

impl Drop for IngestApplication {
    fn drop(&mut self) {
        self.stop_player();
        self.cancel_thumbnail_load();
        self.selection_writer.cancel();
        self.selection_session.cancel();
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

    fn wait_for_component(component: &mut IngestApplication) {
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while component.has_pending_work() {
            component.poll();
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn component_with_registered_source() -> (tempfile::TempDir, IngestApplication) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("PRIVATE")).unwrap();
        let uri = "qnc://local/source/test-card";
        let path = dir.path().to_path_buf();
        let browser = qnc_dir_browser::TransportBrowserSession::new(vec![
            qnc_dir_browser::BrowserSource::new(
                qnc_dir_browser::BrowserEntry {
                    name: "Test".into(),
                    qnc_uri: uri.into(),
                    serial_number: "serial".into(),
                    volume_name: "volume".into(),
                },
                move || {
                    qnc_source_reader::SourceReader::local(uri, &path).map_err(|e| e.to_string())
                },
            ),
        ])
        .unwrap();
        let mut component = IngestApplication::new();
        component.browse.connect(browser);
        (dir, component)
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
    fn proxy_poster_approval_action_is_explicitly_handled() {
        let mut component = IngestApplication::default();
        component.view.clips.push(ClipView {
            clip_id: "clip-1".into(),
            selected: true,
            thumb_status: ThumbStatus::Missing,
            ..Default::default()
        });

        let result = component.dispatch(IngestIntent::empty(
            action_ids::INGEST_APPROVE_PROXY_POSTERS,
        ));

        assert!(!result.accepted);
        assert_eq!(
            result.message.as_deref(),
            Some(
                "Samostalno generiranje postera jos nije spojeno; posteri se generiraju tijekom uvoza."
            )
        );
    }

    #[test]
    fn local_browser_exposes_qnc_uri_not_raw_path() {
        let (_dir, mut component) = component_with_registered_source();
        load_local_roots(&mut component);
        wait_for_component(&mut component);
        for entry in &component.view().browser_entries {
            assert!(entry.qnc_uri.starts_with("qnc://local/source/"));
            assert!(!qnc_contracts::looks_like_raw_os_path(&entry.qnc_uri));
        }
    }

    #[test]
    fn switching_source_kind_clears_stale_local_browser_state() {
        let (_dir, mut component) = component_with_registered_source();
        load_local_roots(&mut component);
        wait_for_component(&mut component);
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
        wait_for_component(&mut component);
        assert!(!component.view().browser_path_label.is_empty());

        component.dispatch(IngestIntent::empty(action_ids::INGEST_SOURCE_KIND_LAN));
        wait_for_component(&mut component);

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
        let (_dir, mut component) = component_with_registered_source();
        load_local_roots(&mut component);
        wait_for_component(&mut component);
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
        wait_for_component(&mut component);
        component.view.selected_source_name = "stale".to_string();
        component.view.selected_source_serial_number = "serial".to_string();
        component.view.selected_source_volume_name = "volume".to_string();

        component.dispatch(IngestIntent::empty(action_ids::INGEST_DIR_CANCEL));
        wait_for_component(&mut component);

        assert_eq!(component.view().source_kind, SourceKind::Local);
        assert!(component.view().browser_roots);
        assert!(component.view().browser_current_uri.is_none());
        assert!(!component.view().browser_entries.is_empty());
        assert!(component.view().selected_source_name.is_empty());
        assert!(component.view().selected_source_serial_number.is_empty());
        assert!(component.view().selected_source_volume_name.is_empty());
        assert_eq!(component.view().message, "Odaberi lokalni izvor.");
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

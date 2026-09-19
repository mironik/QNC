//! Editorial application composition root, shared by the Media Assist and Story
//! groups. It follows the chain of AGENTS.md section 4.1: active project from
//! the DB, work settings, project catalog (clip list), then the Broadcast
//! Player and the published timeline artifacts. Everything here is read-only:
//! no scan, no probe, no write to any database. The form only paints
//! `EditorialView` and sends `EditorialIntent`.

mod content;
mod view;

use std::{
    path::Path,
    sync::{
        mpsc::{self, Receiver, TryRecvError},
        Arc,
    },
    time::Duration,
};

use qnc_ingest_catalog as catalog;
use qnc_ingest_store::content::{CatalogStats, ContentTarget};
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_player_client::{Action, Player, View as PlayerView};
use qnc_player_input::InputReader;
use qnc_player_launcher::SourceTransportBinding;
use qnc_source_bindings::SourceBinding;
use qnc_timeline::TimelineIntent;
use qnc_timeline_assets::{SourceTimelineAssets, TimelineAssetContext, TimelineAssetReader};
use qnc_work_settings::SettingsReader;

pub use view::{action_ids, EditorialClip, EditorialIntent, EditorialView, MonitorFrame};

/// Finds the QNC root (the directory with `AGENTS.md` and the editorial layout
/// contract) starting from the executable and the working directory.
pub fn locate_qnc_root() -> Option<std::path::PathBuf> {
    let mut starts = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        starts.push(exe);
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    for start in starts {
        let mut current = if start.is_file() {
            start.parent().map(std::path::PathBuf::from)
        } else {
            Some(start)
        };
        while let Some(dir) = current {
            if dir.join("AGENTS.md").is_file()
                && dir
                    .join("contracts")
                    .join("ui")
                    .join("editorial.layout.json")
                    .is_file()
            {
                return Some(dir);
            }
            current = dir.parent().map(std::path::PathBuf::from);
        }
    }
    None
}

type CatalogResult = Result<catalog::LoadedCatalog, String>;

pub struct EditorialApplication {
    view: EditorialView,
    player: Option<Player>,
    player_view: PlayerView,
    settings_reader: Option<SettingsReader>,
    settings_result: Option<Receiver<CatalogResult>>,
    catalog_target: Option<ContentTarget>,
    catalog_stats: Option<CatalogStats>,
    work_plan: Option<IngestWorkPlan>,
    bindings: Result<Vec<SourceBinding>, String>,
    timeline_assets: TimelineAssetReader,
    play_when_ready: bool,
}

impl Default for EditorialApplication {
    fn default() -> Self {
        Self {
            view: EditorialView::default(),
            player: None,
            player_view: PlayerView::default(),
            settings_reader: None,
            settings_result: None,
            catalog_target: None,
            catalog_stats: None,
            work_plan: None,
            bindings: Err("Izvori medija nisu ucitani.".into()),
            timeline_assets: TimelineAssetReader::default(),
            play_when_ready: false,
        }
    }
}

impl EditorialApplication {
    /// Starts from the QNC root: the active project and its settings come from
    /// the DB, never from a default. Missing settings end in a controlled error.
    pub fn new(root: impl AsRef<Path>) -> Self {
        let mut app = Self::default();
        app.bindings = qnc_source_bindings::load(root.as_ref());
        match SettingsReader::from_root(root.as_ref()) {
            Ok(reader) => {
                app.settings_reader = Some(reader);
                app.load_catalog();
            }
            Err(error) => app.fail(error.to_string()),
        }
        app
    }

    pub fn view(&self) -> &EditorialView {
        &self.view
    }

    /// Text for the shell footer: what the surface says about its state.
    pub fn footer_status(&self) -> &str {
        &self.view.message
    }

    pub fn notify_on_player_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        if let Some(player) = &self.player {
            player.notify_on_change(notify);
        }
    }

    /// Rereads the active project. Cheap when nothing changed: the catalog is
    /// compared by its lightweight statistics before clips are loaded again.
    pub fn refresh(&mut self) {
        self.load_catalog();
    }

    pub fn has_player(&self) -> bool {
        self.player.is_some()
    }

    /// Delay until the next repaint that this surface needs, if any.
    pub fn next_repaint_delay(&self) -> Option<Duration> {
        if self.player.is_some() && (self.player_view.preparing || self.player_view.playing()) {
            return self.player_view.source_frame_interval();
        }
        if self.settings_result.is_some() {
            return Some(Duration::from_millis(100));
        }
        None
    }

    fn fail(&mut self, error: String) {
        self.view.loading = false;
        self.view.message = error;
    }

    fn load_catalog(&mut self) {
        if self.settings_result.is_some() {
            return;
        }
        let Some(reader) = self.settings_reader.clone() else {
            self.fail("Nema konfiguriranog citaca radnih postavki.".into());
            return;
        };
        let retained_workspace = self
            .work_plan
            .as_ref()
            .map(|plan| plan.settings.workspace_db_uri.clone());
        let retained_stats = self.catalog_stats.clone();
        self.view.loading = self.work_plan.is_none();
        let (send, receive) = mpsc::sync_channel(1);
        match std::thread::Builder::new()
            .name("editorial-catalog".into())
            .spawn(move || {
                let result = catalog::load(
                    &reader,
                    retained_workspace.as_deref(),
                    retained_stats.as_ref(),
                );
                let _ = send.send(result);
            }) {
            Ok(_) => self.settings_result = Some(receive),
            Err(_) => self.fail("Nije moguce pokrenuti citanje projektnog kataloga.".into()),
        }
    }

    /// Applies finished background work and the player state. Returns whether
    /// the view changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.poll_catalog();
        if let Some(player) = &self.player {
            let playback = player.view();
            if playback != self.player_view {
                if let Some(error) = &playback.error {
                    self.view.message = error.clone();
                    self.play_when_ready = false;
                }
                self.apply_player_view(playback);
                changed = true;
            }
        }
        if self.play_when_ready {
            if self.player_view.error.is_some() {
                self.play_when_ready = false;
            } else if self.player_view.can_start_playback() && !self.player_view.playing() {
                if let Some(player) = &self.player {
                    match player.send(Action::TogglePlayPause) {
                        Ok(()) => self.play_when_ready = false,
                        Err(error) if error == "Player se priprema." => {}
                        Err(error) => {
                            self.play_when_ready = false;
                            self.view.message = error;
                        }
                    }
                    changed = true;
                }
            } else if self.player_view.playing() {
                self.play_when_ready = false;
            }
        }
        changed
    }

    fn poll_catalog(&mut self) -> bool {
        let Some(receiver) = self.settings_result.as_ref() else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err("Citanje projektnog kataloga je prekinuto.".into()),
        };
        self.settings_result = None;
        self.view.loading = false;
        match result {
            Ok(loaded) => self.apply_catalog(loaded),
            Err(error) => self.fail(error),
        }
        true
    }

    fn apply_catalog(&mut self, loaded: catalog::LoadedCatalog) {
        let plan = loaded.plan;
        // A different project must never inherit the preceding one's clips or preview.
        if self.work_plan.as_ref().map(|old| &old.settings.project_id)
            != Some(&plan.settings.project_id)
        {
            self.view.clips.clear();
            self.view.preview_clip_id = None;
            self.stop_player();
            self.timeline_assets.reset();
            self.view.assets = SourceTimelineAssets::empty();
        }
        self.catalog_target = Some(loaded.target);
        self.catalog_stats = Some(loaded.stats);
        self.work_plan = Some(plan);
        if let Some(clips) = loaded.clips {
            self.view.clips = clips
                .into_iter()
                .map(|clip| EditorialClip {
                    clip_id: clip.clip_id,
                    name: clip.name,
                    duration_seconds: clip.duration_seconds,
                })
                .collect();
        }
        self.configure_assets();
        self.view.message = match self.view.clips.len() {
            0 => "Projekt nema uvezenih klipova.".to_string(),
            count => format!("{count} klipova"),
        };
    }

    fn configure_assets(&mut self) {
        let (Some(reader), Some(plan), Some(target)) = (
            self.settings_reader.as_ref(),
            self.work_plan.as_ref(),
            self.catalog_target.as_ref(),
        ) else {
            return;
        };
        match reader.local_workspace_dir(&plan.settings) {
            Ok(Some(project_dir)) => {
                self.timeline_assets.configure(TimelineAssetContext {
                    project_id: plan.settings.project_id.clone(),
                    reader: Arc::new(content::ArtifactReader {
                        target: target.clone(),
                        filmstrip_root_uri: plan.filmstrip_uri.clone(),
                        filmstrip_dir: project_dir.join("filmstrip"),
                    }),
                });
            }
            Ok(None) => {
                self.view.message = "Timeline artefakti nemaju lokalni filmstrip binding.".into();
            }
            Err(error) => self.view.message = error.to_string(),
        }
    }

    fn apply_player_view(&mut self, playback: PlayerView) {
        self.view.video_visible = playback.video_visible;
        self.view.monitor_message = playback.error.clone();
        self.view.monitor_frame = playback.picture.as_ref().map(|picture| MonitorFrame {
            session_id: picture.header.session_id.to_string(),
            generation: picture.header.output_generation,
            sequence: picture.header.sequence,
            width: picture.header.width as usize,
            height: picture.header.height as usize,
            rgba: picture.rgba.clone(),
        });
        self.view.timeline = qnc_player_timeline::projection_from_player_reply(playback.reply.as_ref());
        self.view.playing = playback.playing();
        self.player_view = playback;
    }

    fn stop_player(&mut self) {
        self.play_when_ready = false;
        if let Some(player) = &self.player {
            player.close();
        }
        self.player_view = PlayerView::default();
        self.view.monitor_frame = None;
        self.view.video_visible = false;
        self.view.monitor_message = None;
        self.view.timeline = qnc_timeline::TimelineProjection::default();
        self.view.playing = false;
    }

    /// Handles one intent from the form. Returns whether the view changed.
    pub fn dispatch(&mut self, intent: EditorialIntent) -> bool {
        match intent {
            EditorialIntent::PreviewClip(clip_id) => self.prepare_preview(clip_id),
            EditorialIntent::Action(action_id) => match action_id {
                action_ids::PLAY_PAUSE => self.toggle_play(),
                action_ids::STEP_BACK_FRAME => self.send(Action::Step(-1)),
                action_ids::STEP_FORWARD_FRAME => self.send(Action::Step(1)),
                _ => false,
            },
            EditorialIntent::Timeline(TimelineIntent::CueFrame(frame)) => {
                self.send(Action::Cue(frame))
            }
            EditorialIntent::Timeline(_) => false,
        }
    }

    fn toggle_play(&mut self) -> bool {
        let Some(player) = &self.player else {
            self.view.message = "Broadcast Player nije povezan.".into();
            return true;
        };
        let playback = player.view();
        if !playback.playing() && !playback.can_start_playback() && playback.error.is_none() {
            // Not ready yet: start as soon as the prepared session is.
            self.play_when_ready = true;
            return true;
        }
        self.send(Action::TogglePlayPause)
    }

    fn send(&mut self, action: Action) -> bool {
        let result = self
            .player
            .as_ref()
            .ok_or_else(|| "Broadcast Player nije povezan.".to_string())
            .and_then(|player| player.send(action));
        match result {
            Ok(()) => self.play_when_ready = false,
            Err(error) => self.view.message = error,
        }
        true
    }

    fn prepare_preview(&mut self, clip_id: String) -> bool {
        if self.view.preview_clip_id.as_deref() == Some(&clip_id)
            && self.player_view.error.is_none()
            && (self.player_view.preparing || self.player_view.reply.is_some())
        {
            return false;
        }
        if !self.view.clips.iter().any(|clip| clip.clip_id == clip_id) {
            self.view.message = "Klip nije pronadjen.".into();
            return true;
        }
        // Cut the old session even when the new clip cannot be prepared.
        self.stop_player();
        self.view.preview_clip_id = Some(clip_id.clone());
        self.focus_assets(&clip_id);

        let (Some(reader), Some(plan), Some(target)) = (
            self.settings_reader.clone(),
            self.work_plan.as_ref(),
            self.catalog_target.clone(),
        ) else {
            self.view.message = "Radne postavke projekta nisu ucitane.".into();
            return true;
        };
        let bindings = match &self.bindings {
            Ok(bindings) => bindings.clone(),
            Err(error) => {
                self.view.message = error.clone();
                return true;
            }
        };
        let workspace = plan.settings.workspace_db_uri.clone();
        if self.player.is_none() {
            match Player::new() {
                Ok(player) => self.player = Some(player),
                Err(error) => {
                    self.view.message = error;
                    return true;
                }
            }
        }
        let Some(player) = &self.player else {
            return true;
        };
        player.prepare(move || {
            let sources = bindings
                .iter()
                .map(|binding| {
                    if let Some(root) = &binding.file {
                        return Ok(SourceTransportBinding::local(
                            binding.uri.clone(),
                            root.clone(),
                        ));
                    }
                    SourceTransportBinding::network(
                        binding.uri.clone(),
                        binding
                            .endpoint
                            .clone()
                            .ok_or("Source endpoint missing.")?,
                        binding.token()?.ok_or("Source credential missing.")?,
                    )
                })
                .collect::<Result<Vec<_>, String>>()?;
            let executable = qnc_player_launcher::sibling_executable("qnc-broadcast-player")?;
            let input = InputReader::with_content_reader(
                reader,
                Arc::new(content::PlayerContent { target }),
            )
            .load(&workspace, &clip_id)
            .map_err(|error| error.to_string())?;
            qnc_player_launcher::prepare_launch(input, &sources, executable)
        });
        self.apply_player_view(player.view());
        self.view.message = String::new();
        true
    }

    /// Reads the published filmstrip and waveform of the clip (read-only).
    fn focus_assets(&mut self, clip_id: &str) {
        self.view.assets = self
            .timeline_assets
            .refresh_clip(clip_id)
            .unwrap_or_else(|_| SourceTimelineAssets::empty_for(clip_id));
    }
}

impl Drop for EditorialApplication {
    fn drop(&mut self) {
        self.stop_player();
        self.player = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(id: &str, name: &str) -> EditorialClip {
        EditorialClip {
            clip_id: id.into(),
            name: name.into(),
            duration_seconds: 10.0,
        }
    }

    #[test]
    fn starts_passive_and_empty() {
        let app = EditorialApplication::default();
        assert!(app.view().clips.is_empty());
        assert!(app.view().preview_clip_id.is_none());
        assert!(!app.has_player());
        assert!(app.next_repaint_delay().is_none());
    }

    #[test]
    fn missing_settings_end_in_a_controlled_error_not_a_default_project() {
        let root = std::env::temp_dir().join(format!("qnc_editorial_no_project_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let app = EditorialApplication::new(&root);
        // Either the reader could not be built, or the catalog load fails asynchronously:
        // in both cases no clips and no invented project.
        assert!(app.view().clips.is_empty());
        assert!(app.work_plan.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn unknown_clip_is_rejected_without_creating_a_player() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi")];
        assert!(app.dispatch(EditorialIntent::PreviewClip("missing".into())));
        assert_eq!(app.view().message, "Klip nije pronadjen.");
        assert!(!app.has_player());
        assert!(app.view().preview_clip_id.is_none());
    }

    #[test]
    fn preview_without_loaded_settings_keeps_the_choice_but_reports_it() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi")];
        assert!(app.dispatch(EditorialIntent::PreviewClip("a".into())));
        assert_eq!(app.view().preview_clip_id.as_deref(), Some("a"));
        assert_eq!(app.view().message, "Radne postavke projekta nisu ucitane.");
        assert!(!app.has_player());
    }

    #[test]
    fn transport_without_a_player_says_so() {
        let mut app = EditorialApplication::default();
        for action in [
            action_ids::PLAY_PAUSE,
            action_ids::STEP_BACK_FRAME,
            action_ids::STEP_FORWARD_FRAME,
        ] {
            app.view.message.clear();
            app.dispatch(EditorialIntent::Action(action));
            assert_eq!(app.view().message, "Broadcast Player nije povezan.");
        }
        app.view.message.clear();
        app.dispatch(EditorialIntent::Timeline(TimelineIntent::CueFrame(10)));
        assert_eq!(app.view().message, "Broadcast Player nije povezan.");
    }

    #[test]
    fn current_clip_label_follows_the_preview_choice() {
        let mut app = EditorialApplication::default();
        app.view.clips = vec![clip("a", "Prvi"), clip("b", "Drugi")];
        assert_eq!(app.view().current_clip_label(), None);
        app.view.preview_clip_id = Some("b".into());
        assert_eq!(app.view().current_clip_label(), Some("Drugi"));
    }
}

//! Source preview session for one clip: it prepares the Broadcast Player from
//! the stored media record, keeps the confirmed monitor picture and timeline
//! projection, and reads the published filmstrip and waveform artifacts. It is a
//! public component: any form or application can embed it, and it knows none of
//! them. Read-only: no scan, no probe, no write to any database.
//!
//! The chain follows AGENTS.md section 4.1: work settings of the active project,
//! project content (public views), media record, player launch.

mod content;

use std::{sync::Arc, time::Duration};

use qnc_content_read::ContentReader;
use qnc_player_client::{Action, Player, View as PlayerView};
use qnc_player_input::{InputReader, PlayerContentRead};
use qnc_player_launcher::SourceTransportBinding;
use qnc_source_bindings::{SourceBinding, TransportBindings};
use qnc_timeline::{TimelineIntent, TimelineProjection};
use qnc_timeline_assets::{
    SourceTimelineAssets, TimelineArtifactRead, TimelineAssetContext, TimelineAssetReader,
};
use qnc_work_settings::{SettingsReader, WorkSettings};

pub const MODULE_ID: &str = "qnc.module.source-preview";
pub const VERSION: &str = "0.1.0";

/// Confirmed monitor picture (RGBA), as delivered by the player.
#[derive(Debug, Clone)]
pub struct MonitorFrame {
    pub session_id: String,
    pub generation: u64,
    pub sequence: u64,
    pub width: usize,
    pub height: usize,
    pub rgba: Arc<[u8]>,
}

/// Everything a form needs to paint the preview.
#[derive(Clone)]
pub struct PreviewView {
    pub clip_id: Option<String>,
    pub monitor_frame: Option<MonitorFrame>,
    pub video_visible: bool,
    pub monitor_message: Option<String>,
    pub timeline: TimelineProjection,
    pub assets: SourceTimelineAssets,
    pub playing: bool,
    /// Controlled error text of the last failed command; empty otherwise.
    pub message: String,
}

impl Default for PreviewView {
    fn default() -> Self {
        Self {
            clip_id: None,
            monitor_frame: None,
            video_visible: false,
            monitor_message: None,
            timeline: TimelineProjection::default(),
            assets: SourceTimelineAssets::empty(),
            playing: false,
            message: String::new(),
        }
    }
}

/// The project a preview works in. Where the clips and artifacts come from is decided
/// by the readers it carries, so any form over any content database can use it.
#[derive(Clone)]
pub struct PreviewContext {
    reader: SettingsReader,
    settings: WorkSettings,
    sources: Vec<SourceBinding>,
    player_content: Arc<dyn PlayerContentRead>,
    artifacts: Option<Arc<dyn TimelineArtifactRead>>,
    notice: Option<String>,
}

impl PreviewContext {
    /// Any content database: the caller supplies the readers.
    pub fn with_readers(
        reader: SettingsReader,
        settings: WorkSettings,
        sources: Vec<SourceBinding>,
        player_content: Arc<dyn PlayerContentRead>,
        artifacts: Option<Arc<dyn TimelineArtifactRead>>,
    ) -> Self {
        Self {
            reader,
            settings,
            sources,
            player_content,
            artifacts,
            notice: None,
        }
    }

    /// The project content views (`qnc-content-read`).
    pub fn new(
        reader: SettingsReader,
        settings: WorkSettings,
        content: ContentReader,
        bindings: TransportBindings,
    ) -> Self {
        let (artifacts, notice) = match reader.local_workspace_dir(&settings) {
            Ok(Some(project_dir)) => {
                let artifacts: Arc<dyn TimelineArtifactRead> = Arc::new(content::ArtifactReader {
                    content: content.clone(),
                    filmstrip_root_uri: format!(
                        "{}/filmstrip",
                        settings.output_root_uri.trim_end_matches('/')
                    ),
                    filmstrip_dir: project_dir.join("filmstrip"),
                });
                (Some(artifacts), None)
            }
            Ok(None) => (
                None,
                Some("Timeline artefakti nemaju lokalni filmstrip binding.".to_string()),
            ),
            Err(error) => (None, Some(error.to_string())),
        };
        Self {
            reader,
            settings,
            sources: bindings.sources,
            player_content: Arc::new(content::PlayerContent {
                content,
                records: bindings.media_records,
            }),
            artifacts,
            notice,
        }
    }

    pub fn project_id(&self) -> &str {
        &self.settings.project_id
    }
}

pub struct SourcePreview {
    view: PreviewView,
    context: Option<PreviewContext>,
    player: Option<Player>,
    player_view: PlayerView,
    timeline_assets: TimelineAssetReader,
    play_when_ready: bool,
    marker_at: Option<std::time::Instant>,
}

impl Default for SourcePreview {
    fn default() -> Self {
        Self {
            view: PreviewView::default(),
            context: None,
            player: None,
            player_view: PlayerView::default(),
            timeline_assets: TimelineAssetReader::default(),
            play_when_ready: false,
            marker_at: None,
        }
    }
}

impl SourcePreview {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &PreviewView {
        &self.view
    }

    /// The player state as the player reports it, for forms that paint it themselves.
    pub fn player_view(&self) -> &PlayerView {
        &self.player_view
    }

    /// A play command is waiting for the prepared session.
    pub fn play_when_ready(&self) -> bool {
        self.play_when_ready
    }

    /// Uses a player the caller already made instead of creating one on `open`.
    pub fn attach_player(&mut self, player: Player) {
        self.player = Some(player);
    }

    /// The message of the last failed command; it is handed over once.
    pub fn take_message(&mut self) -> String {
        std::mem::take(&mut self.view.message)
    }

    pub fn has_player(&self) -> bool {
        self.player.is_some()
    }

    pub fn notify_on_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        if let Some(player) = &self.player {
            player.notify_on_change(notify);
        }
    }

    /// Delay until the next repaint this session needs, if any.
    pub fn next_repaint_delay(&self) -> Option<Duration> {
        if self.player.is_some() && (self.player_view.preparing || self.player_view.playing()) {
            return self.player_view.source_frame_interval();
        }
        None
    }

    /// Sets the project. A different project never inherits the previous clip.
    pub fn configure(&mut self, context: PreviewContext) {
        let changed = self
            .context
            .as_ref()
            .map(PreviewContext::project_id)
            .is_some_and(|old| old != context.project_id());
        if changed {
            self.close();
            self.timeline_assets.reset();
        }
        if let Some(artifacts) = &context.artifacts {
            self.timeline_assets.configure(TimelineAssetContext {
                project_id: context.settings.project_id.clone(),
                reader: artifacts.clone(),
            });
        }
        if let Some(notice) = &context.notice {
            self.view.message = notice.clone();
        }
        self.context = Some(context);
    }

    /// Cuts the current session and clears everything shown.
    pub fn close(&mut self) {
        self.play_when_ready = false;
        self.release_marker();
        if let Some(player) = &self.player {
            player.close();
        }
        self.player_view = PlayerView::default();
        self.view = PreviewView {
            assets: SourceTimelineAssets::empty(),
            ..PreviewView::default()
        };
    }

    /// Prepares the preview of `clip_id`: cuts the old session first, even when
    /// the new clip cannot be prepared. Returns whether the view changed.
    pub fn open(&mut self, clip_id: &str) -> bool {
        if self.view.clip_id.as_deref() == Some(clip_id)
            && self.player_view.error.is_none()
            && (self.player_view.preparing || self.player_view.reply.is_some())
        {
            return false;
        }
        self.close();
        self.view.clip_id = Some(clip_id.to_string());
        let Some(context) = self.context.clone() else {
            self.view.message = "Radne postavke projekta nisu ucitane.".into();
            return true;
        };
        // Published artifacts are read again on every choice: they may have
        // appeared since the clip was last shown.
        self.view.assets = self
            .timeline_assets
            .refresh_clip(clip_id)
            .unwrap_or_else(|_| SourceTimelineAssets::empty_for(clip_id));

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
        let clip_id = clip_id.to_string();
        player.prepare(move || {
            let sources = context
                .sources
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
                context.reader.clone(),
                context.player_content.clone(),
            )
            .load(&context.settings.workspace_db_uri, &clip_id)
            .map_err(|error| error.to_string())?;
            qnc_player_launcher::prepare_launch(input, &sources, executable)
        });
        self.apply_player_view(player.view());
        true
    }

    pub fn toggle_play(&mut self) -> bool {
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

    pub fn step(&mut self, frames: i64) -> bool {
        self.send(Action::Step(frames))
    }

    pub fn cue(&mut self, frame: u64) -> bool {
        self.send(Action::Cue(frame))
    }

    /// Handles the intent of a passive timeline.
    pub fn timeline_intent(&mut self, intent: &TimelineIntent) -> bool {
        match intent {
            TimelineIntent::CueFrame(frame) => self.cue(*frame),
            _ => false,
        }
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

    /// Applies the player state. Returns whether the view changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
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
        self.refresh_marker();
        changed
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
        self.view.timeline =
            qnc_player_timeline::projection_from_player_reply(playback.reply.as_ref());
        self.view.playing = playback.playing();
        self.player_view = playback;
    }
}

impl Drop for SourcePreview {
    fn drop(&mut self) {
        self.close();
        self.player = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_passive_and_empty() {
        let preview = SourcePreview::new();
        assert!(preview.view().clip_id.is_none());
        assert!(!preview.has_player());
        assert!(preview.next_repaint_delay().is_none());
    }

    #[test]
    fn opening_without_a_project_keeps_the_choice_and_reports_it() {
        let mut preview = SourcePreview::new();
        assert!(preview.open("clip-a"));
        assert_eq!(preview.view().clip_id.as_deref(), Some("clip-a"));
        assert_eq!(preview.view().message, "Radne postavke projekta nisu ucitane.");
        assert!(!preview.has_player());
    }

    #[test]
    fn transport_without_a_player_says_so() {
        let mut preview = SourcePreview::new();
        assert!(preview.toggle_play());
        assert_eq!(preview.view().message, "Broadcast Player nije povezan.");
        for change in [preview.step(1), preview.step(-1), preview.cue(10)] {
            assert!(change);
        }
        assert_eq!(preview.view().message, "Broadcast Player nije povezan.");
    }

    #[test]
    fn only_a_cue_timeline_intent_is_acted_on() {
        let mut preview = SourcePreview::new();
        assert!(!preview.timeline_intent(&TimelineIntent::None));
        assert!(preview.timeline_intent(&TimelineIntent::CueFrame(5)));
    }

    #[test]
    fn close_clears_everything_shown() {
        let mut preview = SourcePreview::new();
        preview.open("clip-a");
        preview.close();
        assert!(preview.view().clip_id.is_none());
        assert!(preview.view().message.is_empty());
    }
}

impl std::fmt::Debug for SourcePreview {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourcePreview")
            .field("clip_id", &self.view.clip_id)
            .field("has_player", &self.player.is_some())
            .field("play_when_ready", &self.play_when_ready)
            .finish()
    }
}

impl SourcePreview {
    /// Tells background processes on this machine that a player works, so they can wait.
    fn refresh_marker(&mut self) {
        let working = self.player.is_some()
            && (self.player_view.preparing || self.player_view.playing() || self.play_when_ready);
        if !working {
            self.release_marker();
        } else if self
            .marker_at
            .is_none_or(|at| at.elapsed() >= qnc_playback_marker::REFRESH_EVERY)
        {
            qnc_playback_marker::touch();
            self.marker_at = Some(std::time::Instant::now());
        }
    }

    fn release_marker(&mut self) {
        if self.marker_at.take().is_some() {
            qnc_playback_marker::clear();
        }
    }
}

//! The preview panel of Ingest: the clip chosen in the pool, what the form shows of the
//! preview and of the one source timeline, and the keys of that timeline.

use crate::preview_context;
use qnc_ingest_select::SelectionConfig;
use qnc_ingest_store::content::ContentTarget;
use qnc_source_preview::{MonitorState, SourcePreview};
use qnc_timeline_assets::SourceTimelineAssets;
use qnc_work_settings::{SettingsReader, WorkSettings};

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewView {
    pub playback: qnc_player_client::View,
    pub timeline: qnc_timeline::TimelineProjection,
    pub assets: SourceTimelineAssets,
    /// What the monitor shows (the monitor panel's input).
    pub monitor: MonitorState,
}

/// What the form shows of the preview and of the one source timeline.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub playback: qnc_player_client::View,
    pub monitor: MonitorState,
    pub timeline: qnc_timeline::TimelineProjection,
    pub assets: SourceTimelineAssets,
    pub source_timeline: qnc_source_timeline::SourceTimelineView,
}

/// How choosing a clip went.
#[derive(Debug, Clone, PartialEq)]
pub enum Chosen {
    /// The session is already on this clip: only its artifacts are read again.
    Refreshed,
    /// The old session is closed; the clip is not saved in the database yet.
    NotSaved,
    /// The old session is closed; the project is not loaded, nothing is opened.
    Waiting,
    Opened,
    Failed(String),
}

/// The project a clip is opened from (`None` while the project is not loaded).
pub struct OpenFrom<'a> {
    pub reader: SettingsReader,
    pub settings: WorkSettings,
    pub config: &'a SelectionConfig,
    pub target: ContentTarget,
}

#[derive(Debug, Default)]
pub struct IngestPreviewSource {
    preview: SourcePreview,
    /// The one source timeline every form embeds (piece `qnc-source-timeline`).
    source_timeline: qnc_source_timeline::SourceTimeline,
    chosen: Option<String>,
}

impl IngestPreviewSource {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn configure(
        &mut self,
        reader: SettingsReader,
        settings: WorkSettings,
        config: &SelectionConfig,
        target: ContentTarget,
    ) {
        self.preview
            .configure(preview_context(reader, settings, config, target));
    }

    /// The clip chosen in the pool: the old session is cut first, even when the new clip
    /// cannot be opened; the same clip already preparing or prepared only reads its artifacts again.
    pub fn choose(&mut self, clip_id: &str, saved: bool, from: Option<OpenFrom<'_>>) -> Chosen {
        let player = self.preview.player_view();
        if self.chosen.as_deref() == Some(clip_id) && player.error.is_none() && (player.preparing || player.reply.is_some()) {
            self.preview.refresh_assets();
            return Chosen::Refreshed;
        }
        self.preview.close();
        self.chosen = Some(clip_id.to_string());
        if !saved {
            return Chosen::NotSaved;
        }
        let Some(from) = from else { return Chosen::Waiting };
        self.configure(from.reader, from.settings, from.config, from.target);
        match self.open_saved_clip(clip_id, true) {
            Ok(()) => Chosen::Opened,
            Err(error) => Chosen::Failed(error),
        }
    }

    /// The preview and the source timeline of `clip` as the form shows them; the IN/OUT of
    /// `previous` stay, and a clip without its filmstrip shows its poster in the strip.
    pub fn shown<'a>(
        &mut self,
        clip: Option<&str>,
        previous: &qnc_timeline::TimelineProjection,
        posters: impl IntoIterator<Item = (&'a str, Option<&'a qnc_image_assets::RgbaImage>)>,
        catalog: Option<&ContentTarget>,
    ) -> Shown {
        let view = self.view();
        let clip_id = clip.unwrap_or_default();
        let assets = if view.assets.clip_id == clip_id { view.assets } else { SourceTimelineAssets::empty_for(clip_id) };
        Shown {
            playback: view.playback,
            monitor: view.monitor,
            timeline: view.timeline.preserving_source_marks_from(previous),
            assets: qnc_timeline_assets::with_clip_poster(assets, posters),
            source_timeline: self.source_timeline.sync(clip, catalog, &mut self.preview).clone(),
        }
    }

    /// The A1/A2 lanes and marks of the source timeline (Ctrl+1/2, a taken lane, a channel click);
    /// `None`: not the source timeline's action.
    pub fn route_input(&mut self, action_id: &str, value: Option<i64>, timeline: &mut qnc_timeline::TimelineProjection) -> Option<bool> {
        self.source_timeline.route_input(action_id, value, &mut self.preview, timeline)
    }

    /// A key of the source timeline or a click on it (`timeline_cue_frame` with its frame);
    /// `Ok(None)`: nothing to do. Without a player only Escape still lets go of what is taken.
    pub fn route(&mut self, action_id: &str, frame: Option<i64>, timeline: &mut qnc_timeline::TimelineProjection) -> Result<Option<bool>, String> {
        if !self.preview.has_player() && action_id != "clear_focus" {
            return Err(qnc_ui_text::text("player.broadcast_player_connected").into());
        }
        if action_id == "timeline_cue_frame" {
            return match frame {
                Some(frame) if frame >= 0 => {
                    let cue = qnc_timeline::TimelineIntent::CueFrame(frame as u64);
                    Ok(Some(self.source_timeline.cue(&cue, &mut self.preview)))
                }
                _ => Err(qnc_ui_text::text("ingest.msg.invalid_frame").into()),
            };
        }
        Ok(self.source_timeline.route(action_id, &mut self.preview, timeline))
    }

    /// What the source timeline says (its message after a key).
    pub fn timeline_message(&self) -> String {
        self.source_timeline.view().message.clone()
    }

    pub fn open_saved_clip(&mut self, clip_id: &str, saved: bool) -> Result<(), String> {
        self.preview.close();
        if !saved {
            return Err(qnc_ui_text::text("ingest.msg.clip_not_saved").into());
        }
        self.preview.open(clip_id);
        let message = self.preview.take_message();
        if message.is_empty() {
            Ok(())
        } else {
            Err(message)
        }
    }

    pub fn close(&mut self) {
        self.preview.close();
    }

    pub fn poll(&mut self, pointed: Option<&str>) -> bool {
        self.preview.poll_pointed(pointed)
    }

    pub fn refresh_assets(&mut self) -> bool {
        self.preview.refresh_assets()
    }

    pub fn player_view(&self) -> &qnc_player_client::View {
        self.preview.player_view()
    }

    pub fn view(&self) -> PreviewView {
        let view = self.preview.view();
        let playback = self.preview.player_view().clone();
        PreviewView {
            timeline: view.timeline.clone(),
            assets: view.assets.clone(),
            playback,
            monitor: view.monitor(),
        }
    }

    pub fn take_message(&mut self) -> String {
        self.preview.take_message()
    }

    /// The universal source preview itself, for the pieces every form shares (the keys of
    /// the source timeline, `qnc-source-panel`).
    pub fn source_preview_mut(&mut self) -> &mut SourcePreview {
        &mut self.preview
    }

    pub fn has_player(&self) -> bool {
        self.preview.has_player()
    }

    pub fn play_when_ready(&self) -> bool {
        self.preview.play_when_ready()
    }

    pub fn toggle_play(&mut self) -> bool {
        self.preview.toggle_play()
    }

    pub fn step(&mut self, frames: i64) -> bool {
        self.preview.step(frames)
    }

    pub fn cue(&mut self, frame: u64) -> bool {
        self.preview.cue(frame)
    }

    pub fn notify_on_change(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.preview.notify_on_change(notify);
    }

    pub fn attach_player(&mut self, player: qnc_player_client::Player) {
        self.preview.attach_player(player);
    }
}


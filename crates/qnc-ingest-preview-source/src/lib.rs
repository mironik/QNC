//! The Ingest content database as one source adapter for the universal source preview.
//!
//! The preview itself is the neutral `qnc-source-preview`; this crate only says where
//! Ingest keeps its clips and which sources carry the media. Other applications must
//! provide their own source adapter over the same universal preview.

use qnc_ingest_select::SelectionConfig;
use qnc_ingest_store::content::{Access, ContentTarget};
use qnc_player_input::{PlayerClipRecord, PlayerContentRead};
use qnc_source_bindings::SourceBinding;
use qnc_source_preview::{PreviewContext, SourcePreview};
use qnc_timeline_assets::SourceTimelineAssets;
use qnc_work_settings::{SettingsReader, WorkSettings};
use std::sync::Arc;

pub const MODULE_ID: &str = "qnc.module.ingest-preview-source";
pub const VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewCommand {
    TogglePlay,
    Step(i64),
    Cue(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewCommandOutcome {
    pub started_playback: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreviewView {
    pub playback: qnc_player_client::View,
    pub timeline: qnc_timeline::TimelineProjection,
    pub assets: SourceTimelineAssets,
}

#[derive(Clone)]
struct IngestPlayerContent {
    target: ContentTarget,
}

impl PlayerContentRead for IngestPlayerContent {
    fn read_clip(&self, clip_id: &str) -> Result<Option<PlayerClipRecord>, String> {
        let stored = self.target.open(Access::ReadOnly)?.read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
        }))
    }
}

pub fn preview_context(
    reader: SettingsReader,
    settings: WorkSettings,
    config: &SelectionConfig,
    target: ContentTarget,
) -> PreviewContext {
    let sources = config
        .sources
        .iter()
        .map(|source| SourceBinding {
            uri: source.location.uri.clone(),
            file: source.location.file.clone(),
            endpoint: source.location.endpoint.clone(),
            token_env: source.location.token_env.clone(),
        })
        .collect();
    let artifacts = qnc_content_store::ContentTarget::for_project(&reader, &settings)
        .ok()
        .and_then(|artifact_target| {
            qnc_content_artifacts::timeline_artifact_reader(&reader, &settings, artifact_target)
                .ok()
        });
    PreviewContext::with_readers(
        reader,
        settings,
        sources,
        Arc::new(IngestPlayerContent { target }),
        artifacts,
    )
}

#[derive(Debug, Default)]
pub struct IngestPreviewSource {
    preview: SourcePreview,
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

    pub fn open_saved_clip(&mut self, clip_id: &str, saved: bool) -> Result<(), String> {
        self.preview.close();
        if !saved {
            return Err("Klip jos nije spremljen u bazu.".into());
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

    pub fn poll(&mut self) -> bool {
        self.preview.poll()
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
        }
    }

    pub fn take_message(&mut self) -> String {
        self.preview.take_message()
    }

    pub fn has_player(&self) -> bool {
        self.preview.has_player()
    }

    pub fn play_when_ready(&self) -> bool {
        self.preview.play_when_ready()
    }

    pub fn send_command(
        &mut self,
        command: PreviewCommand,
    ) -> Result<PreviewCommandOutcome, String> {
        if !self.preview.has_player() {
            return Err("Broadcast Player nije povezan.".into());
        }
        let started_playback = match command {
            PreviewCommand::TogglePlay => {
                let was_playing = self.preview.player_view().playing();
                self.preview.toggle_play();
                !was_playing
            }
            PreviewCommand::Step(frames) => {
                self.preview.step(frames);
                false
            }
            PreviewCommand::Cue(frame) => {
                self.preview.cue(frame);
                false
            }
        };
        Ok(PreviewCommandOutcome {
            started_playback,
            message: self.preview.take_message(),
        })
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

//! Saved media -> player input. No decoder, source I/O, probe, UI or database writes.
use qnc_frame_timebase::FrameTimebase;
use qnc_media_metadata::{FrameRateMode, MediaRepresentation, StreamDetails};
use qnc_media_records::{Phase, Snapshot};
pub use qnc_work_settings::PlaybackInput;
use qnc_work_settings::{SettingsReader, WorkSettings};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub type Result<T> = std::result::Result<T, InputError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "code",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum InputError {
    Settings(String),
    ChangedSettings,
    WrongWorkspace,
    Database(String),
    MissingClip,
    InvalidRecord(String),
    NotFinal,
    MissingProxy,
    IncompleteMedia(Vec<String>),
    UnsupportedMedia(String),
    InvalidDescriptor,
}
impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "player input: {self:?}")
    }
}
impl std::error::Error for InputError {}

fn playback_input(settings: &WorkSettings) -> Result<PlaybackInput> {
    settings
        .validate()
        .map_err(|e| InputError::Settings(e.to_string()))?;
    settings
        .playback_input()
        .map_err(|e| InputError::Settings(e.to_string()))
}

fn choose(mode: PlaybackInput, snapshot: &Snapshot) -> Result<Representation> {
    match (mode, snapshot.metadata.proxy.is_some()) {
        (PlaybackInput::Original, _) | (PlaybackInput::ProxyIfAvailable, false) => {
            Ok(Representation::Original)
        }
        (PlaybackInput::Proxy, false) => Err(InputError::MissingProxy),
        (_, true) => Ok(Representation::Proxy),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Representation {
    Original,
    Proxy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerClipRecord {
    pub name: String,
    pub snapshot: Snapshot,
    pub imported_media_uri: Option<String>,
}

impl PlayerClipRecord {
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() || self.name.len() > 1024 {
            return Err(InputError::InvalidRecord(
                "Neispravan naziv klipa u player ulazu.".into(),
            ));
        }
        self.snapshot
            .validate()
            .map_err(|e| InputError::InvalidRecord(e.to_string()))
    }
}

pub trait PlayerContentRead: Send + Sync {
    fn read_clip(&self, clip_id: &str) -> std::result::Result<Option<PlayerClipRecord>, String>;
}

trait PlayerClipSource {
    fn snapshot(&self) -> &Snapshot;
    fn imported_media_uri(&self) -> Option<&String>;
    fn validate_clip(&self) -> Result<()>;
}

impl PlayerClipSource for PlayerClipRecord {
    fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    fn imported_media_uri(&self) -> Option<&String> {
        self.imported_media_uri.as_ref()
    }

    fn validate_clip(&self) -> Result<()> {
        self.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoInput {
    pub stream_index: u32,
    pub timebase: FrameTimebase,
    pub duration_frames: u64,
    /// Unknown/variable is not silently converted to constant frame rate.
    pub frame_rate_mode: FrameRateMode,
}

/// Native encoded stream index and zero-based channel within that stream.
/// This is an inventory, not a downmix or a claim that proxy channels match original channels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioChannel {
    pub stream_index: u32,
    pub channel_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamLayout {
    pub video: Option<VideoInput>,
    /// Always addresses the original representation, independent of the video choice.
    pub audio_channels: Vec<AudioChannel>,
}

/// Output format read from the active project's public DB record, not source metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectAudio {
    pub channels: u16,
    pub sample_rate_hz: u32,
}
impl ProjectAudio {
    fn read(settings: &WorkSettings) -> Result<Self> {
        let channels = settings
            .audio
            .get("channels")
            .and_then(|v| v.as_u64())
            .and_then(|v| u16::try_from(v).ok());
        let sample_rate_hz = settings
            .audio
            .get("sample_rate")
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok());
        let value = Self {
            channels: channels.ok_or_else(|| {
                InputError::Settings("Missing or invalid audio.channels in project DB.".into())
            })?,
            sample_rate_hz: sample_rate_hz.ok_or_else(|| {
                InputError::Settings("Missing or invalid audio.sample_rate in project DB.".into())
            })?,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if !(1..=64).contains(&self.channels) || !(8000..=384000).contains(&self.sample_rate_hz) {
            return Err(InputError::Settings(
                "Project audio format is outside supported limits.".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedInput {
    pub contract_version: String,
    pub workspace_db_uri: String,
    pub playback_input: PlaybackInput,
    pub project_audio: ProjectAudio,
    /// Project-selected video representation. Audio retains the original channels.
    pub representation: Representation,
    /// Preserve the full saved codec/container/timing/color/audio evidence, not only core AV fields.
    pub snapshot: Snapshot,
    pub layout: StreamLayout,
}
impl PreparedInput {
    /// Saved representation selected for the picture, not the audio inventory.
    pub fn media(&self) -> Result<&MediaRepresentation> {
        match self.representation {
            Representation::Original => Ok(&self.snapshot.metadata.original),
            Representation::Proxy => self
                .snapshot
                .metadata
                .proxy
                .as_ref()
                .ok_or(InputError::MissingProxy),
        }
    }

    pub fn audio_media(&self) -> &MediaRepresentation {
        &self.snapshot.metadata.original
    }

    /// A transport receiver must bind this descriptor to its expected DB-derived input identity.
    pub fn validate_for(&self, workspace_uri: &str, clip_id: &str) -> Result<()> {
        if self.contract_version != VERSION || self.snapshot.metadata.clip_id != clip_id {
            return Err(InputError::InvalidDescriptor);
        }
        validate_workspace(&self.workspace_db_uri)?;
        if self.workspace_db_uri != workspace_uri {
            return Err(InputError::WrongWorkspace);
        }
        validate_snapshot(&self.snapshot)?;
        self.project_audio.validate()?;
        if self.representation != choose(self.playback_input, &self.snapshot)? {
            return Err(InputError::InvalidDescriptor);
        }
        if self.layout != layout(&self.snapshot, self.representation)? {
            return Err(InputError::InvalidDescriptor);
        }
        Ok(())
    }
}

/// No retained active project, catalog or playback state. Every load reads the public DB contract.
pub struct InputReader {
    settings: SettingsReader,
    content_reader: Option<Arc<dyn PlayerContentRead>>,
}
impl InputReader {
    pub fn new(settings: SettingsReader) -> Self {
        Self {
            settings,
            content_reader: None,
        }
    }

    pub fn with_content_reader(
        settings: SettingsReader,
        content_reader: Arc<dyn PlayerContentRead>,
    ) -> Self {
        Self {
            settings,
            content_reader: Some(content_reader),
        }
    }

    /// `workspace_uri` comes from the caller's existing read-only work-settings snapshot,
    /// so a stale clip click cannot silently target a different active project.
    pub fn load(&self, workspace_uri: &str, clip_id: &str) -> Result<PreparedInput> {
        validate_workspace(workspace_uri)?;
        qnc_media_records::valid_id(clip_id)
            .map_err(|e| InputError::InvalidRecord(e.to_string()))?;
        let settings = self
            .settings
            .read()
            .map_err(|e| InputError::Settings(e.to_string()))?;
        if settings.workspace_db_uri != workspace_uri {
            return Err(InputError::WrongWorkspace);
        }
        playback_input(&settings)?;
        let stored = self
            .content_reader
            .as_ref()
            .ok_or_else(|| InputError::Database("Nedostaje player content read port.".into()))?
            .read_clip(clip_id)
            .map_err(InputError::Database)?
            .ok_or(InputError::MissingClip)?;
        let prepared = prepare(&settings, &stored)?;
        let current = self
            .settings
            .read()
            .map_err(|e| InputError::Settings(e.to_string()))?;
        if current != settings {
            return Err(InputError::ChangedSettings);
        }
        prepared.validate_for(workspace_uri, clip_id)?;
        Ok(prepared)
    }
}

fn prepare(settings: &WorkSettings, stored: &impl PlayerClipSource) -> Result<PreparedInput> {
    let mode = playback_input(settings)?;
    stored.validate_clip()?;
    let snapshot = stored.snapshot();
    validate_snapshot(snapshot)?;
    if stored.imported_media_uri().is_some_and(|uri| {
        uri != &snapshot.binding.original_uri && Some(uri) != snapshot.binding.proxy_uri.as_ref()
    }) {
        return Err(InputError::UnsupportedMedia(
            "Imported media URI has no saved original/proxy representation binding.".into(),
        ));
    }
    let representation = choose(mode, snapshot)?;
    Ok(PreparedInput {
        contract_version: VERSION.into(),
        workspace_db_uri: settings.workspace_db_uri.clone(),
        playback_input: mode,
        project_audio: ProjectAudio::read(settings)?,
        representation,
        layout: layout(snapshot, representation)?,
        snapshot: snapshot.clone(),
    })
}

fn validate_workspace(uri: &str) -> Result<()> {
    let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|_| InputError::WrongWorkspace)?;
    if parsed.resource_kind != "db" {
        return Err(InputError::WrongWorkspace);
    }
    let id = parsed
        .resource_id
        .strip_prefix("project_workspace/")
        .ok_or(InputError::WrongWorkspace)?;
    qnc_media_records::valid_id(id).map_err(|_| InputError::WrongWorkspace)
}

fn validate_snapshot(snapshot: &Snapshot) -> Result<()> {
    snapshot
        .validate()
        .map_err(|e| InputError::InvalidRecord(e.to_string()))?;
    if snapshot.phase != Phase::Final {
        return Err(InputError::NotFinal);
    }
    Ok(())
}

fn layout(snapshot: &Snapshot, representation: Representation) -> Result<StreamLayout> {
    let original = representation_layout(snapshot, Representation::Original)?;
    if representation == Representation::Original {
        return Ok(original);
    }
    let mut selected = representation_layout(snapshot, representation)?;
    match (&original.video, &selected.video) {
        (Some(a), Some(b))
            if a.timebase == b.timebase
                && a.duration_frames == b.duration_frames
                && a.frame_rate_mode != FrameRateMode::Variable
                && b.frame_rate_mode != FrameRateMode::Variable =>
        {
            ()
        }
        _ => {
            return Err(InputError::UnsupportedMedia(
                "Proxy picture and original audio require matching saved video timing.".into(),
            ));
        }
    }
    selected.audio_channels = original.audio_channels;
    Ok(selected)
}

fn representation_layout(
    snapshot: &Snapshot,
    representation: Representation,
) -> Result<StreamLayout> {
    let (prefix, media) = match representation {
        Representation::Original => ("original", &snapshot.metadata.original),
        Representation::Proxy => (
            "proxy",
            snapshot
                .metadata
                .proxy
                .as_ref()
                .ok_or(InputError::MissingProxy)?,
        ),
    };
    let missing: Vec<_> = snapshot
        .report
        .issues
        .iter()
        .filter(|i| i.path == prefix || i.path.starts_with(&format!("{prefix}.")))
        .map(|i| i.path.clone())
        .collect();
    if !missing.is_empty() {
        return Err(InputError::IncompleteMedia(missing));
    }
    let mut result = StreamLayout {
        video: None,
        audio_channels: Vec::new(),
    };
    let mut streams: Vec<_> = media.streams.iter().collect();
    streams.sort_by_key(|s| s.index.as_ref().map(|i| i.value));
    for stream in streams {
        let index = stream
            .index
            .as_ref()
            .ok_or(InputError::InvalidDescriptor)?
            .value;
        match &stream.details {
            StreamDetails::Video(video) => {
                if result.video.is_some() {
                    return Err(InputError::UnsupportedMedia(
                        "Multiple video streams require an explicit stream selection contract."
                            .into(),
                    ));
                }
                let duration_frames = video.exact_frame_count().ok_or_else(|| {
                    InputError::UnsupportedMedia(
                        "Frame-accurate playback requires a saved exact frame count.".into(),
                    )
                })?;
                result.video = Some(VideoInput {
                    stream_index: index,
                    timebase: video
                        .frame_rate
                        .as_ref()
                        .ok_or(InputError::InvalidDescriptor)?
                        .value,
                    duration_frames,
                    frame_rate_mode: video
                        .frame_rate_mode
                        .as_ref()
                        .ok_or(InputError::InvalidDescriptor)?
                        .value,
                });
            }
            StreamDetails::Audio(audio) => {
                let count = audio
                    .channels
                    .as_ref()
                    .ok_or(InputError::InvalidDescriptor)?
                    .value;
                if result.audio_channels.len() as u64 + u64::from(count) > u64::from(u16::MAX) {
                    return Err(InputError::UnsupportedMedia(
                        "Audio channel inventory exceeds the player contract limit.".into(),
                    ));
                }
                result
                    .audio_channels
                    .extend((0..count).map(|channel_index| AudioChannel {
                        stream_index: index,
                        channel_index,
                    }));
            }
            StreamDetails::Other { .. } => (),
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

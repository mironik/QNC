//! The flat program playlist contract (v5 `qnc-service-contracts/program_playlist.rs`).
//!
//! Frames only. Media is named by its QNC URI, never by a raw path; the facts
//! come from the saved media record, never from a new probe.

use qnc_frame_timebase::FrameTimebase;
use serde::{Deserialize, Serialize};

/// A rule of the contract that does not hold: a stable code and a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistError {
    pub code: &'static str,
    pub message: String,
}

impl PlaylistError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for PlaylistError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

pub type PlaylistResult<T> = Result<T, PlaylistError>;

/// A clip and the saved representation the program plays (v5 `MediaRef`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRef {
    pub clip_id: String,
    /// QNC URI of the media; the resolver of the host that decodes binds it.
    pub media_uri: String,
}

/// Source frames `[source_in, source_out)` in the source timebase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameRange {
    pub source_in: i64,
    pub source_out: i64,
    pub timebase: FrameTimebase,
}

impl FrameRange {
    pub fn frame_len(&self) -> i64 {
        (self.source_out - self.source_in).max(0)
    }

    pub fn is_empty(&self) -> bool {
        self.source_out <= self.source_in
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanMode {
    Progressive,
    InterlacedTopFieldFirst,
    InterlacedBottomFieldFirst,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbedVideoFormat {
    pub width: u32,
    pub height: u32,
    pub scan_mode: ScanMode,
}

impl ProbedVideoFormat {
    pub fn validate(self) -> PlaylistResult<()> {
        if self.width == 0 || self.height == 0 {
            return Err(PlaylistError::new(
                "invalid_source_video_format",
                "Probed video width and height must be greater than zero.",
            ));
        }
        if self.scan_mode == ScanMode::Unknown {
            return Err(PlaylistError::new(
                "unknown_source_scan_mode",
                "Source scan mode must come from media probe metadata.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbedAudioFormat {
    pub sample_rate_hz: u32,
    pub channel_count: u16,
}

impl ProbedAudioFormat {
    pub fn validate(self) -> PlaylistResult<()> {
        if self.sample_rate_hz == 0 || self.channel_count == 0 {
            return Err(PlaylistError::new(
                "invalid_source_audio_format",
                "Probed audio sample rate and channel count must be greater than zero.",
            ));
        }
        Ok(())
    }
}

/// Program frames `[in_frame, out_frame)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramFrameRange {
    pub in_frame: i64,
    pub out_frame: i64,
}

impl ProgramFrameRange {
    pub fn new(in_frame: i64, out_frame: i64) -> PlaylistResult<Self> {
        if in_frame < 0 || out_frame <= in_frame {
            return Err(PlaylistError::new(
                "invalid_program_range",
                format!("Program range must be positive and non-empty: {in_frame}..{out_frame}"),
            ));
        }
        Ok(Self {
            in_frame,
            out_frame,
        })
    }

    pub fn frame_len(self) -> i64 {
        self.out_frame - self.in_frame
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProgramVideoLayer {
    Base,
    Cover,
    Graphics,
}

/// Discrete program output channels; the caller gives the project `audio.channels`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramAudioLayout {
    pub channel_count: u16,
}

impl ProgramAudioLayout {
    pub fn discrete(channel_count: u16) -> PlaylistResult<Self> {
        let layout = Self { channel_count };
        layout.validate()?;
        Ok(layout)
    }

    fn validate(self) -> PlaylistResult<()> {
        if self.channel_count == 0 {
            return Err(PlaylistError::new(
                "invalid_program_audio_channels",
                "Program audio must define at least one discrete output channel.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramAudioRoute {
    pub source_channel: u16,
    pub output_channel: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlatProgramSource {
    pub source_id: String,
    pub clip_id: String,
    pub virtual_shot_id: String,
    pub media: MediaRef,
    pub source_range: FrameRange,
    pub source_duration_frames: i64,
    pub video_layer: Option<ProgramVideoLayer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_video_format: Option<ProbedVideoFormat>,
    #[serde(default)]
    pub audio_routes: Vec<ProgramAudioRoute>,
    pub source_audio_channels: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_audio_format: Option<ProbedAudioFormat>,
}

impl FlatProgramSource {
    pub fn has_video(&self) -> bool {
        self.video_layer.is_some()
    }

    pub fn has_audio(&self) -> bool {
        !self.audio_routes.is_empty()
    }

    fn validate(
        &self,
        item_range: ProgramFrameRange,
        program_timebase: FrameTimebase,
        program_audio_layout: ProgramAudioLayout,
    ) -> PlaylistResult<()> {
        if self.source_id.trim().is_empty() {
            return Err(PlaylistError::new(
                "invalid_program_source",
                "Program source_id must not be blank.",
            ));
        }
        if self.clip_id.trim().is_empty() || self.media.clip_id.trim().is_empty() {
            return Err(PlaylistError::new(
                "invalid_program_source",
                format!("Program source '{}' has no clip identity.", self.source_id),
            ));
        }
        if self.media.media_uri.trim().is_empty() {
            return Err(PlaylistError::new(
                "invalid_program_source",
                format!("Program source '{}' has no media URI.", self.source_id),
            ));
        }
        if self.clip_id != self.media.clip_id {
            return Err(PlaylistError::new(
                "program_media_identity_mismatch",
                format!(
                    "Program source '{}' clip_id '{}' does not match media clip_id '{}'.",
                    self.source_id, self.clip_id, self.media.clip_id
                ),
            ));
        }
        if self.source_range.is_empty() || self.source_range.source_in < 0 {
            return Err(PlaylistError::new(
                "invalid_source_range",
                format!(
                    "Program source '{}' has invalid source range {}..{}.",
                    self.source_id, self.source_range.source_in, self.source_range.source_out
                ),
            ));
        }
        if self.source_duration_frames <= 0
            || self.source_range.source_out > self.source_duration_frames
        {
            return Err(PlaylistError::new(
                "invalid_source_duration",
                format!(
                    "Program source '{}' range ends at {}, outside probed source duration {}.",
                    self.source_id, self.source_range.source_out, self.source_duration_frames
                ),
            ));
        }
        if self.source_range.timebase != program_timebase {
            return Err(PlaylistError::new(
                "mixed_program_timebase",
                format!(
                    "Program source '{}' timebase {}/{} does not match program timebase {}/{}.",
                    self.source_id,
                    self.source_range.timebase.fps_num,
                    self.source_range.timebase.fps_den,
                    program_timebase.fps_num,
                    program_timebase.fps_den
                ),
            ));
        }
        if self.source_range.frame_len() != item_range.frame_len() {
            return Err(PlaylistError::new(
                "program_source_length_mismatch",
                format!(
                    "Program source '{}' length {} does not match record length {}.",
                    self.source_id,
                    self.source_range.frame_len(),
                    item_range.frame_len()
                ),
            ));
        }
        if !self.has_video() && !self.has_audio() {
            return Err(PlaylistError::new(
                "empty_program_source",
                format!(
                    "Program source '{}' has no active video or audio output.",
                    self.source_id
                ),
            ));
        }
        match (self.has_video(), self.source_video_format) {
            (true, Some(format)) => format.validate()?,
            (true, None) => {
                return Err(PlaylistError::new(
                    "missing_source_video_format",
                    format!(
                        "Program video source '{}' has no persisted probe format.",
                        self.source_id
                    ),
                ));
            }
            (false, Some(_)) => {
                return Err(PlaylistError::new(
                    "unexpected_source_video_format",
                    format!(
                        "Audio-only program source '{}' carries a video format.",
                        self.source_id
                    ),
                ));
            }
            (false, None) => {}
        }
        if self.has_audio() && self.source_audio_format.is_none() {
            return Err(PlaylistError::new(
                "missing_source_audio_format",
                format!(
                    "Program audio source '{}' has no persisted probe format.",
                    self.source_id
                ),
            ));
        }
        if let Some(format) = self.source_audio_format {
            format.validate()?;
        }
        let mut output_channels = Vec::new();
        for route in &self.audio_routes {
            let probed = self
                .source_audio_format
                .map_or(self.source_audio_channels, |format| {
                    format.channel_count.min(self.source_audio_channels)
                });
            if route.source_channel >= probed {
                return Err(PlaylistError::new(
                    "program_audio_source_channel_out_of_bounds",
                    format!(
                        "Program source '{}' routes source channel {}, but probe reports {} channels.",
                        self.source_id,
                        route.source_channel + 1,
                        probed
                    ),
                ));
            }
            if route.output_channel >= program_audio_layout.channel_count {
                return Err(PlaylistError::new(
                    "program_audio_route_out_of_bounds",
                    format!(
                        "Program source '{}' routes to output channel {}, but program has {} channels.",
                        self.source_id,
                        route.output_channel + 1,
                        program_audio_layout.channel_count
                    ),
                ));
            }
            if output_channels.contains(&route.output_channel) {
                return Err(PlaylistError::new(
                    "duplicate_program_audio_route",
                    format!(
                        "Program source '{}' routes more than once to output channel {}.",
                        self.source_id,
                        route.output_channel + 1
                    ),
                ));
            }
            output_channels.push(route.output_channel);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlatProgramItem {
    pub item_id: String,
    pub record_range: ProgramFrameRange,
    pub sources: Vec<FlatProgramSource>,
}

impl FlatProgramItem {
    fn validate(
        &self,
        program_timebase: FrameTimebase,
        program_audio_layout: ProgramAudioLayout,
    ) -> PlaylistResult<()> {
        let record_range =
            ProgramFrameRange::new(self.record_range.in_frame, self.record_range.out_frame)?;
        if self.item_id.trim().is_empty() {
            return Err(PlaylistError::new(
                "invalid_program_item",
                "Program item_id must not be blank.",
            ));
        }
        if self.sources.is_empty() {
            return Err(PlaylistError::new(
                "empty_program_item",
                format!("Program item '{}' has no sources.", self.item_id),
            ));
        }
        let mut video_layers = Vec::new();
        let mut audio_channels = Vec::new();
        for source in &self.sources {
            source.validate(record_range, program_timebase, program_audio_layout)?;
            if let Some(layer) = source.video_layer {
                if video_layers.contains(&layer) {
                    return Err(PlaylistError::new(
                        "duplicate_program_video_layer",
                        format!(
                            "Program item '{}' has duplicate {:?} video layer.",
                            self.item_id, layer
                        ),
                    ));
                }
                video_layers.push(layer);
            }
            for route in &source.audio_routes {
                if audio_channels.contains(&route.output_channel) {
                    return Err(PlaylistError::new(
                        "duplicate_program_audio_bus",
                        format!(
                            "Program item '{}' has duplicate output audio channel {}.",
                            self.item_id,
                            route.output_channel + 1
                        ),
                    ));
                }
                audio_channels.push(route.output_channel);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlatProgramPlaylist {
    pub playlist_id: String,
    pub project_id: String,
    pub revision: u64,
    pub program_timebase: FrameTimebase,
    pub audio_layout: ProgramAudioLayout,
    pub duration_frames: i64,
    pub items: Vec<FlatProgramItem>,
}

impl FlatProgramPlaylist {
    pub fn validate(&self) -> PlaylistResult<()> {
        if self.playlist_id.trim().is_empty() || self.project_id.trim().is_empty() {
            return Err(PlaylistError::new(
                "invalid_program_playlist",
                "Program playlist and project identities must not be blank.",
            ));
        }
        if self.duration_frames <= 0 || self.items.is_empty() {
            return Err(PlaylistError::new(
                "empty_program_playlist",
                "Program playlist must contain a positive frame duration and at least one item.",
            ));
        }
        self.audio_layout.validate()?;
        let mut expected_in = 0;
        for item in &self.items {
            item.validate(self.program_timebase, self.audio_layout)?;
            if item.record_range.in_frame != expected_in {
                return Err(PlaylistError::new(
                    "non_contiguous_program_playlist",
                    format!(
                        "Program item '{}' starts at {}, expected {}.",
                        item.item_id, item.record_range.in_frame, expected_in
                    ),
                ));
            }
            if item.record_range.out_frame > self.duration_frames {
                return Err(PlaylistError::new(
                    "program_item_out_of_bounds",
                    format!(
                        "Program item '{}' ends at {}, outside duration {}.",
                        item.item_id, item.record_range.out_frame, self.duration_frames
                    ),
                ));
            }
            expected_in = item.record_range.out_frame;
        }
        if expected_in != self.duration_frames {
            return Err(PlaylistError::new(
                "incomplete_program_playlist",
                format!(
                    "Program items end at frame {expected_in}, expected duration {}.",
                    self.duration_frames
                ),
            ));
        }
        Ok(())
    }
}

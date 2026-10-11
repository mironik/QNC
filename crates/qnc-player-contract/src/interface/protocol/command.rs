use serde::{Deserialize, Serialize};

use super::BroadcastPlaybackRequest;
use crate::model::{FrameNumber, SourceRuntime};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum BroadcastPlayerProtocolCommand {
    LoadSource {
        source: SourceRuntime,
    },
    PreloadSource {
        source: SourceRuntime,
    },
    SetActiveSource {
        source_id: String,
    },
    UnloadSource,
    SetPlaybackRequest {
        request: Box<BroadcastPlaybackRequest>,
    },
    CueFrame {
        frame: FrameNumber,
        present_frame: bool,
    },

    /// The pictures of these frames the player already holds in memory go to the stills
    /// frame map of the session (user 2026-10-11: the trim overlay takes its pictures from
    /// the player, nothing is decoded for them). A picture it does not hold is left out.
    PublishStills {
        pictures: Vec<StillRef>,
    },

    Play,
    Pause,
    Stop,
    Shutdown,
}

impl BroadcastPlayerProtocolCommand {
    pub fn command_name(&self) -> &'static str {
        match self {
            BroadcastPlayerProtocolCommand::LoadSource { .. } => "LoadSource",
            BroadcastPlayerProtocolCommand::PreloadSource { .. } => "PreloadSource",
            BroadcastPlayerProtocolCommand::SetActiveSource { .. } => "SetActiveSource",
            BroadcastPlayerProtocolCommand::UnloadSource => "UnloadSource",
            BroadcastPlayerProtocolCommand::SetPlaybackRequest { .. } => "SetPlaybackRequest",
            BroadcastPlayerProtocolCommand::CueFrame { .. } => "CueFrame",
            BroadcastPlayerProtocolCommand::PublishStills { .. } => "PublishStills",
            BroadcastPlayerProtocolCommand::Play => "Play",
            BroadcastPlayerProtocolCommand::Pause => "Pause",
            BroadcastPlayerProtocolCommand::Stop => "Stop",
            BroadcastPlayerProtocolCommand::Shutdown => "Shutdown",
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self {
            BroadcastPlayerProtocolCommand::LoadSource { source }
            | BroadcastPlayerProtocolCommand::PreloadSource { source } => {
                validate_source_runtime(source)
            }
            BroadcastPlayerProtocolCommand::SetActiveSource { source_id } => {
                reject_blank("source_id", source_id)
            }
            BroadcastPlayerProtocolCommand::SetPlaybackRequest { request } => request.validate(),
            BroadcastPlayerProtocolCommand::CueFrame { .. } => Ok(()),
            BroadcastPlayerProtocolCommand::PublishStills { pictures } => {
                if pictures.len() > MAX_STILLS {
                    return Err(format!("at most {MAX_STILLS} stills at once"));
                }
                pictures.iter().try_for_each(|picture| reject_blank("source_id", &picture.source_id))
            }
            BroadcastPlayerProtocolCommand::UnloadSource
            | BroadcastPlayerProtocolCommand::Play
            | BroadcastPlayerProtocolCommand::Pause
            | BroadcastPlayerProtocolCommand::Stop => Ok(()),
            BroadcastPlayerProtocolCommand::Shutdown => Ok(()),
        }
    }
}

pub fn validate_broadcast_player_protocol_command(
    command: &BroadcastPlayerProtocolCommand,
) -> Result<(), String> {
    command.validate()
}

fn validate_source_runtime(source: &SourceRuntime) -> Result<(), String> {
    source.validate()
}

fn reject_blank(label: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{label} must not be blank"));
    }
    Ok(())
}

/// At most this many stills are asked at once (the stills frame map keeps 8).
pub const MAX_STILLS: usize = 8;

/// A picture of a media of the session: its source id and frame (a source frame of that
/// media, also within a story program).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StillRef {
    pub source_id: String,
    pub frame: FrameNumber,
}

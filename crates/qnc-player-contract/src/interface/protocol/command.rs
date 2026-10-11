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

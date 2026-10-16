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
    /// The level of the first output channels in percent (100 as recorded, at most
    /// 400), heard at once; nothing is prepared again (user 2026-10-16).
    SetOutputLevels {
        levels_percent: Vec<u16>,
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
            BroadcastPlayerProtocolCommand::SetOutputLevels { .. } => "SetOutputLevels",
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
            BroadcastPlayerProtocolCommand::SetOutputLevels { levels_percent } => {
                if levels_percent.is_empty() || levels_percent.len() > 8 || levels_percent.iter().any(|level| *level > 400) {
                    Err("output levels: 1 to 8 channels of 0..400 percent".into())
                } else {
                    Ok(())
                }
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

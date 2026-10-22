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
    /// The source channel (zero based, in the saved inventory) each of the first output
    /// channels plays, heard at once; the player decodes every channel, nothing is
    /// prepared again (user 2026-10-19).
    SetOutputRoutes {
        channels: Vec<u16>,
    },
    /// The story's A1 under a Sync (the bed the session was launched with) plays from the
    /// sample of clip frame `source_frame` on, from its frame `bed_frame`, in the session's
    /// own clock and device (user 2026-10-20).
    StartSyncBed {
        source_frame: FrameNumber,
        bed_frame: FrameNumber,
    },
    /// The Sync bed stops at once; the clip's own A1 is heard again.
    StopSyncBed,
    /// One part of a program (frames `[from_frame, to_frame)` on output `output`) heard
    /// `gain_db` louder than stored, or silent (`off`), at once; nothing is prepared again
    /// (user 2026-10-22: the level of one cover). `from_frame == to_frame` clears it.
    SetPartLevel {
        output: u16,
        from_frame: FrameNumber,
        to_frame: FrameNumber,
        gain_db: i16,
        off: bool,
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
            BroadcastPlayerProtocolCommand::SetOutputRoutes { .. } => "SetOutputRoutes",
            BroadcastPlayerProtocolCommand::StartSyncBed { .. } => "StartSyncBed",
            BroadcastPlayerProtocolCommand::StopSyncBed => "StopSyncBed",
            BroadcastPlayerProtocolCommand::SetPartLevel { .. } => "SetPartLevel",
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
            BroadcastPlayerProtocolCommand::SetOutputRoutes { channels } => {
                if channels.is_empty() || channels.len() > 8 || channels.iter().any(|channel| *channel >= 64) {
                    Err("output routes: 1 to 8 outputs of source channels 0..64".into())
                } else {
                    Ok(())
                }
            }
            BroadcastPlayerProtocolCommand::StartSyncBed { .. } | BroadcastPlayerProtocolCommand::StopSyncBed => Ok(()),
            BroadcastPlayerProtocolCommand::SetPartLevel { output, from_frame, to_frame, gain_db, .. } => {
                if *output >= 8 || from_frame > to_frame || !(-100..=100).contains(gain_db) {
                    Err("part level: output 0..8, from <= to, -100..100 dB".into())
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

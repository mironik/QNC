//! Player input of a story program: the flat program playlist and the prepared
//! input of every clip it plays. Read only, no DB access here; the program input
//! reader builds it, the player only checks and plays it.

use super::*;
use qnc_program_playlist::FlatProgramPlaylist;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramInput {
    pub contract_version: String,
    pub workspace_db_uri: String,
    /// Output format of the program: the project `audio.channels` and rate.
    pub project_audio: ProjectAudio,
    pub playlist: FlatProgramPlaylist,
    /// Prepared input of every clip of the playlist, by clip id.
    pub clips: BTreeMap<String, PreparedInput>,
}

impl ProgramInput {
    /// Takes the workspace and project audio of the prepared clips.
    pub fn new(
        playlist: FlatProgramPlaylist,
        clips: BTreeMap<String, PreparedInput>,
    ) -> Result<Self> {
        let first = clips.values().next().ok_or(InputError::MissingClip)?;
        let input = Self {
            contract_version: VERSION.into(),
            workspace_db_uri: first.workspace_db_uri.clone(),
            project_audio: first.project_audio.clone(),
            playlist,
            clips,
        };
        input.validate_for(&input.workspace_db_uri.clone())?;
        Ok(input)
    }

    /// Every clip of the playlist is prepared for this workspace and project
    /// audio, and the playlist names exactly the media of those clips: the
    /// picture the project plays, the sound of the original.
    pub fn validate_for(&self, workspace_uri: &str) -> Result<()> {
        if self.contract_version != VERSION {
            return Err(InputError::InvalidDescriptor);
        }
        validate_workspace(&self.workspace_db_uri)?;
        if self.workspace_db_uri != workspace_uri {
            return Err(InputError::WrongWorkspace);
        }
        self.project_audio.validate()?;
        self.playlist
            .validate()
            .map_err(|error| InputError::InvalidRecord(error.to_string()))?;
        if self.playlist.audio_layout.channel_count != self.project_audio.channels {
            return Err(InputError::InvalidRecord(
                "Program audio layout differs from project audio.channels.".into(),
            ));
        }
        for (clip_id, clip) in &self.clips {
            clip.validate_for(workspace_uri, clip_id)?;
            if clip.project_audio != self.project_audio {
                return Err(InputError::ChangedSettings);
            }
        }
        for source in self.playlist.items.iter().flat_map(|item| &item.sources) {
            let clip = self
                .clips
                .get(&source.clip_id)
                .ok_or(InputError::MissingClip)?;
            if source.has_video() && source.media.media_uri != clip.media()?.media_uri {
                return Err(InputError::InvalidRecord(format!(
                    "Program picture of '{}' is not the project playback media.",
                    source.clip_id
                )));
            }
            if !source.has_video() && source.media.media_uri != clip.audio_media().media_uri {
                return Err(InputError::InvalidRecord(format!(
                    "Program sound of '{}' is not the original media.",
                    source.clip_id
                )));
            }
        }
        Ok(())
    }
}

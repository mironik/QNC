//! Program input of an edited story (v5 host `program_playlist.rs` and
//! `editorial_playlist.rs`), read only.
//!
//! The story comes from the active project database (`story_parts`,
//! `story_covers`, table module `qnc-program-db`); the media of every clip comes by
//! the same path as a single clip (`qnc-player-input`): the picture follows the
//! project `playback.input`, the sound is the original, the facts are the saved
//! record (no probe). The output is the flat program playlist of
//! `qnc-program-playlist` and the prepared input of every clip in it, all a
//! program player needs. Nothing is written, no media is opened.

use std::collections::BTreeMap;

use qnc_db_broker::ProjectDbTarget;
use qnc_program_db::{ProgramCover, ProgramSegment, StoryReader};
use qnc_media_metadata::{MediaRepresentation, StreamDetails};
use qnc_player_input::{PreparedInput, ProgramInput, StreamLayout};
use qnc_program_playlist::{
    build_flat_program_playlist, build_program_frame_window, FrameRange, FrameTimebase, MediaRef,
    ProbedAudioFormat, ProbedVideoFormat, ProgramAudioLayout, ProgramCoverInput, ProgramFrameRange,
    ProgramMediaResolver, ProgramPlaylistBuildInput, ProgramSegmentInput, ResolvedProgramMedia,
    ScanMode,
};

pub const MODULE_ID: &str = "qnc.module.program-input";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The prepared input of a clip of the active project (`qnc-player-input`).
pub trait ClipInputs {
    fn prepared(&self, clip_id: &str) -> Result<PreparedInput, String>;
    /// Frames of the A2 crossfade, the project `audio.crossfade_frames` (user 2026-10-21).
    fn crossfade_frames(&self) -> Result<u16, String>;
}

/// `qnc-player-input` reader of the active project workspace.
pub struct PlayerClipInputs<'a> {
    pub session: &'a qnc_player_input::ClipSession<'a>,
}

impl ClipInputs for PlayerClipInputs<'_> {
    fn prepared(&self, clip_id: &str) -> Result<PreparedInput, String> {
        self.session
            .load(clip_id)
            .map_err(|error| format!("Klip '{clip_id}': {error}"))
    }

    fn crossfade_frames(&self) -> Result<u16, String> {
        self.session.settings().audio_crossfade_frames().map_err(|error| error.to_string())
    }
}

/// Every clip of the program under one read of the project settings, checked
/// unchanged at the end.
fn with_session(
    reader: &qnc_player_input::InputReader,
    workspace_db_uri: &str,
    build: impl FnOnce(&PlayerClipInputs) -> Result<ProgramInput, String>,
) -> Result<ProgramInput, String> {
    let session = reader.session(workspace_db_uri).map_err(|error| error.to_string())?;
    let program = build(&PlayerClipInputs { session: &session })?;
    session.finish().map_err(|error| error.to_string())?;
    Ok(program)
}

/// The sound of one stored segment (lane 0, A1) or cover (lane 1, A2) as drafted on the
/// Wrap row, heard before Enter writes it (user 2026-10-16). It is never written here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundDraft {
    pub lane: u8,
    pub id: String,
    pub channel: u16,
    pub level_db: i8,
    pub off: bool,
    /// Covers with their J and L as drafted on the Wrap row (user 2026-10-21: the sound of an
    /// M marker's cut), story frames; the others keep theirs.
    pub overlaps: Vec<(String, (u32, u32))>,
    /// The segment's picture J as drafted (lane 0); `None` keeps the stored one.
    pub picture_lead: Option<u32>,
    /// Clips with the J and L of their A1 sound as drafted at a cut (user 2026-10-29), story
    /// frames; the others keep theirs.
    pub joins: Vec<(String, (u32, u32))>,
}

impl SoundDraft {
    fn apply(&self, segments: &mut [ProgramSegment], covers: &mut [ProgramCover]) {
        if self.lane == 0 {
            for segment in segments.iter_mut().filter(|segment| segment.segment_id == self.id) {
                (segment.a1_source_channel, segment.a1_level_db, segment.a1_off) = (self.channel, self.level_db, self.off);
                if let Some(frames) = self.picture_lead {
                    segment.v_lead_frames = frames;
                }
            }
        } else {
            for cover in covers.iter_mut().filter(|cover| cover.cover_id == self.id) {
                (cover.a2_source_channel, cover.a2_level_db, cover.a2_off) = (self.channel, self.level_db, self.off);
            }
        }
        for (id, (lead, tail)) in &self.joins {
            for segment in segments.iter_mut().filter(|segment| &segment.segment_id == id) {
                (segment.a1_lead_frames, segment.a1_tail_frames) = (*lead, *tail);
            }
        }
        for (id, (lead, tail)) in &self.overlaps {
            for cover in covers.iter_mut().filter(|cover| &cover.cover_id == id) {
                (cover.a2_lead_frames, cover.a2_tail_frames) = (*lead, *tail);
            }
        }
    }
}


mod build;
mod load;
mod media;
mod normalize;
mod pieces;

pub use qnc_program_loudness::CoverRule;

pub use build::*;
pub use load::*;
pub use media::*;
pub use pieces::*;

#[cfg(test)]
mod tests;

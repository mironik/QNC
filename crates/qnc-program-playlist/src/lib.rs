//! Flat program playlist of an edited story (v5 `qnc-program-playlist`).
//!
//! Neutral and frame based: no UI, database, player, export or worker. Callers
//! give the editorial spans (segments in program order, their covers) and resolve
//! media through [`ProgramMediaResolver`] from the saved media record, never by a
//! new probe. The program becomes contiguous items: Ton = picture (Base) + A1,
//! Off = no picture + A1, a cover = picture (Cover) + A2 over the A1 of its
//! segment. Which source channel goes to A1 (segment) or A2 (cover) is a field of
//! the segment or cover, chosen while building on the Wrap segment (v5 takes
//! channel 1; a talk recorded only on channel 2 needs channel 2). The program is
//! read only for the user: it only takes what the segments hold.
//!
//! QNC differences to v5: the output channel count is the project
//! `audio.channels` given by the caller, media is a QNC URI, and there are no
//! stand-in formats; missing facts end in an error.

mod contract;
mod crossfade;
mod a1_joins;
mod picture_lead;
mod build;
mod overlay;
mod window;

pub(crate) use build::*;
pub use a1_joins::{join_a1, A1Join};
pub use overlay::*;
pub use window::*;

pub use contract::{
    FlatProgramItem, FlatProgramPlaylist, FlatProgramSource, FrameRange, MediaRef, PlaylistError,
    PlaylistResult, ProbedAudioFormat, ProbedVideoFormat, ProgramAudioLayout, ProgramAudioRoute,
    ProgramFrameRange, ProgramVideoLayer, ScanMode,
};
pub use qnc_frame_timebase::FrameTimebase;

pub const MODULE_ID: &str = "qnc.module.program-playlist";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Program output channel of A1 (Ton / Off sound).
pub const PROGRAM_AUDIO_OUTPUT_A1: u16 = 0;
/// Program output channel of A2 (cover sound).
pub const PROGRAM_AUDIO_OUTPUT_A2: u16 = 1;
/// v5 routes channel 1 of a source (index 0) to A1 and A2.
pub const DEFAULT_SOURCE_CHANNEL: u16 = 0;

/// Saved facts of the media a clip plays from. The picture follows the project
/// `playback.input` (proxy or original); the sound always comes from the original
/// (AGENTS 8.2), so it has its own media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProgramMedia {
    /// Media of the picture.
    pub media: MediaRef,
    /// Media of the sound: the original.
    pub audio_media: MediaRef,
    pub duration_frames: i64,
    pub timebase: FrameTimebase,
    pub video_format: Option<ProbedVideoFormat>,
    pub has_audio: bool,
    pub audio_channels: u16,
    pub audio_format: Option<ProbedAudioFormat>,
    /// Original timecode of frame 0 of the clip (frames at its nominal rate).
    pub timecode_start: Option<i64>,
}

pub trait ProgramMediaResolver {
    fn resolve(&mut self, clip_id: &str) -> Result<ResolvedProgramMedia, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramPlaylistBuildInput {
    pub playlist_id: String,
    pub project_id: String,
    pub revision: u64,
    pub program_timebase: FrameTimebase,
    pub audio_layout: ProgramAudioLayout,
    pub duration_frames: i64,
    pub segments: Vec<ProgramSegmentInput>,
    /// Frames of the crossfade on every A2 cut, the project `audio.crossfade_frames`.
    pub a2_crossfade_frames: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramSegmentInput {
    pub segment_id: String,
    /// `tonovi` or `offovi`.
    pub kind: String,
    pub clip_id: String,
    pub virtual_shot_id: String,
    pub record_range: ProgramFrameRange,
    pub source_range: FrameRange,
    /// Source channel heard on A1 (zero based), chosen on the Wrap segment.
    pub a1_source_channel: u16,
    /// Level of A1 in dB (0 as recorded) and A1 off (silent), user 2026-10-16.
    pub a1_level_db: i8,
    pub a1_off: bool,
    pub covers: Vec<ProgramCoverInput>,
    /// The picture J (Ton only): its picture starts this many frames before its sound.
    pub v_lead_frames: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramCoverInput {
    pub cover_id: String,
    pub clip_id: String,
    pub virtual_shot_id: String,
    pub record_range: ProgramFrameRange,
    pub source_range: FrameRange,
    /// Source channel heard on A2 (zero based).
    pub a2_source_channel: u16,
    /// Level of A2 in dB (0 as recorded) and A2 off (silent).
    pub a2_level_db: i8,
    pub a2_off: bool,
    pub active: bool,
    /// Frames the cover's A2 starts before its picture (a J), 0 for the project crossfade only.
    pub a2_lead_frames: i64,
    /// Frames the cover's A2 goes on after its picture (an L), 0 for none.
    pub a2_tail_frames: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramTransientOverlayInput {
    pub record_range: ProgramFrameRange,
    pub source: FlatProgramSource,
}

pub fn build_flat_program_playlist(
    input: &ProgramPlaylistBuildInput,
    resolver: &mut impl ProgramMediaResolver,
) -> Result<FlatProgramPlaylist, String> {
    let mut items = Vec::new();
    for segment in &input.segments {
        items.extend(flat_items_for_segment(segment, resolver)?);
    }
    if items.is_empty() {
        return Err("The program is empty.".into());
    }
    picture_lead::lead_pictures(&mut items, &input.segments, resolver)?;
    let asked: std::collections::HashMap<&str, crossfade::Overlap> = input
        .segments
        .iter()
        .flat_map(|segment| &segment.covers)
        .map(|cover| (cover.cover_id.as_str(), crossfade::Overlap { lead: cover.a2_lead_frames, tail: cover.a2_tail_frames }))
        .collect();
    let end = items.iter().map(|item| item.record_range.out_frame).max().unwrap_or(0);
    crossfade::crossfade_a2(&mut items, input.a2_crossfade_frames, |id| asked.get(id).copied().unwrap_or_default(), end)?;
    let playlist = FlatProgramPlaylist {
        playlist_id: input.playlist_id.clone(),
        project_id: input.project_id.clone(),
        revision: input.revision,
        program_timebase: input.program_timebase,
        audio_layout: input.audio_layout,
        duration_frames: input.duration_frames,
        items,
    };
    playlist.validate().map_err(|error| error.message)?;
    Ok(playlist)
}

#[cfg(test)]
mod tests;

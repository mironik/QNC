//! Program input of an edited story (v5 host `program_playlist.rs` and
//! `editorial_playlist.rs`), read only.
//!
//! The story comes from the active project database (`story_parts`,
//! `story_covers`, owner `qnc-content-store`); the media of every clip comes by
//! the same path as a single clip (`qnc-player-input`): the picture follows the
//! project `playback.input`, the sound is the original, the facts are the saved
//! record (no probe). The output is the flat program playlist of
//! `qnc-program-playlist` and the prepared input of every clip in it, all a
//! program player needs. Nothing is written, no media is opened.

use std::collections::BTreeMap;

use qnc_content_store::{Access, ContentTarget, ProgramCover, ProgramSegment};
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
}

/// `qnc-player-input` reader of the active project workspace.
pub struct PlayerClipInputs<'a> {
    pub reader: &'a qnc_player_input::InputReader,
    pub workspace_db_uri: &'a str,
}

impl ClipInputs for PlayerClipInputs<'_> {
    fn prepared(&self, clip_id: &str) -> Result<PreparedInput, String> {
        self.reader
            .load(self.workspace_db_uri, clip_id)
            .map_err(|error| format!("Klip '{clip_id}': {error}"))
    }
}

/// A cover played over a program window only (v5 Sync/B-roll preview): the
/// source from its IN as cover picture and A2 (source channel 1); the stored
/// covers of the window give way to it. It is never written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientCover {
    pub clip_id: String,
    pub source_in: u64,
    pub timebase: (u32, u32),
}

const TRANSIENT_COVER_ID: &str = "sync-cover-preview";

/// Reads the story of the active project and builds its program.
pub fn load_program(
    target: &ContentTarget,
    project_id: &str,
    clips: &impl ClipInputs,
) -> Result<ProgramInput, String> {
    load(target, project_id, clips, None)
}

/// The program window `[in, out)` of the active project with a transient cover
/// over it (v5 `build_program_frame_window` + `apply_transient_program_overlay`);
/// its frames start at 0.
pub fn load_program_window(
    target: &ContentTarget,
    project_id: &str,
    clips: &impl ClipInputs,
    window: (u64, u64),
    cover: &TransientCover,
) -> Result<ProgramInput, String> {
    load(target, project_id, clips, Some((window, cover)))
}

/// The stored covers outside the window and the transient cover over all of it.
pub fn with_transient_cover(
    covers: &[ProgramCover],
    (start, end): (u64, u64),
    cover: &TransientCover,
) -> Vec<ProgramCover> {
    covers
        .iter()
        .filter(|stored| stored.program_end_frame <= start || stored.program_start_frame >= end)
        .cloned()
        .chain(std::iter::once(ProgramCover {
            cover_id: TRANSIENT_COVER_ID.into(),
            slot_id: String::new(),
            clip_id: cover.clip_id.clone(),
            virtual_shot_id: TRANSIENT_COVER_ID.into(),
            program_start_frame: start,
            program_end_frame: end,
            source_in_frame: cover.source_in,
            source_out_frame: cover.source_in + end.saturating_sub(start),
            fps_num: cover.timebase.0,
            fps_den: cover.timebase.1,
            a2_source_channel: 0,
        }))
        .collect()
}

fn load(
    target: &ContentTarget,
    project_id: &str,
    clips: &impl ClipInputs,
    window: Option<((u64, u64), &TransientCover)>,
) -> Result<ProgramInput, String> {
    let mut client = target.open(Access::ReadOnly)?;
    let segments = client.list_segments()?;
    let mut covers = client.list_covers()?;
    drop(client);
    if let Some((range, cover)) = window {
        covers = with_transient_cover(&covers, range, cover);
    }
    let first = segments
        .iter()
        .find(|segment| segment.active)
        .ok_or("Program je prazan.")?;
    let mut resolver = PreparedResolver {
        clips,
        prepared: BTreeMap::new(),
    };
    let channels = resolver.prepared(&first.clip_id)?.project_audio.channels;
    let input = build_input(project_id, &segments, &covers, channels)?;
    let mut playlist = build_flat_program_playlist(&input, &mut resolver)?;
    if let Some(((start, end), _)) = window {
        let range =
            ProgramFrameRange::new(frame(start)?, frame(end)?).map_err(|error| error.message)?;
        playlist = build_program_frame_window(&playlist, range)?;
    }
    ProgramInput::new(playlist, resolver.prepared).map_err(|error| error.to_string())
}

/// The program of the active project, built where the player is prepared (off
/// the form thread): the story from its database, every clip through the
/// player input reader of the workspace.
pub fn loader(
    target: ContentTarget,
    project_id: String,
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static
{
    move |reader, workspace_db_uri| {
        load_program(
            &target,
            &project_id,
            &PlayerClipInputs {
                reader,
                workspace_db_uri,
            },
        )
    }
}
/// The program window with a transient cover, built where the player is prepared.
pub fn window_loader(
    target: ContentTarget,
    project_id: String,
    window: (u64, u64),
    cover: TransientCover,
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static
{
    move |reader, workspace_db_uri| {
        load_program_window(
            &target,
            &project_id,
            &PlayerClipInputs {
                reader,
                workspace_db_uri,
            },
            window,
            &cover,
        )
    }
}

/// v5 `build_segments` + `program_playlist_input`: the active segments one after
/// another on the program axis, each with the covers that overlap it; a cover
/// without its virtual shot or timebase is not played (v5 `streamable`).
pub fn build_input(
    project_id: &str,
    segments: &[ProgramSegment],
    covers: &[ProgramCover],
    audio_channels: u16,
) -> Result<ProgramPlaylistBuildInput, String> {
    let mut start = 0i64;
    let mut inputs = Vec::new();
    for segment in segments.iter().filter(|segment| segment.active) {
        let timebase = timebase(segment.fps_num, segment.fps_den, &segment.segment_id)?;
        let frames = frame(segment.out_frame)? - frame(segment.in_frame)?;
        let record_range =
            ProgramFrameRange::new(start, start + frames).map_err(|error| error.message)?;
        let covers = covers
            .iter()
            .filter(|cover| {
                !cover.virtual_shot_id.trim().is_empty()
                    && (cover.program_start_frame as i64) < record_range.out_frame
                    && (cover.program_end_frame as i64) > record_range.in_frame
            })
            .map(cover_input)
            .collect::<Result<Vec<_>, _>>()?;
        inputs.push(ProgramSegmentInput {
            segment_id: segment.segment_id.clone(),
            kind: segment.kind.clone(),
            clip_id: segment.clip_id.clone(),
            virtual_shot_id: String::new(),
            record_range,
            source_range: FrameRange {
                source_in: frame(segment.in_frame)?,
                source_out: frame(segment.out_frame)?,
                timebase,
            },
            a1_source_channel: segment.a1_source_channel,
            covers,
        });
        start += frames;
    }
    let first = segments
        .iter()
        .find(|segment| segment.active)
        .ok_or("Program je prazan.")?;
    Ok(ProgramPlaylistBuildInput {
        playlist_id: format!("program:{project_id}"),
        project_id: project_id.to_string(),
        revision: 0,
        program_timebase: timebase(first.fps_num, first.fps_den, &first.segment_id)?,
        audio_layout: ProgramAudioLayout::discrete(audio_channels)
            .map_err(|error| error.message)?,
        duration_frames: start,
        segments: inputs,
    })
}

fn cover_input(cover: &ProgramCover) -> Result<ProgramCoverInput, String> {
    Ok(ProgramCoverInput {
        cover_id: cover.cover_id.clone(),
        clip_id: cover.clip_id.clone(),
        virtual_shot_id: cover.virtual_shot_id.clone(),
        record_range: ProgramFrameRange::new(
            frame(cover.program_start_frame)?,
            frame(cover.program_end_frame)?,
        )
        .map_err(|error| error.message)?,
        source_range: FrameRange {
            source_in: frame(cover.source_in_frame)?,
            source_out: frame(cover.source_out_frame)?,
            timebase: timebase(cover.fps_num, cover.fps_den, &cover.cover_id)?,
        },
        a2_source_channel: cover.a2_source_channel,
        active: true,
    })
}

/// The saved facts of a clip for the program: picture from the representation
/// the project plays, sound from the original, channels in inventory order
/// (channel 1 is the first channel of the first audio stream).
pub fn resolved_media(
    clip_id: &str,
    picture: &MediaRepresentation,
    sound: &MediaRepresentation,
    layout: &StreamLayout,
) -> Result<ResolvedProgramMedia, String> {
    let video = layout
        .video
        .as_ref()
        .ok_or_else(|| format!("Klip '{clip_id}' nema spremljenu sliku za program."))?;
    let details = picture
        .streams
        .iter()
        .find(|stream| stream.index.as_ref().map(|i| i.value) == Some(video.stream_index))
        .and_then(|stream| match &stream.details {
            StreamDetails::Video(details) => Some(details),
            _ => None,
        })
        .ok_or_else(|| format!("Klip '{clip_id}' nema spremljeni video stream."))?;
    let known = |what: &str| format!("Klip '{clip_id}' nema spremljen podatak: {what}.");
    let video_format = ProbedVideoFormat {
        width: details.width.as_ref().ok_or_else(|| known("sirina"))?.value,
        height: details
            .height
            .as_ref()
            .ok_or_else(|| known("visina"))?
            .value,
        scan_mode: match details
            .scan_mode
            .as_ref()
            .ok_or_else(|| known("scan"))?
            .value
        {
            qnc_media_metadata::ScanMode::Progressive => ScanMode::Progressive,
            qnc_media_metadata::ScanMode::InterlacedTopFieldFirst => {
                ScanMode::InterlacedTopFieldFirst
            }
            qnc_media_metadata::ScanMode::InterlacedBottomFieldFirst => {
                ScanMode::InterlacedBottomFieldFirst
            }
        },
    };
    let audio_channels =
        u16::try_from(layout.audio_channels.len()).map_err(|_| known("broj audio kanala"))?;
    let audio_format = if audio_channels == 0 {
        None
    } else {
        let mut rates = layout.audio_channels.iter().map(|channel| {
            sound
                .streams
                .iter()
                .find(|stream| stream.index.as_ref().map(|i| i.value) == Some(channel.stream_index))
                .and_then(|stream| match &stream.details {
                    StreamDetails::Audio(audio) => audio.sample_rate_hz.as_ref(),
                    _ => None,
                })
                .map(|rate| rate.value)
                .ok_or_else(|| known("audio sample rate"))
        });
        let rate = rates
            .next()
            .unwrap_or_else(|| Err(known("audio sample rate")))?;
        for other in rates {
            if other? != rate {
                return Err(format!(
                    "Klip '{clip_id}' ima audio streamove s razlicitim sample rateom."
                ));
            }
        }
        Some(ProbedAudioFormat {
            sample_rate_hz: rate,
            channel_count: audio_channels,
        })
    };
    Ok(ResolvedProgramMedia {
        media: MediaRef {
            clip_id: clip_id.to_string(),
            media_uri: picture.media_uri.clone(),
        },
        audio_media: MediaRef {
            clip_id: clip_id.to_string(),
            media_uri: sound.media_uri.clone(),
        },
        duration_frames: i64::try_from(video.duration_frames).map_err(|_| known("trajanje"))?,
        timebase: video.timebase,
        video_format: Some(video_format),
        has_audio: audio_channels > 0,
        audio_channels,
        audio_format,
    })
}

struct PreparedResolver<'a, C> {
    clips: &'a C,
    prepared: BTreeMap<String, PreparedInput>,
}

impl<C: ClipInputs> PreparedResolver<'_, C> {
    fn prepared(&mut self, clip_id: &str) -> Result<&PreparedInput, String> {
        if !self.prepared.contains_key(clip_id) {
            let input = self.clips.prepared(clip_id)?;
            self.prepared.insert(clip_id.to_string(), input);
        }
        self.prepared
            .get(clip_id)
            .ok_or_else(|| format!("Klip '{clip_id}' nije pripremljen."))
    }
}

impl<C: ClipInputs> ProgramMediaResolver for PreparedResolver<'_, C> {
    fn resolve(&mut self, clip_id: &str) -> Result<ResolvedProgramMedia, String> {
        let input = self.prepared(clip_id)?;
        let picture = input.media().map_err(|error| error.to_string())?;
        resolved_media(clip_id, picture, input.audio_media(), &input.layout)
    }
}

fn timebase(num: u32, den: u32, label: &str) -> Result<FrameTimebase, String> {
    FrameTimebase::new(i64::from(num), i64::from(den))
        .map_err(|_| format!("'{label}' nema valjan originalni timebase iz zapisa."))
}

fn frame(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| "Frame je izvan raspona.".to_string())
}

#[cfg(test)]
mod tests;

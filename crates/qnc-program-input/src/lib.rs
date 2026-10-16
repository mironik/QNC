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

/// A cover played over a program window only (v5 Sync/B-roll preview): the
/// source from its IN as cover picture and A2 (the channel chosen for A2); the stored
/// covers of the window give way to it. It is never written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransientCover {
    pub clip_id: String,
    pub source_in: u64,
    pub timebase: (u32, u32),
    /// Source channel (zero based) heard on A2, the one chosen for covers.
    pub a2_source_channel: u16,
    /// Level of A2 in dB and A2 off, as set on the source timeline (user 2026-10-16).
    pub a2_level_db: i8,
    pub a2_off: bool,
}

const TRANSIENT_COVER_ID: &str = "sync-cover-preview";

/// The sound of one stored segment (lane 0, A1) or cover (lane 1, A2) as drafted on the
/// Wrap row, heard before Enter writes it (user 2026-10-16). It is never written here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundDraft {
    pub lane: u8,
    pub id: String,
    pub channel: u16,
    pub level_db: i8,
    pub off: bool,
}

impl SoundDraft {
    fn apply(&self, segments: &mut [ProgramSegment], covers: &mut [ProgramCover]) {
        if self.lane == 0 {
            for segment in segments.iter_mut().filter(|segment| segment.segment_id == self.id) {
                (segment.a1_source_channel, segment.a1_level_db, segment.a1_off) = (self.channel, self.level_db, self.off);
            }
        } else {
            for cover in covers.iter_mut().filter(|cover| cover.cover_id == self.id) {
                (cover.a2_source_channel, cover.a2_level_db, cover.a2_off) = (self.channel, self.level_db, self.off);
            }
        }
    }
}

/// Reads the story of the active project and builds its program.
pub fn load_program(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &impl ClipInputs,
) -> Result<ProgramInput, String> {
    load(target, project_id, clips, None, None)
}

/// The program window `[in, out)` of the active project with a transient cover
/// over it (v5 `build_program_frame_window` + `apply_transient_program_overlay`);
/// its frames start at 0.
pub fn load_program_window(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &impl ClipInputs,
    window: (u64, u64),
    cover: &TransientCover,
) -> Result<ProgramInput, String> {
    load(target, project_id, clips, Some((window, cover)), None)
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
            a2_source_channel: cover.a2_source_channel,
            a2_level_db: cover.a2_level_db,
            a2_off: cover.a2_off,
        }))
        .collect()
}

fn load(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &impl ClipInputs,
    window: Option<((u64, u64), &TransientCover)>,
    draft: Option<&SoundDraft>,
) -> Result<ProgramInput, String> {
    let mut client = StoryReader::open(target)?;
    let mut segments = client.list_segments()?;
    let mut covers = client.list_covers()?;
    if let Some(draft) = draft {
        draft.apply(&mut segments, &mut covers);
    }
    let story_rate = client.read_story_selection()?.story_fps.ok_or(qnc_ui_text::text("story.msg.program_is_empty"))?;
    drop(client);
    if let Some((range, cover)) = window {
        covers = with_transient_cover(&covers, range, cover);
    }
    let first = segments
        .iter()
        .find(|segment| segment.active)
        .ok_or(qnc_ui_text::text("story.msg.program_is_empty"))?;
    let mut resolver = PreparedResolver {
        clips,
        prepared: BTreeMap::new(),
        story_rate,
    };
    let channels = resolver.prepared(&first.clip_id)?.project_audio.channels;
    let covers = fit_covers(covers, &mut resolver, story_rate)?;
    let input = build_input(project_id, story_rate, &segments, &covers, channels)?;
    let mut playlist = build_flat_program_playlist(&input, &mut resolver)?;
    if let Some(((start, end), _)) = window {
        let range =
            ProgramFrameRange::new(frame(start)?, frame(end)?).map_err(|error| error.message)?;
        playlist = build_program_frame_window(&playlist, range)?;
    }
    let mut input = ProgramInput::new(playlist, resolver.prepared).map_err(|error| error.to_string())?;
    // The whole story plays its rendered units (a Sync window plays a cover not in the
    // story: from its sources). Units that cannot be read leave the story to its sources.
    if window.is_none() {
        input.prerendered = prerendered(target);
    }
    Ok(input)
}

/// The rendered units of the story's prerender (user 2026-10-12), as their render
/// described them; none when the project has none or they cannot be read.
fn prerendered(target: &ProjectDbTarget) -> Vec<qnc_player_input::PrerenderedUnit> {
    let Ok(units) = qnc_prerender_db::PrerenderReader::open(target).and_then(|mut reader| reader.all()) else {
        return Vec::new();
    };
    units
        .into_iter()
        .filter_map(|unit| {
            Some(qnc_player_input::PrerenderedUnit {
                fingerprint: unit.fingerprint,
                key: unit.key,
                media: serde_json::from_value(unit.media).ok()?,
                frames: unit.frames,
                samples: unit.samples,
            })
        })
        .collect()
}

/// The program of the active project, built where the player is prepared (off
/// the form thread): the story from its database, every clip through the
/// player input reader of the workspace.
pub fn loader(
    target: ProjectDbTarget,
    project_id: String,
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static
{
    move |reader, workspace_db_uri| {
        with_session(reader, workspace_db_uri, |clips| load_program(&target, &project_id, clips))
    }
}
/// The program with the drafted sound of one segment or cover, built where the player
/// is prepared. Its rendered units still play where the draft does not change them (a
/// changed unit has another fingerprint and plays from its sources).
pub fn draft_loader(
    target: ProjectDbTarget,
    project_id: String,
    draft: SoundDraft,
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static
{
    move |reader, workspace_db_uri| {
        with_session(reader, workspace_db_uri, |clips| load(&target, &project_id, clips, None, Some(&draft)))
    }
}

/// The program window with a transient cover, built where the player is prepared.
pub fn window_loader(
    target: ProjectDbTarget,
    project_id: String,
    window: (u64, u64),
    cover: TransientCover,
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static
{
    move |reader, workspace_db_uri| {
        with_session(reader, workspace_db_uri, |clips| {
            load_program_window(&target, &project_id, clips, window, &cover)
        })
    }
}

/// v5 `build_segments` + `program_playlist_input`: the active segments one after
/// another on the program axis, each with the covers that overlap it; a cover
/// without its virtual shot or timebase is not played (v5 `streamable`).
pub fn build_input(
    project_id: &str,
    story_rate: (u32, u32),
    segments: &[ProgramSegment],
    covers: &[ProgramCover],
    audio_channels: u16,
) -> Result<ProgramPlaylistBuildInput, String> {
    let program_timebase = timebase(story_rate.0, story_rate.1, "prica")?;
    let mut start = 0i64;
    let mut inputs = Vec::new();
    for segment in segments.iter().filter(|segment| segment.active) {
        // A source of twice or half the story rate is counted at the story rate (fields
        // or repeated frames, or every other frame; user 2026-10-09).
        let (num, den) = qnc_program_db::story_per_source(story_rate, (segment.fps_num, segment.fps_den))?;
        let frames = frame(segment.duration_frames)?;
        let record_range =
            ProgramFrameRange::new(start, start + frames).map_err(|error| error.message)?;
        let covers = covers
            .iter()
            .filter(|cover| {
                !cover.virtual_shot_id.trim().is_empty()
                    && (cover.program_start_frame as i64) < record_range.out_frame
                    && (cover.program_end_frame as i64) > record_range.in_frame
            })
            .map(|cover| cover_input(cover, story_rate))
            .collect::<Result<Vec<_>, _>>()?;
        let source_in = frame(segment.in_frame)? * num / den;
        inputs.push(ProgramSegmentInput {
            segment_id: segment.segment_id.clone(),
            kind: segment.kind.clone(),
            clip_id: segment.clip_id.clone(),
            virtual_shot_id: String::new(),
            record_range,
            source_range: FrameRange {
                source_in,
                source_out: source_in + frames,
                timebase: program_timebase,
            },
            a1_source_channel: segment.a1_source_channel,
            a1_level_db: segment.a1_level_db,
            a1_off: segment.a1_off,
            covers,
        });
        start += frames;
    }
    if inputs.is_empty() {
        return Err(qnc_ui_text::text("story.msg.program_is_empty").into());
    }
    Ok(ProgramPlaylistBuildInput {
        playlist_id: format!("program:{project_id}"),
        project_id: project_id.to_string(),
        revision: 0,
        program_timebase,
        audio_layout: ProgramAudioLayout::discrete(audio_channels)
            .map_err(|error| error.message)?,
        duration_frames: start,
        segments: inputs,
    })
}

/// A cover plays its source from its IN for its slot's length; where its clip ends first
/// (a slot made longer than the cover's source), the cover ends with its clip and the
/// segment under it shows the rest of the slot (user 2026-10-11: one cover may not keep
/// the whole story from opening). A cover with no picture left is not played.
fn fit_covers<R: ProgramMediaResolver>(
    covers: Vec<ProgramCover>,
    resolver: &mut R,
    story_rate: (u32, u32),
) -> Result<Vec<ProgramCover>, String> {
    let mut fitted = Vec::with_capacity(covers.len());
    for mut cover in covers {
        if cover.virtual_shot_id.trim().is_empty() || cover.fps_num == 0 || cover.fps_den == 0 {
            fitted.push(cover);
            continue;
        }
        let (num, den) = qnc_program_db::story_per_source(story_rate, (cover.fps_num, cover.fps_den))?;
        let length = u64::try_from(resolver.resolve(&cover.clip_id)?.duration_frames).unwrap_or(0);
        let source_in = cover.source_in_frame * num as u64 / den as u64;
        let left = length.saturating_sub(source_in);
        if left == 0 {
            continue;
        }
        cover.program_end_frame = cover.program_end_frame.min(cover.program_start_frame + left);
        fitted.push(cover);
    }
    Ok(fitted)
}

/// A cover over its slot; its source counted at the story rate, as long as the slot.
fn cover_input(cover: &ProgramCover, story_rate: (u32, u32)) -> Result<ProgramCoverInput, String> {
    let (num, den) = qnc_program_db::story_per_source(story_rate, (cover.fps_num, cover.fps_den))?;
    let record_range = ProgramFrameRange::new(
        frame(cover.program_start_frame)?,
        frame(cover.program_end_frame)?,
    )
    .map_err(|error| error.message)?;
    let source_in = frame(cover.source_in_frame)? * num / den;
    Ok(ProgramCoverInput {
        cover_id: cover.cover_id.clone(),
        clip_id: cover.clip_id.clone(),
        virtual_shot_id: cover.virtual_shot_id.clone(),
        source_range: FrameRange {
            source_in,
            source_out: source_in + (record_range.out_frame - record_range.in_frame),
            timebase: timebase(story_rate.0, story_rate.1, &cover.cover_id)?,
        },
        record_range,
        a2_source_channel: cover.a2_source_channel,
        a2_level_db: cover.a2_level_db,
        a2_off: cover.a2_off,
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
        .ok_or_else(|| qnc_ui_text::fill("story.msg.clip_no_picture", &[("clip", clip_id)]))?;
    let details = picture
        .streams
        .iter()
        .find(|stream| stream.index.as_ref().map(|i| i.value) == Some(video.stream_index))
        .and_then(|stream| match &stream.details {
            StreamDetails::Video(details) => Some(details),
            _ => None,
        })
        .ok_or_else(|| qnc_ui_text::fill("story.msg.clip_no_video_stream", &[("clip", clip_id)]))?;
    let known = |what: &'static str| qnc_ui_text::fill("story.msg.clip_missing_fact", &[("clip", clip_id), ("what", qnc_ui_text::text(what))]);
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
        u16::try_from(layout.audio_channels.len()).map_err(|_| known("story.fact.audio_channels"))?;
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
                .ok_or_else(|| known("story.fact.sample_rate"))
        });
        let rate = rates
            .next()
            .unwrap_or_else(|| Err(known("story.fact.sample_rate")))?;
        for other in rates {
            if other? != rate {
                return Err(qnc_ui_text::fill("story.msg.clip_mixed_sample_rates", &[("clip", clip_id)]));
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
        timecode_start: None,
    })
}

/// The original timecode of frame 0 of a clip from its stored record (user rule
/// 2026-10-01: proxy and original are linked by timecode). When the picture is the
/// proxy, its stored timecode must be the one of the original, or frame N of the proxy
/// would not be frame N of the original: that is a controlled error, never a guess.
pub fn timecode_start(clip_id: &str, input: &PreparedInput) -> Result<Option<i64>, String> {
    let Some(video) = input.layout.video.as_ref() else {
        return Ok(None);
    };
    let (Ok(num), Ok(den)) = (u64::try_from(video.timebase.fps_num), u64::try_from(video.timebase.fps_den)) else {
        return Ok(None);
    };
    let tags = |media: &MediaRepresentation| -> Vec<(String, String)> {
        media.tags.iter().map(|(key, fact)| (key.clone(), fact.value.clone())).collect()
    };
    let original = tags(input.audio_media());
    let Some(timecode) = qnc_source_timecode::SourceTimecode::from_tags(
        original.iter().map(|(key, value)| (key.as_str(), value.as_str())),
        (num, den),
    ) else {
        return Ok(None);
    };
    if input.representation == qnc_player_input::Representation::Proxy {
        let proxy = tags(input.media().map_err(|error| error.to_string())?);
        if timecode.proxy_matches(proxy.iter().map(|(key, value)| (key.as_str(), value.as_str()))) == Some(false) {
            return Err(qnc_ui_text::fill("story.msg.proxy_timecode_differs", &[("clip", clip_id)]));
        }
    }
    timecode
        .start_frame
        .map(|frames| i64::try_from(frames).map_err(|_| qnc_ui_text::text("story.msg.timecode_out_of_range").to_string()))
        .transpose()
}

struct PreparedResolver<'a, C> {
    clips: &'a C,
    prepared: BTreeMap<String, PreparedInput>,
    story_rate: (u32, u32),
}

impl<C: ClipInputs> PreparedResolver<'_, C> {
    fn prepared(&mut self, clip_id: &str) -> Result<&PreparedInput, String> {
        if !self.prepared.contains_key(clip_id) {
            let input = self.clips.prepared(clip_id)?;
            self.prepared.insert(clip_id.to_string(), input);
        }
        self.prepared
            .get(clip_id)
            .ok_or_else(|| qnc_ui_text::fill("story.msg.clip_not_prepared", &[("clip", clip_id)]))
    }
}

impl<C: ClipInputs> ProgramMediaResolver for PreparedResolver<'_, C> {
    fn resolve(&mut self, clip_id: &str) -> Result<ResolvedProgramMedia, String> {
        let input = self.prepared(clip_id)?;
        let picture = input.media().map_err(|error| error.to_string())?;
        let timecode = timecode_start(clip_id, input)?;
        let mut media = resolved_media(clip_id, picture, input.audio_media(), &input.layout)?;
        media.timecode_start = timecode;
        at_story_rate(&mut media, self.story_rate)?;
        Ok(media)
    }
}

/// A clip of twice or half the story rate described at the story rate, as the player
/// plays it (user 2026-10-09: 2:1 only): its frames and the timecode of its frame 0 are
/// counted in story frames.
pub fn at_story_rate(media: &mut ResolvedProgramMedia, story_rate: (u32, u32)) -> Result<(), String> {
    let own = (
        u32::try_from(media.timebase.fps_num).map_err(|_| qnc_ui_text::text("story.msg.bad_clip_timebase"))?,
        u32::try_from(media.timebase.fps_den).map_err(|_| qnc_ui_text::text("story.msg.bad_clip_timebase"))?,
    );
    let (num, den) = qnc_program_db::story_per_source(story_rate, own)?;
    if (num, den) != (1, 1) {
        media.timebase = timebase(story_rate.0, story_rate.1, &media.media.clip_id)?;
        media.duration_frames = media.duration_frames * num / den;
        media.timecode_start = media.timecode_start.map(|start| start * num / den);
    }
    Ok(())
}

fn timebase(num: u32, den: u32, label: &str) -> Result<FrameTimebase, String> {
    FrameTimebase::new(i64::from(num), i64::from(den))
        .map_err(|_| qnc_ui_text::fill("story.msg.no_original_timebase", &[("source", label)]))
}

fn frame(value: u64) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| qnc_ui_text::text("story.msg.frame_out_of_range").to_string())
}

#[cfg(test)]
mod tests;

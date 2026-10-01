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
    pub covers: Vec<ProgramCoverInput>,
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
    pub active: bool,
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
        return Err("Program input je prazan.".into());
    }
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

/// Applies a transient cutaway to an already flattened program snapshot.
///
/// The operation preserves the program A1 bus and replaces the visible video
/// and A2 bus only inside `record_range`. It does not change editorial data.
pub fn apply_transient_program_overlay(
    playlist: &FlatProgramPlaylist,
    overlay: &ProgramTransientOverlayInput,
) -> Result<FlatProgramPlaylist, String> {
    playlist.validate().map_err(|error| error.message)?;
    let overlay_range = ProgramFrameRange::new(
        overlay.record_range.in_frame,
        overlay.record_range.out_frame,
    )
    .map_err(|error| error.message)?;
    if overlay_range.out_frame > playlist.duration_frames {
        return Err(format!(
            "Transient overlay završava izvan programa: {} > {}.",
            overlay_range.out_frame, playlist.duration_frames
        ));
    }
    if overlay.source.video_layer != Some(ProgramVideoLayer::Cover) {
        return Err("Transient overlay mora biti cover video source.".into());
    }
    if overlay.source.source_range.frame_len() != overlay_range.frame_len() {
        return Err("Transient overlay source i program raspon nemaju istu duljinu.".into());
    }
    let replaced_audio_outputs = overlay
        .source
        .audio_routes
        .iter()
        .map(|route| route.output_channel)
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    for item in &playlist.items {
        let item_start = item.record_range.in_frame;
        let item_end = item.record_range.out_frame;
        let overlap_start = item_start.max(overlay_range.in_frame);
        let overlap_end = item_end.min(overlay_range.out_frame);
        if overlap_end <= overlap_start {
            items.push(item.clone());
            continue;
        }
        if item_start < overlap_start {
            items.push(trim_program_item(item, item_start, overlap_start)?);
        }
        let mut sources = item
            .sources
            .iter()
            .filter_map(|source| {
                retained_audio_source(
                    source,
                    item.record_range,
                    overlap_start,
                    overlap_end,
                    &replaced_audio_outputs,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        sources.push(trim_flat_source(
            &overlay.source,
            overlay_range,
            overlap_start,
            overlap_end,
        )?);
        items.push(FlatProgramItem {
            item_id: item_id(overlap_start, overlap_end),
            record_range: ProgramFrameRange::new(overlap_start, overlap_end)
                .map_err(|error| error.message)?,
            sources,
        });
        if overlap_end < item_end {
            items.push(trim_program_item(item, overlap_end, item_end)?);
        }
    }
    let result = FlatProgramPlaylist {
        items,
        ..playlist.clone()
    };
    result.validate().map_err(|error| error.message)?;
    Ok(result)
}

/// A playlist for an exact program-frame window (export of a part). Record
/// frames are rebased to zero; linked source ranges stay absolute.
pub fn build_program_frame_window(
    playlist: &FlatProgramPlaylist,
    record_range: ProgramFrameRange,
) -> Result<FlatProgramPlaylist, String> {
    playlist.validate().map_err(|error| error.message)?;
    let record_range = ProgramFrameRange::new(record_range.in_frame, record_range.out_frame)
        .map_err(|error| error.message)?;
    if record_range.out_frame > playlist.duration_frames {
        return Err(format!(
            "Program window zavrsava izvan programa: {} > {}.",
            record_range.out_frame, playlist.duration_frames
        ));
    }
    let mut items = Vec::new();
    for item in &playlist.items {
        let overlap_in = item.record_range.in_frame.max(record_range.in_frame);
        let overlap_out = item.record_range.out_frame.min(record_range.out_frame);
        if overlap_out <= overlap_in {
            continue;
        }
        let mut window_item = trim_program_item(item, overlap_in, overlap_out)?;
        window_item.record_range = ProgramFrameRange::new(
            overlap_in.saturating_sub(record_range.in_frame),
            overlap_out.saturating_sub(record_range.in_frame),
        )
        .map_err(|error| error.message)?;
        window_item.item_id = item_id(
            window_item.record_range.in_frame,
            window_item.record_range.out_frame,
        );
        items.push(window_item);
    }
    let result = FlatProgramPlaylist {
        playlist_id: format!(
            "{}:window:{}-{}",
            playlist.playlist_id, record_range.in_frame, record_range.out_frame
        ),
        duration_frames: record_range.frame_len(),
        items,
        ..playlist.clone()
    };
    result.validate().map_err(|error| error.message)?;
    Ok(result)
}

fn trim_program_item(
    item: &FlatProgramItem,
    record_in: i64,
    record_out: i64,
) -> Result<FlatProgramItem, String> {
    Ok(FlatProgramItem {
        item_id: item_id(record_in, record_out),
        record_range: ProgramFrameRange::new(record_in, record_out)
            .map_err(|error| error.message)?,
        sources: item
            .sources
            .iter()
            .map(|source| trim_flat_source(source, item.record_range, record_in, record_out))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn retained_audio_source(
    source: &FlatProgramSource,
    item_range: ProgramFrameRange,
    record_in: i64,
    record_out: i64,
    replaced_audio_outputs: &[u16],
) -> Option<Result<FlatProgramSource, String>> {
    let audio_routes = source
        .audio_routes
        .iter()
        .copied()
        .filter(|route| !replaced_audio_outputs.contains(&route.output_channel))
        .collect::<Vec<_>>();
    if audio_routes.is_empty() {
        return None;
    }
    Some(
        trim_flat_source(source, item_range, record_in, record_out).map(|mut source| {
            source.video_layer = None;
            source.source_video_format = None;
            source.audio_routes = audio_routes;
            source
        }),
    )
}

fn trim_flat_source(
    source: &FlatProgramSource,
    origin_record_range: ProgramFrameRange,
    record_in: i64,
    record_out: i64,
) -> Result<FlatProgramSource, String> {
    let offset = record_in.saturating_sub(origin_record_range.in_frame);
    let source_in = source.source_range.source_in.saturating_add(offset);
    let source_out = source_in.saturating_add(record_out.saturating_sub(record_in));
    if source_out > source.source_range.source_out {
        return Err(format!(
            "Source '{}' nema dovoljno frameova za program raspon {}..{}.",
            source.source_id, record_in, record_out
        ));
    }
    let mut source = source.clone();
    source.source_range.source_in = source_in;
    source.source_range.source_out = source_out;
    Ok(source)
}

fn flat_items_for_segment(
    segment: &ProgramSegmentInput,
    resolver: &mut impl ProgramMediaResolver,
) -> Result<Vec<FlatProgramItem>, String> {
    let clip_id = segment.clip_id.trim();
    if clip_id.is_empty() {
        return Err(format!("Segment '{}' nema clip_id.", segment.segment_id));
    }
    let base_media = resolver.resolve(clip_id)?;
    let is_off = segment.kind.trim().eq_ignore_ascii_case("offovi");
    let has_base_audio = is_off || base_media.has_audio;
    let segment_start = segment.record_range.in_frame;
    let segment_end = segment.record_range.out_frame;
    let mut items = Vec::new();

    let mut covers = segment
        .covers
        .iter()
        .filter(|cover| cover_is_available(cover, segment.record_range))
        .collect::<Vec<_>>();
    covers.sort_by_key(|cover| {
        (
            cover.record_range.in_frame,
            cover.record_range.out_frame,
            cover.cover_id.as_str(),
        )
    });

    let base = BaseSegment {
        segment,
        media: &base_media,
        has_audio: has_base_audio,
        is_off,
    };
    let mut cursor = segment_start;
    for cover in covers {
        let cover_start = cover.record_range.in_frame.max(segment_start).max(cursor);
        let cover_end = cover_available_record_end(cover)
            .min(segment_end)
            .max(cover_start);
        if cover_end <= cover_start {
            continue;
        }
        if cursor < cover_start {
            items.push(base.item(cursor, cover_start)?);
        }
        let mut sources = Vec::new();
        if has_base_audio {
            sources.push(base.audio_source(cover_start, cover_end)?);
        }
        sources.extend(cover_sources(cover, resolver, cover_start, cover_end)?);
        items.push(FlatProgramItem {
            item_id: item_id(cover_start, cover_end),
            record_range: ProgramFrameRange::new(cover_start, cover_end)
                .map_err(|error| error.message)?,
            sources,
        });
        cursor = cover_end;
    }
    if cursor < segment_end {
        items.push(base.item(cursor, segment_end)?);
    }
    Ok(items)
}

/// The base of a segment: Ton picture and the A1 sound of Ton or Off.
struct BaseSegment<'a> {
    segment: &'a ProgramSegmentInput,
    media: &'a ResolvedProgramMedia,
    has_audio: bool,
    is_off: bool,
}

impl BaseSegment<'_> {
    fn item(&self, record_in: i64, record_out: i64) -> Result<FlatProgramItem, String> {
        let mut sources = Vec::new();
        if !self.is_off {
            sources.push(self.source("base_video", true, false, record_in, record_out)?);
        }
        if self.has_audio {
            sources.push(self.audio_source(record_in, record_out)?);
        }
        Ok(FlatProgramItem {
            item_id: item_id(record_in, record_out),
            record_range: ProgramFrameRange::new(record_in, record_out)
                .map_err(|error| error.message)?,
            sources,
        })
    }

    fn audio_source(&self, record_in: i64, record_out: i64) -> Result<FlatProgramSource, String> {
        self.source("base_audio", false, true, record_in, record_out)
    }

    fn source(
        &self,
        source_kind: &str,
        has_video: bool,
        has_audio: bool,
        record_in: i64,
        record_out: i64,
    ) -> Result<FlatProgramSource, String> {
        let (segment, media) = (self.segment, self.media);
        let source_range = source_range_for_record_chunk(
            segment.source_range,
            segment.record_range.in_frame,
            record_in,
            record_out,
        );
        if source_range.timebase != media.timebase {
            return Err(format!(
                "Clip '{}' editorial timebase {}/{} ne odgovara spremljenom probe timebaseu {}/{}.",
                segment.clip_id,
                source_range.timebase.fps_num,
                source_range.timebase.fps_den,
                media.timebase.fps_num,
                media.timebase.fps_den
            ));
        }
        Ok(FlatProgramSource {
            source_id: format!("part:{}:{source_kind}", segment.segment_id),
            clip_id: segment.clip_id.trim().to_string(),
            virtual_shot_id: segment.virtual_shot_id.clone(),
            media: if has_video {
                media.media.clone()
            } else {
                media.audio_media.clone()
            },
            source_range,
            source_duration_frames: media.duration_frames,
            video_layer: has_video.then_some(ProgramVideoLayer::Base),
            source_video_format: has_video.then_some(media.video_format).flatten(),
            audio_routes: audio_routes(
                has_audio,
                segment.a1_source_channel,
                PROGRAM_AUDIO_OUTPUT_A1,
            ),
            source_audio_channels: if has_audio { media.audio_channels } else { 0 },
            source_audio_format: has_audio.then_some(media.audio_format).flatten(),
            source_timecode_start: media.timecode_start,
        })
    }
}

/// The cover picture (Cover layer) and its sound on A2. One source as in v5 when
/// picture and sound share a media; a proxy picture has its own source, so the
/// sound stays from the original.
fn cover_sources(
    cover: &ProgramCoverInput,
    resolver: &mut impl ProgramMediaResolver,
    record_in: i64,
    record_out: i64,
) -> Result<Vec<FlatProgramSource>, String> {
    let clip_id = cover.clip_id.trim();
    if clip_id.is_empty() {
        return Err(format!("Pokrivalica '{}' nema clip_id.", cover.cover_id));
    }
    let media = resolver.resolve(clip_id)?;
    let source_range = source_range_for_record_chunk(
        cover.source_range,
        cover.record_range.in_frame,
        record_in,
        record_out,
    );
    if source_range.timebase != media.timebase {
        return Err(format!(
            "Pokrivalica '{}' editorial timebase {}/{} ne odgovara spremljenom probe timebaseu {}/{}.",
            cover.cover_id,
            source_range.timebase.fps_num,
            source_range.timebase.fps_den,
            media.timebase.fps_num,
            media.timebase.fps_den
        ));
    }
    let picture = FlatProgramSource {
        source_id: format!("cover:{}", cover.cover_id),
        clip_id: clip_id.to_string(),
        virtual_shot_id: cover.virtual_shot_id.clone(),
        media: media.media.clone(),
        source_range,
        source_duration_frames: media.duration_frames,
        video_layer: Some(ProgramVideoLayer::Cover),
        source_video_format: media.video_format,
        audio_routes: Vec::new(),
        source_audio_channels: 0,
        source_audio_format: None,
        source_timecode_start: media.timecode_start,
    };
    if !media.has_audio {
        return Ok(vec![picture]);
    }
    let routes = audio_routes(true, cover.a2_source_channel, PROGRAM_AUDIO_OUTPUT_A2);
    if media.audio_media == media.media {
        return Ok(vec![FlatProgramSource {
            audio_routes: routes,
            source_audio_channels: media.audio_channels,
            source_audio_format: media.audio_format,
            ..picture
        }]);
    }
    let sound = FlatProgramSource {
        source_id: format!("cover:{}:audio", cover.cover_id),
        media: media.audio_media,
        video_layer: None,
        source_video_format: None,
        audio_routes: routes,
        source_audio_channels: media.audio_channels,
        source_audio_format: media.audio_format,
        ..picture.clone()
    };
    Ok(vec![picture, sound])
}

fn audio_routes(
    has_audio: bool,
    source_channel: u16,
    output_channel: u16,
) -> Vec<ProgramAudioRoute> {
    if !has_audio {
        return Vec::new();
    }
    vec![ProgramAudioRoute {
        source_channel,
        output_channel,
    }]
}

fn cover_is_available(cover: &ProgramCoverInput, segment: ProgramFrameRange) -> bool {
    let cover_end = cover_available_record_end(cover);
    cover.active
        && cover_end > segment.in_frame
        && cover.record_range.in_frame < segment.out_frame
        && cover_end > cover.record_range.in_frame
}

fn cover_available_record_end(cover: &ProgramCoverInput) -> i64 {
    let source_len = cover.source_range.frame_len().max(0);
    if source_len <= 0 {
        return cover.record_range.in_frame;
    }
    cover
        .record_range
        .in_frame
        .saturating_add(source_len)
        .min(cover.record_range.out_frame)
}

fn source_range_for_record_chunk(
    source_range: FrameRange,
    origin_record_in: i64,
    record_in: i64,
    record_out: i64,
) -> FrameRange {
    let source_in = source_range.source_in.max(0);
    let source_out = source_range.source_out.max(source_in + 1);
    let offset = record_in
        .max(origin_record_in)
        .saturating_sub(origin_record_in.max(0));
    let start = source_in.saturating_add(offset).min(source_out - 1);
    let span = record_out
        .max(record_in + 1)
        .saturating_sub(record_in.max(0))
        .max(1);
    let end = start.saturating_add(span).min(source_out).max(start + 1);
    FrameRange {
        source_in: start,
        source_out: end,
        timebase: source_range.timebase,
    }
}

fn item_id(record_in: i64, record_out: i64) -> String {
    format!(
        "item:{}-{}",
        record_in.max(0),
        record_out.max(record_in + 1)
    )
}

#[cfg(test)]
mod tests;

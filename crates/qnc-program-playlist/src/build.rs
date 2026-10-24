use super::*;

pub(crate) fn flat_items_for_segment(
    segment: &ProgramSegmentInput,
    resolver: &mut impl ProgramMediaResolver,
) -> Result<Vec<FlatProgramItem>, String> {
    let clip_id = segment.clip_id.trim();
    if clip_id.is_empty() {
        return Err(format!("Segment '{}' has no clip.", segment.segment_id));
    }
    let base_media = resolver.resolve(clip_id)?;
    let is_off = segment.kind.trim().eq_ignore_ascii_case("offovi");
    // A1 switched off (by the user, or a lead-in/lead-out) has no sound to route: an Off with
    // A1 off is black and silence (live 2026-10-24: its empty sound refused the whole story).
    let has_base_audio = (is_off || base_media.has_audio) && !segment.a1_off;
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
pub(crate) struct BaseSegment<'a> {
    pub(crate) segment: &'a ProgramSegmentInput,
    pub(crate) media: &'a ResolvedProgramMedia,
    pub(crate) has_audio: bool,
    pub(crate) is_off: bool,
}

impl BaseSegment<'_> {
    pub(crate) fn item(&self, record_in: i64, record_out: i64) -> Result<FlatProgramItem, String> {
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

    pub(crate) fn audio_source(&self, record_in: i64, record_out: i64) -> Result<FlatProgramSource, String> {
        self.source("base_audio", false, true, record_in, record_out)
    }

    pub(crate) fn source(
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
                "Clip '{}': the editorial timebase {}/{} does not match the stored probe timebase {}/{}.",
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
                has_audio && !segment.a1_off,
                segment.a1_source_channel,
                PROGRAM_AUDIO_OUTPUT_A1,
                segment.a1_level_db,
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
pub(crate) fn cover_sources(
    cover: &ProgramCoverInput,
    resolver: &mut impl ProgramMediaResolver,
    record_in: i64,
    record_out: i64,
) -> Result<Vec<FlatProgramSource>, String> {
    let clip_id = cover.clip_id.trim();
    if clip_id.is_empty() {
        return Err(format!("B-roll '{}' has no clip.", cover.cover_id));
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
            "B-roll '{}': the editorial timebase {}/{} does not match the stored probe timebase {}/{}.",
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
    if !media.has_audio || cover.a2_off {
        return Ok(vec![picture]); // A2 silent: no sound source (user 2026-10-16)
    }
    let routes = audio_routes(!cover.a2_off, cover.a2_source_channel, PROGRAM_AUDIO_OUTPUT_A2, cover.a2_level_db);
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

/// The route of a lane that is heard (it has sound and is not off); none otherwise.
pub(crate) fn audio_routes(
    heard: bool,
    source_channel: u16,
    output_channel: u16,
    level_db: i8,
) -> Vec<ProgramAudioRoute> {
    if !heard {
        return Vec::new();
    }
    vec![ProgramAudioRoute {
        source_channel,
        output_channel,
        level_db,
        fade_in: None,
        fade_out: None,
    }]
}

pub(crate) fn cover_is_available(cover: &ProgramCoverInput, segment: ProgramFrameRange) -> bool {
    let cover_end = cover_available_record_end(cover);
    cover.active
        && cover_end > segment.in_frame
        && cover.record_range.in_frame < segment.out_frame
        && cover_end > cover.record_range.in_frame
}

pub(crate) fn cover_available_record_end(cover: &ProgramCoverInput) -> i64 {
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

pub(crate) fn source_range_for_record_chunk(
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

pub(crate) fn item_id(record_in: i64, record_out: i64) -> String {
    format!(
        "item:{}-{}",
        record_in.max(0),
        record_out.max(record_in + 1)
    )
}

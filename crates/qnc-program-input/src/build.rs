use super::*;

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
            v_lead_frames: i64::from(segment.v_lead_frames),
            v_tail_frames: i64::from(segment.v_tail_frames),
            a1_lead_frames: i64::from(segment.a1_lead_frames),
            a1_tail_frames: i64::from(segment.a1_tail_frames),
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
        a2_crossfade_frames: 0,
    })
}

/// A cover plays its source from its IN for its slot's length; where its clip ends first
/// (a slot made longer than the cover's source), the cover ends with its clip and the
/// segment under it shows the rest of the slot (user 2026-10-11: one cover may not keep
/// the whole story from opening). A cover with no picture left is not played.
pub(crate) fn fit_covers<R: ProgramMediaResolver>(
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
pub(crate) fn cover_input(cover: &ProgramCover, story_rate: (u32, u32)) -> Result<ProgramCoverInput, String> {
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
        a2_lead_frames: i64::from(cover.a2_lead_frames),
        a2_tail_frames: i64::from(cover.a2_tail_frames),
    })
}

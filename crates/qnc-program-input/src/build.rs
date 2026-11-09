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
    // A compound statement is one segment of the story and its parts in the program.
    let expanded = qnc_program_db::expand_compounds(segments, story_rate)?;
    // A cover of the story is over the picture: it wins over the in-between pictures under it.
    let smooths: Vec<_> = expanded
        .smooths
        .iter()
        .filter(|smooth| !covers.iter().any(|cover| (cover.program_start_frame as i64) < smooth.program.1 && (cover.program_end_frame as i64) > smooth.program.0))
        .collect();
    for segment in &expanded.segments {
        // A source of twice or half the story rate is counted at the story rate (fields
        // or repeated frames, or every other frame; user 2026-10-09).
        let (num, den) = qnc_program_db::story_per_source(story_rate, (segment.fps_num, segment.fps_den))?;
        let frames = frame(segment.duration_frames)?;
        let record_range =
            ProgramFrameRange::new(start, start + frames).map_err(|error| error.message)?;
        let mut over = covers
            .iter()
            .filter(|cover| {
                !cover.virtual_shot_id.trim().is_empty()
                    && (cover.program_start_frame as i64) < record_range.out_frame
                    && (cover.program_end_frame as i64) > record_range.in_frame
            })
            .map(|cover| cover_input(cover, story_rate))
            .collect::<Result<Vec<_>, _>>()?;
        for smooth in smooths.iter().filter(|smooth| smooth.program.0 < record_range.out_frame && smooth.program.1 > record_range.in_frame) {
            over.push(smooth_cover(smooth, program_timebase)?);
        }
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
            covers: over,
            v_lead_frames: i64::from(segment.v_lead_frames),
            v_tail_frames: i64::from(segment.v_tail_frames),
            a1_lead_frames: i64::from(segment.a1_lead_frames),
            a1_tail_frames: i64::from(segment.a1_tail_frames),
            a2_source_channel: segment.a2_source_channel,
            a2_level_db: segment.a2_level_db,
            a2_off: segment.a2_off,
            a2_lead_frames: i64::from(segment.a2_lead_frames),
            a2_tail_frames: i64::from(segment.a2_tail_frames),
            a1_points: points(&segment.level_keys, num, den),
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

/// The in-between pictures of a smooth cut as a cover without sound.
fn smooth_cover(smooth: &qnc_program_db::SmoothPicture, timebase: FrameTimebase) -> Result<ProgramCoverInput, String> {
    Ok(ProgramCoverInput {
        cover_id: smooth.id.clone(),
        clip_id: smooth.clip_id.clone(),
        virtual_shot_id: String::new(),
        record_range: ProgramFrameRange::new(smooth.program.0, smooth.program.1).map_err(|error| error.message)?,
        source_range: FrameRange { source_in: smooth.clip.0, source_out: smooth.clip.1, timebase },
        a2_source_channel: 0,
        a2_level_db: 0,
        a2_off: true,
        active: true,
        a2_lead_frames: 0,
        a2_tail_frames: 0,
        a2_points: Vec::new(),
    })
}

/// The joins of the parts of compound statements: the sounds cross or cut as each join says.
pub(crate) fn apply_joins(playlist: &mut qnc_program_playlist::FlatProgramPlaylist, segments: &[ProgramSegment], story_rate: (u32, u32), crossfade: impl Into<i64>) -> Result<(), String> {
    let crossfade: i64 = crossfade.into();
    let joins: Vec<qnc_program_playlist::A1Join> = qnc_program_db::expand_compounds(segments, story_rate)?
        .joins
        .iter()
        .map(|join| qnc_program_playlist::A1Join { end: join.end, start: join.start, frames: if join.crossfade { crossfade } else { 0 } })
        .collect();
    if joins.is_empty() {
        return Ok(());
    }
    qnc_program_playlist::join_a1(&mut playlist.items, &joins).map_err(|error| error.to_string())
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
        a2_points: points(&cover.level_keys, num, den),
    })
}

/// The level curve of a segment or cover (frames of its clip) on its source counted at the
/// story rate (docs/98 2.1).
fn points(keys: &[qnc_program_db::LevelKey], num: i64, den: i64) -> Vec<qnc_program_playlist::LevelPoint> {
    keys.iter()
        .map(|key| qnc_program_playlist::LevelPoint { source_frame: i64::try_from(key.frame).unwrap_or(i64::MAX / 4) * num / den, db10: key.db10 })
        .collect()
}

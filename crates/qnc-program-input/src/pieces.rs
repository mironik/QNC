use super::*;

/// One part of a SubSegment (user 2026-10-29): frames `[from, to)` of a clip, the pause of
/// silence before it (frames, none before the first part) and how its sound joins the part
/// before (a crossfade of the project's frames or a cut).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PieceInput {
    pub clip_id: String,
    pub from_frame: u64,
    pub to_frame: u64,
    pub gap_before: u64,
    pub crossfade: bool,
    /// The part's own A1: camera channel (`None`: the one kept for its clip), level in dB and
    /// off (user 2026-10-29); `None`: the sound kept for its clip.
    pub a1: Option<(Option<u16>, i8, bool)>,
}

/// The program of a SubSegment: its parts one after another on V1 and A1 (a pause is black
/// and silence), each clip heard on the A1 channel and level kept for it on the source
/// timeline (`qnc-clip-sound`, channel 1 as recorded without one). Nothing is written.
pub fn load_pieces(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &impl ClipInputs,
    pieces: &[PieceInput],
) -> Result<ProgramInput, String> {
    let first = pieces.first().ok_or(qnc_ui_text::text("story.msg.program_is_empty"))?;
    let mut resolver = PreparedResolver { clips, prepared: BTreeMap::new(), story_rate: (0, 1) };
    let input = resolver.prepared(&first.clip_id)?;
    let own = resolved_media(&first.clip_id, input.media().map_err(|error| error.to_string())?, input.audio_media(), &input.layout)?;
    let channels = input.project_audio.channels;
    let rate = (
        u32::try_from(own.timebase.fps_num).map_err(|_| qnc_ui_text::text("story.msg.bad_clip_timebase"))?,
        u32::try_from(own.timebase.fps_den).map_err(|_| qnc_ui_text::text("story.msg.bad_clip_timebase"))?,
    );
    resolver.story_rate = rate;
    let program_timebase = timebase(rate.0, rate.1, "subsegment")?;
    let crossfade = i64::from(clips.crossfade_frames()?);
    let (mut segments, mut joins, mut start) = (Vec::new(), Vec::new(), 0i64);
    for (index, piece) in pieces.iter().enumerate() {
        let gap = if index == 0 { 0 } else { frame(piece.gap_before)? };
        if gap > 0 {
            // The pause: an Off with A1 off, black and silence.
            segments.push(gap_segment(index, &pieces[index - 1].clip_id, (start, start + gap), program_timebase)?);
        }
        let media = resolver.resolve(&piece.clip_id)?;
        let own = (u32::try_from(media.timebase.fps_num).unwrap_or(0), u32::try_from(media.timebase.fps_den).unwrap_or(0));
        let (num, den) = qnc_program_db::story_per_source(rate, own)?;
        let source_in = frame(piece.from_frame)? * num / den;
        let frames = (frame(piece.to_frame)? - frame(piece.from_frame)?) * num / den;
        if frames <= 0 {
            return Err(qnc_ui_text::text("script.sot_marks").into());
        }
        // The part's own A1 (user 2026-10-29), else the sound kept for its clip.
        let kept = qnc_clip_sound::read(target, &piece.clip_id)?.map(|sound| (sound.a1.channel, sound.a1.level_db, sound.a1.off));
        let sound = match piece.a1 {
            Some((channel, level, off)) => Some((channel.or(kept.map(|kept| kept.0)).unwrap_or(0), level, off)),
            None => kept,
        };
        let record_range = ProgramFrameRange::new(start + gap, start + gap + frames).map_err(|error| error.message)?;
        if index > 0 {
            joins.push(qnc_program_playlist::A1Join { end: start, start: start + gap, frames: if piece.crossfade { crossfade } else { 0 } });
        }
        segments.push(ProgramSegmentInput {
            segment_id: format!("piece-{index}"),
            kind: "tonovi".into(),
            clip_id: piece.clip_id.clone(),
            virtual_shot_id: String::new(),
            record_range,
            source_range: FrameRange { source_in, source_out: source_in + frames, timebase: program_timebase },
            a1_source_channel: sound.map_or(0, |sound| sound.0),
            a1_level_db: sound.map_or(0, |sound| sound.1),
            a1_off: sound.is_some_and(|sound| sound.2),
            covers: Vec::new(),
            v_lead_frames: 0,
            a1_lead_frames: 0,
            a1_tail_frames: 0,
        });
        start = record_range.out_frame;
    }
    let build = ProgramPlaylistBuildInput {
        playlist_id: format!("subsegment:{project_id}"),
        project_id: project_id.to_string(),
        revision: 0,
        program_timebase,
        audio_layout: ProgramAudioLayout::discrete(channels).map_err(|error| error.message)?,
        duration_frames: start,
        segments,
        a2_crossfade_frames: 0,
    };
    let mut playlist = build_flat_program_playlist(&build, &mut resolver)?;
    qnc_program_playlist::join_a1(&mut playlist.items, &joins)?;
    playlist.validate().map_err(|error| error.message)?;
    ProgramInput::new(playlist, resolver.prepared).map_err(|error| error.to_string())
}

fn gap_segment(index: usize, clip_id: &str, (from, to): (i64, i64), timebase: FrameTimebase) -> Result<ProgramSegmentInput, String> {
    Ok(ProgramSegmentInput {
        segment_id: format!("pause-{index}"),
        kind: "offovi".into(),
        clip_id: clip_id.to_string(),
        virtual_shot_id: String::new(),
        record_range: ProgramFrameRange::new(from, to).map_err(|error| error.message)?,
        source_range: FrameRange { source_in: 0, source_out: to - from, timebase },
        a1_source_channel: 0,
        a1_level_db: 0,
        a1_off: true,
        covers: Vec::new(),
        v_lead_frames: 0,
        a1_lead_frames: 0,
        a1_tail_frames: 0,
    })
}

/// The program of a SubSegment, built where the player is prepared.
pub fn pieces_loader(
    target: ProjectDbTarget,
    project_id: String,
    pieces: Vec<PieceInput>,
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static {
    move |reader, workspace_db_uri| with_session(reader, workspace_db_uri, |clips| load_pieces(&target, &project_id, clips, &pieces))
}

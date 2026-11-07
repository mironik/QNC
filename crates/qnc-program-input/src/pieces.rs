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
    /// The J and L of the join before it (user 2026-11-03), in its clip's frames: its sound
    /// starts `sound_lead` before its picture, the part before's goes on `sound_tail` after its
    /// OUT. Either one crosses the two sounds as at a story's cut (`cross_a1`), in place of the
    /// join's crossfade; after a pause there is nothing to cross.
    /// Negative: a hole of silence at the join (the sound after starts after the picture cut,
    /// the sound before ends before it).
    pub sound_lead: i64,
    pub sound_tail: i64,
    /// The J and L of the join's picture (user 2026-11-04, as at a story's cut), in its clip's
    /// frames: its picture starts `picture_lead` before the join, the part before's goes on
    /// `picture_tail` after it; the sound stays. Nothing after a pause.
    pub picture_lead: u64,
    pub picture_tail: u64,
    /// The level curve of the part's A1 (user 2026-11-07, as a segment's): points at frames of
    /// its clip, tenths of a dB added to its level.
    pub level_keys: Vec<(u64, i16)>,
}

/// The program of a SubSegment: its parts one after another on V1 and A1 (a pause is black
/// and silence), each clip heard on the A1 channel and level kept for it on the source
/// timeline (`qnc-clip-sound`, channel 1 as recorded without one). Nothing is written.
pub fn load_pieces(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &PlayerClipInputs<'_>,
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
    // The story frames per frame of the part before (its L is counted in its own clip).
    let mut before_scale = (1i64, 1i64);
    for (index, piece) in pieces.iter().enumerate() {
        // A pause may come before the first part too (a draft story that starts with an OFF
        // not recorded yet, user 2026-10-30).
        let gap = frame(piece.gap_before)?;
        if gap > 0 {
            // The pause: an Off with A1 off, black and silence.
            segments.push(gap_segment(index, &pieces[index.saturating_sub(1)].clip_id, (start, start + gap), program_timebase)?);
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
        // The J and L of the join (only where two parts meet, no pause): the sounds cross as at
        // a story's cut, in place of the join's crossfade.
        let (lead, tail) = if index > 0 && gap == 0 { (piece.sound_lead * num / den, piece.sound_tail * before_scale.0 / before_scale.1) } else { (0, 0) };
        // A negative side is a hole in the sound (user 2026-11-04), the playlist's as at every
        // story's cut; a positive side crosses.
        if lead != 0 || tail != 0 {
            if let Some(before) = segments.last_mut() {
                before.a1_tail_frames = tail;
            }
        }
        // The J and L of the picture, as at a story's cut (`picture_lead.rs` of the playlist).
        let joined = index > 0 && gap == 0;
        let picture_lead = if joined { frame(piece.picture_lead)? * num / den } else { 0 };
        if joined && piece.picture_tail > 0 {
            if let Some(before) = segments.last_mut() {
                before.v_tail_frames = (frame(piece.picture_tail)? * before_scale.0 / before_scale.1).try_into().unwrap_or_default();
            }
        }
        if index > 0 {
            let shaped = piece.sound_lead != 0 || piece.sound_tail != 0;
            joins.push(qnc_program_playlist::A1Join { end: start, start: start + gap, frames: if piece.crossfade && !shaped { crossfade } else { 0 } });
        }
        before_scale = (num, den);
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
            v_lead_frames: picture_lead.try_into().unwrap_or_default(),
            v_tail_frames: 0,
            a1_lead_frames: lead,
            a1_tail_frames: 0,
            a2_source_channel: 0,
            a2_level_db: 0,
            a2_off: false,
            a2_lead_frames: 0,
            a2_tail_frames: 0,
            // The part's level curve, on the story's frames (as a segment's points).
            a1_points: piece
                .level_keys
                .iter()
                .map(|(at, db10)| qnc_program_playlist::LevelPoint { source_frame: i64::try_from(*at).unwrap_or(i64::MAX / 4) * num / den, db10: *db10 })
                .collect(),
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
        v_tail_frames: 0,
        a1_lead_frames: 0,
        a1_tail_frames: 0,
        a2_source_channel: 0,
        a2_level_db: 0,
        a2_off: false,
        a2_lead_frames: 0,
        a2_tail_frames: 0,
        a1_points: Vec::new(),
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

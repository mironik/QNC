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
    /// The smooth cut of the join before it (user 2026-11-08): the clip of its in-between
    /// pictures and their count. They overwrite the picture around the cut as a cover does (half
    /// before it, half after), with no sound of their own: the length and the sound stay.
    pub smooth: Option<(String, u64)>,
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
    let crossfade = clips.crossfade_frames()?;
    // One layout for a SubSegment and a compound segment of a story (user 2026-11-09).
    let mut parts = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let media = resolver.resolve(&piece.clip_id)?;
        let own = (u32::try_from(media.timebase.fps_num).unwrap_or(0), u32::try_from(media.timebase.fps_den).unwrap_or(0));
        // The part's own A1 (user 2026-10-29), else the sound kept for its clip.
        let kept = qnc_clip_sound::read(target, &piece.clip_id)?.map(|sound| (sound.a1.channel, sound.a1.level_db, sound.a1.off));
        let sound = match piece.a1 {
            Some((channel, level, off)) => Some((channel.or(kept.map(|kept| kept.0)).unwrap_or(0), level, off)),
            None => kept,
        };
        parts.push(compound_part(piece, own, sound.unwrap_or((0, 0, false))));
    }
    let total = qnc_program_db::compound_frames(&parts, rate)?;
    let segment = qnc_program_db::ProgramSegment {
        segment_id: "subsegment".into(),
        kind: "tonovi".into(),
        clip_id: first.clip_id.clone(),
        out_frame: total,
        fps_num: rate.0,
        fps_den: rate.1,
        active: true,
        duration_frames: total,
        parts,
        ..qnc_program_db::ProgramSegment::default()
    };
    let segments = [segment];
    let mut build = build_input(project_id, rate, &segments, &[], channels)?;
    build.playlist_id = format!("subsegment:{project_id}");
    let mut playlist = build_flat_program_playlist(&build, &mut resolver)?;
    apply_joins(&mut playlist, &segments, rate, crossfade)?;
    playlist.validate().map_err(|error| error.message)?;
    ProgramInput::new(playlist, resolver.prepared).map_err(|error| error.to_string())
}

/// A part as a compound segment keeps it: `own` the rate of its clip, `sound` its A1.
pub fn compound_part(piece: &PieceInput, own: (u32, u32), sound: (u16, i8, bool)) -> qnc_program_db::CompoundPart {
    qnc_program_db::CompoundPart {
        clip_id: piece.clip_id.clone(),
        from_frame: piece.from_frame,
        to_frame: piece.to_frame,
        fps_num: own.0,
        fps_den: own.1,
        gap_before: piece.gap_before,
        crossfade: piece.crossfade,
        a1_channel: sound.0,
        a1_level_db: sound.1,
        a1_off: sound.2,
        sound_lead: piece.sound_lead,
        sound_tail: piece.sound_tail,
        picture_lead: piece.picture_lead,
        picture_tail: piece.picture_tail,
        level_keys: piece.level_keys.iter().map(|(frame, db10)| qnc_program_db::LevelKey { frame: *frame, db10: *db10 }).collect(),
        smooth: piece.smooth.clone(),
    }
}

/// The program of a SubSegment, built where the player is prepared.
pub fn pieces_loader(
    target: ProjectDbTarget,
    project_id: String,
    pieces: Vec<PieceInput>,
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static {
    move |reader, workspace_db_uri| with_session(reader, workspace_db_uri, |clips| load_pieces(&target, &project_id, clips, &pieces))
}

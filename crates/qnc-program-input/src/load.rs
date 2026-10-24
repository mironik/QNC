use super::*;

/// Reads the story of the active project and builds its program.
pub fn load_program(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &impl ClipInputs,
) -> Result<ProgramInput, String> {
    load(target, project_id, clips, None, None)
}

/// The A1 of the story under the program window `[in, out)` only (user 2026-10-20: "the
/// story's A1 must be heard, so the user knows how long to hold the clip"): no picture,
/// no A2, nothing over it. The Sync's source plays picture and A2 on its own player; this
/// bed plays beside it. Its frames start at 0.
pub fn load_sound_bed(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &impl ClipInputs,
    window: (u64, u64),
) -> Result<ProgramInput, String> {
    load(target, project_id, clips, Some(window), None)
}

/// The A1 bed of a window, built where its player is prepared.
pub fn sound_bed_loader(
    target: ProjectDbTarget,
    project_id: String,
    window: (u64, u64),
) -> impl FnOnce(&qnc_player_input::InputReader, &str) -> Result<ProgramInput, String> + Send + 'static
{
    move |reader, workspace_db_uri| {
        with_session(reader, workspace_db_uri, |clips| load_sound_bed(&target, &project_id, clips, window))
    }
}

/// Output 1 (A1) of a playlist only: every picture and every other output goes. A source
/// left with nothing is dropped; an item left with no source is silence.
pub fn sound_only(mut playlist: qnc_program_playlist::FlatProgramPlaylist) -> qnc_program_playlist::FlatProgramPlaylist {
    for item in &mut playlist.items {
        for source in &mut item.sources {
            source.video_layer = None;
            source.source_video_format = None;
            source.audio_routes.retain(|route| route.output_channel == 0);
        }
        item.sources.retain(|source| source.has_audio());
    }
    playlist
}

pub(crate) fn load(
    target: &ProjectDbTarget,
    project_id: &str,
    clips: &impl ClipInputs,
    window: Option<(u64, u64)>,
    draft: Option<&SoundDraft>,
) -> Result<ProgramInput, String> {
    // One consistent read of the story (external audit 2026-10-17): segments, covers and
    // rate of the same revision, never an old segment with a new cover.
    let snapshot = StoryReader::open(target)?.program_snapshot()?;
    let (mut segments, mut covers, revision) = (snapshot.segments, snapshot.covers, snapshot.revision);
    if let Some(draft) = draft {
        draft.apply(&mut segments, &mut covers);
    }
    let story_rate = snapshot.story_fps.ok_or(qnc_ui_text::text("story.msg.program_is_empty"))?;
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
    let mut input = build_input(project_id, story_rate, &segments, &covers, channels)?;
    input.revision = revision;
    input.a2_crossfade_frames = i64::from(clips.crossfade_frames()?);
    let mut playlist = build_flat_program_playlist(&input, &mut resolver)?;
    // A window is only ever the A1 bed of a Sync (the story under the slot, user 2026-10-20).
    if let Some((start, end)) = window {
        let range =
            ProgramFrameRange::new(frame(start)?, frame(end)?).map_err(|error| error.message)?;
        playlist = sound_only(build_program_frame_window(&playlist, range)?);
    }
    let mut input = ProgramInput::new(playlist, resolver.prepared).map_err(|error| error.to_string())?;
    // The whole story plays its rendered units (the A1 bed of a Sync plays from its sources). Units that cannot be read leave the story to its sources.
    if window.is_none() {
        input.prerendered = prerendered(target);
    }
    Ok(input)
}

/// The rendered units of the story's prerender (user 2026-10-12), as their render
/// described them; none when the project has none or they cannot be read.
pub(crate) fn prerendered(target: &ProjectDbTarget) -> Vec<qnc_player_input::PrerenderedUnit> {
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

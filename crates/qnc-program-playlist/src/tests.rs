use std::collections::HashMap;

use super::*;

#[derive(Default)]
struct Resolver(HashMap<String, ResolvedProgramMedia>);

impl ProgramMediaResolver for Resolver {
    fn resolve(&mut self, clip_id: &str) -> Result<ResolvedProgramMedia, String> {
        self.0
            .get(clip_id)
            .cloned()
            .ok_or_else(|| format!("missing media: {clip_id}"))
    }
}

fn tb(num: i64) -> FrameTimebase {
    FrameTimebase::new(num, 1).unwrap()
}

fn frames(source_in: i64, source_out: i64, timebase: FrameTimebase) -> FrameRange {
    FrameRange {
        source_in,
        source_out,
        timebase,
    }
}

fn media(clip_id: &str, timebase: FrameTimebase) -> ResolvedProgramMedia {
    let original = MediaRef {
        clip_id: clip_id.into(),
        media_uri: format!("qnc://local/source/card/file/{clip_id}.mxf"),
    };
    ResolvedProgramMedia {
        media: original.clone(),
        audio_media: original,
        duration_frames: 500,
        timebase,
        video_format: Some(ProbedVideoFormat {
            width: 1920,
            height: 1080,
            scan_mode: ScanMode::Progressive,
        }),
        has_audio: true,
        audio_channels: 2,
        audio_format: Some(ProbedAudioFormat {
            sample_rate_hz: 48_000,
            channel_count: 2,
        }),
        timecode_start: Some(50 * 3600),
    }
}

fn resolver(clips: &[&str], timebase: FrameTimebase) -> Resolver {
    let mut resolver = Resolver::default();
    for clip in clips {
        resolver.0.insert((*clip).into(), media(clip, timebase));
    }
    resolver
}

fn segment(
    id: &str,
    kind: &str,
    clip: &str,
    record: (i64, i64),
    source: (i64, i64),
    timebase: FrameTimebase,
) -> ProgramSegmentInput {
    ProgramSegmentInput {
        segment_id: id.into(),
        kind: kind.into(),
        clip_id: clip.into(),
        virtual_shot_id: format!("{clip}-shot"),
        record_range: ProgramFrameRange::new(record.0, record.1).unwrap(),
        source_range: frames(source.0, source.1, timebase),
        a1_source_channel: DEFAULT_SOURCE_CHANNEL,
        a1_level_db: 0,
        a1_off: false,
        covers: Vec::new(),
        v_lead_frames: 0,
    }
}

fn cover(
    id: &str,
    clip: &str,
    record: (i64, i64),
    source: (i64, i64),
    timebase: FrameTimebase,
) -> ProgramCoverInput {
    ProgramCoverInput {
        cover_id: id.into(),
        clip_id: clip.into(),
        virtual_shot_id: format!("{clip}-shot"),
        record_range: ProgramFrameRange::new(record.0, record.1).unwrap(),
        source_range: frames(source.0, source.1, timebase),
        a2_source_channel: DEFAULT_SOURCE_CHANNEL,
        a2_level_db: 0,
        a2_off: false,
        active: true,
        a2_lead_frames: 0,
        a2_tail_frames: 0,
    }
}

fn input(duration: i64, segments: Vec<ProgramSegmentInput>) -> ProgramPlaylistBuildInput {
    ProgramPlaylistBuildInput {
        playlist_id: "program:test".into(),
        project_id: "test".into(),
        revision: 4,
        program_timebase: tb(50),
        audio_layout: ProgramAudioLayout::discrete(2).unwrap(),
        duration_frames: duration,
        segments,
        a2_crossfade_frames: 0,
    }
}

#[test]
fn builder_flattens_cover_and_keeps_discrete_a1_a2() {
    let timebase = tb(50);
    let mut base = segment("segment-1", "tonovi", "base", (0, 100), (10, 110), timebase);
    base.covers = vec![cover("cover-1", "cover", (25, 75), (100, 150), timebase)];
    let playlist = build_flat_program_playlist(
        &input(100, vec![base]),
        &mut resolver(&["base", "cover"], timebase),
    )
    .unwrap();

    assert_eq!(playlist.items.len(), 3);
    let cover_item = &playlist.items[1];
    assert_eq!(
        cover_item.record_range,
        ProgramFrameRange::new(25, 75).unwrap()
    );
    assert_eq!(cover_item.sources.len(), 2);
    assert_eq!(
        cover_item.sources[0].audio_routes[0].output_channel,
        PROGRAM_AUDIO_OUTPUT_A1
    );
    assert!(!cover_item.sources[0].has_video(), "A1 of the segment only");
    assert_eq!(
        cover_item.sources[1].video_layer,
        Some(ProgramVideoLayer::Cover)
    );
    assert_eq!(
        cover_item.sources[1].audio_routes[0].output_channel,
        PROGRAM_AUDIO_OUTPUT_A2
    );
    assert_eq!(
        cover_item.sources[0].source_range,
        frames(35, 85, timebase),
        "the segment sound goes on under the cover"
    );
}

#[test]
fn off_has_no_picture_and_is_heard_on_a1() {
    let timebase = tb(50);
    let playlist = build_flat_program_playlist(
        &input(
            60,
            vec![
                segment("ton", "tonovi", "a", (0, 20), (0, 20), timebase),
                segment("off", "offovi", "b", (20, 60), (100, 140), timebase),
            ],
        ),
        &mut resolver(&["a", "b"], timebase),
    )
    .unwrap();

    assert_eq!(playlist.items.len(), 2);
    let ton = &playlist.items[0];
    assert!(ton
        .sources
        .iter()
        .any(|source| source.video_layer == Some(ProgramVideoLayer::Base)));
    let off = &playlist.items[1];
    assert_eq!(off.record_range, ProgramFrameRange::new(20, 60).unwrap());
    assert!(
        off.sources.iter().all(|source| !source.has_video()),
        "black"
    );
    assert_eq!(
        off.sources[0].audio_routes[0].output_channel,
        PROGRAM_AUDIO_OUTPUT_A1
    );
}

#[test]
fn the_segment_chooses_which_source_channel_is_heard_on_a1() {
    let timebase = tb(50);
    let mut talk = segment("talk", "tonovi", "a", (0, 30), (0, 30), timebase);
    talk.a1_source_channel = 1; // a talk recorded only on channel 2
    let mut resolver = resolver(&["a"], timebase);
    let playlist =
        build_flat_program_playlist(&input(30, vec![talk.clone()]), &mut resolver).unwrap();
    let a1 = playlist.items[0]
        .sources
        .iter()
        .find(|source| source.has_audio())
        .unwrap();
    assert_eq!(
        a1.audio_routes,
        vec![ProgramAudioRoute {
            source_channel: 1,
            output_channel: PROGRAM_AUDIO_OUTPUT_A1,
            level_db: 0,
            fade_in: None,
            fade_out: None,
        }]
    );

    talk.a1_source_channel = 2;
    let error = build_flat_program_playlist(&input(30, vec![talk]), &mut resolver).unwrap_err();
    assert!(
        error.contains("routes source channel 3"),
        "a channel the clip does not have is an error: {error}"
    );
}

#[test]
fn a_cover_chooses_which_source_channel_is_heard_on_a2() {
    let timebase = tb(50);
    let mut base = segment("segment", "tonovi", "base", (0, 40), (0, 40), timebase);
    let mut b_roll = cover("cover", "b", (10, 30), (0, 20), timebase);
    b_roll.a2_source_channel = 1;
    base.covers = vec![b_roll];
    let playlist = build_flat_program_playlist(
        &input(40, vec![base]),
        &mut resolver(&["base", "b"], timebase),
    )
    .unwrap();
    let a2 = &playlist.items[1].sources[1];
    assert_eq!(a2.audio_routes[0].source_channel, 1);
    assert_eq!(a2.audio_routes[0].output_channel, PROGRAM_AUDIO_OUTPUT_A2);
}

#[test]
fn a_cover_needs_a_second_program_channel() {
    let timebase = tb(50);
    let mut base = segment("segment", "tonovi", "base", (0, 40), (0, 40), timebase);
    base.covers = vec![cover("cover", "b", (10, 30), (0, 20), timebase)];
    let mut mono = input(40, vec![base]);
    mono.audio_layout = ProgramAudioLayout::discrete(1).unwrap();
    let error =
        build_flat_program_playlist(&mono, &mut resolver(&["base", "b"], timebase)).unwrap_err();
    assert!(error.contains("output channel 2"), "{error}");
}

#[test]
fn cover_record_end_is_limited_by_source_frames() {
    let timebase = tb(50);
    let cover = cover("cover", "cover", (910, 1219), (13, 320), timebase);
    assert_eq!(cover_available_record_end(&cover), 1217);
    assert_eq!(
        source_range_for_record_chunk(
            cover.source_range,
            cover.record_range.in_frame,
            cover.record_range.in_frame,
            cover_available_record_end(&cover),
        ),
        frames(13, 320, timebase)
    );
}

#[test]
fn builder_rejects_mixed_probe_timebase() {
    let timebase = tb(50);
    let error = build_flat_program_playlist(
        &input(
            60,
            vec![segment(
                "segment",
                "tonovi",
                "base",
                (0, 60),
                (0, 60),
                tb(60),
            )],
        ),
        &mut resolver(&["base"], timebase),
    )
    .unwrap_err();
    assert!(error.contains("does not match the stored probe timebase"));
}

#[test]
fn transient_overlay_replaces_video_and_a2_but_keeps_a1() {
    let timebase = tb(50);
    let mut base = segment("segment", "tonovi", "base", (0, 100), (100, 200), timebase);
    base.covers = vec![cover(
        "old-cover",
        "old_cover",
        (40, 60),
        (10, 30),
        timebase,
    )];
    let playlist = build_flat_program_playlist(
        &input(100, vec![base]),
        &mut resolver(&["base", "old_cover"], timebase),
    )
    .unwrap();
    let new_cover = media("new_cover", timebase);
    let overlay = ProgramTransientOverlayInput {
        record_range: ProgramFrameRange::new(25, 75).unwrap(),
        source: FlatProgramSource {
            source_id: "transient:cover".into(),
            clip_id: "new_cover".into(),
            virtual_shot_id: "new-cover-shot".into(),
            media: new_cover.media,
            source_range: frames(200, 250, timebase),
            source_duration_frames: 500,
            video_layer: Some(ProgramVideoLayer::Cover),
            source_video_format: new_cover.video_format,
            audio_routes: vec![ProgramAudioRoute {
                source_channel: 0,
                output_channel: PROGRAM_AUDIO_OUTPUT_A2,
                level_db: 0,
                fade_in: None,
                fade_out: None,
            }],
            source_audio_channels: 2,
            source_audio_format: new_cover.audio_format,
            source_timecode_start: None,
        },
    };

    let result = apply_transient_program_overlay(&playlist, &overlay).unwrap();

    assert_eq!(result.items.first().unwrap().record_range.in_frame, 0);
    assert_eq!(result.items.last().unwrap().record_range.out_frame, 100);
    let overlay_items = result
        .items
        .iter()
        .filter(|item| {
            item.sources
                .iter()
                .any(|source| source.source_id == "transient:cover")
        })
        .collect::<Vec<_>>();
    assert_eq!(overlay_items.len(), 3);
    assert_eq!(overlay_items[0].record_range.in_frame, 25);
    assert_eq!(overlay_items[2].record_range.out_frame, 75);
    for item in overlay_items {
        assert_eq!(item.sources.iter().filter(|s| s.has_video()).count(), 1);
        assert!(item.sources.iter().any(|source| source
            .audio_routes
            .iter()
            .any(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A1)));
        assert!(!item
            .sources
            .iter()
            .any(|source| source.clip_id == "old_cover"));
    }
}

#[test]
fn frame_window_rebases_program_and_preserves_linked_source_frames() {
    let timebase = tb(50);
    let playlist = build_flat_program_playlist(
        &input(
            120,
            vec![
                segment(
                    "segment-a",
                    "tonovi",
                    "base_a",
                    (0, 60),
                    (100, 160),
                    timebase,
                ),
                segment(
                    "segment-b",
                    "tonovi",
                    "base_b",
                    (60, 120),
                    (200, 260),
                    timebase,
                ),
            ],
        ),
        &mut resolver(&["base_a", "base_b"], timebase),
    )
    .unwrap();

    let window =
        build_program_frame_window(&playlist, ProgramFrameRange::new(50, 80).unwrap()).unwrap();

    assert_eq!(window.duration_frames, 30);
    assert_eq!(window.items.len(), 2);
    assert_eq!(
        window.items[0].record_range,
        ProgramFrameRange::new(0, 10).unwrap()
    );
    assert_eq!(
        window.items[1].record_range,
        ProgramFrameRange::new(10, 30).unwrap()
    );
    assert_eq!(
        window.items[0].sources[0].source_range,
        frames(150, 160, timebase)
    );
    assert_eq!(
        window.items[1].sources[0].source_range,
        frames(200, 220, timebase)
    );
    window.validate().unwrap();
}

#[test]
fn flat_playlist_rejects_source_timebase_that_differs_from_program() {
    let timebase = tb(50);
    let mut playlist = build_flat_program_playlist(
        &input(
            40,
            vec![segment("s", "tonovi", "a", (0, 40), (0, 40), timebase)],
        ),
        &mut resolver(&["a"], timebase),
    )
    .unwrap();
    playlist.items[0].sources[0].source_range.timebase = tb(60);
    assert_eq!(
        playlist.validate().unwrap_err().code,
        "mixed_program_timebase"
    );
}

#[test]
fn flat_playlist_roundtrips_through_json() {
    let timebase = tb(50);
    let playlist = build_flat_program_playlist(
        &input(
            40,
            vec![segment("s", "tonovi", "a", (0, 40), (0, 40), timebase)],
        ),
        &mut resolver(&["a"], timebase),
    )
    .unwrap();
    let encoded = serde_json::to_string(&playlist).unwrap();
    let decoded: FlatProgramPlaylist = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, playlist);
    decoded.validate().unwrap();
}

#[test]
fn missing_media_facts_are_errors_not_stand_ins() {
    let timebase = tb(50);
    let mut resolver = resolver(&["a"], timebase);
    resolver.0.get_mut("a").unwrap().video_format = None;
    let error = build_flat_program_playlist(
        &input(
            40,
            vec![segment("s", "tonovi", "a", (0, 40), (0, 40), timebase)],
        ),
        &mut resolver,
    )
    .unwrap_err();
    assert!(error.contains("no persisted probe format"), "{error}");
}

#[test]
fn a_proxy_picture_keeps_the_sound_of_the_original() {
    let timebase = tb(50);
    let mut resolver = resolver(&["base", "b"], timebase);
    for clip in ["base", "b"] {
        resolver.0.get_mut(clip).unwrap().media.media_uri =
            format!("qnc://local/source/card/file/{clip}_proxy.mp4");
    }
    let mut base = segment("segment", "tonovi", "base", (0, 40), (0, 40), timebase);
    base.covers = vec![cover("cover", "b", (10, 30), (0, 20), timebase)];
    let playlist = build_flat_program_playlist(&input(40, vec![base]), &mut resolver).unwrap();

    let uri = |source: &FlatProgramSource| source.media.media_uri.clone();
    let first = &playlist.items[0].sources;
    assert!(uri(&first[0]).ends_with("base_proxy.mp4") && first[0].has_video());
    assert!(uri(&first[1]).ends_with("base.mxf") && first[1].has_audio());
    let covered = &playlist.items[1].sources;
    assert_eq!(
        covered.len(),
        3,
        "A1 of the segment, cover picture, cover sound"
    );
    assert!(uri(&covered[1]).ends_with("b_proxy.mp4") && !covered[1].has_audio());
    assert!(uri(&covered[2]).ends_with("b.mxf") && !covered[2].has_video());
    assert_eq!(
        covered[2].audio_routes[0].output_channel,
        PROGRAM_AUDIO_OUTPUT_A2
    );
}

/// The A2 routes of the items at `frame`: (source id, fade at that frame).
fn a2_at(playlist: &FlatProgramPlaylist, frame: i64) -> Vec<(String, f32)> {
    let item = playlist.items.iter().find(|item| item.record_range.in_frame <= frame && frame < item.record_range.out_frame).unwrap();
    item.sources
        .iter()
        .flat_map(|source| source.audio_routes.iter().map(move |route| (source, route)))
        .filter(|(_, route)| route.output_channel == PROGRAM_AUDIO_OUTPUT_A2)
        .map(|(source, route)| (source.source_id.clone(), route.fade_at(frame as f64)))
        .collect()
}

/// Locked (user 2026-10-21, news practice): every A2 cut cross-fades; the next cover's sound
/// comes in before its picture (a J) as far as its source has frames before its IN, the A2
/// before it falls over the same frames; A1 is never faded; the picture still cuts hard.
#[test]
fn a2_cuts_cross_fade_and_the_next_cover_comes_in_before_its_picture() {
    let timebase = tb(50);
    let mut base = segment("segment-1", "tonovi", "base", (0, 100), (10, 110), timebase);
    base.covers = vec![
        cover("cover-1", "first", (20, 50), (100, 130), timebase),
        cover("cover-2", "second", (50, 80), (200, 230), timebase),
    ];
    let mut build = input(100, vec![base]);
    build.a2_crossfade_frames = 4;
    let playlist = build_flat_program_playlist(&build, &mut resolver(&["base", "first", "second"], timebase)).unwrap();
    // The first comes in under the statement from 16, rising.
    assert_eq!(a2_at(&playlist, 16), vec![("cover:cover-1:lead".to_string(), 0.0)]);
    assert!(a2_at(&playlist, 18)[0].1 > 0.0 && a2_at(&playlist, 18)[0].1 < 1.0);
    assert_eq!(a2_at(&playlist, 30), vec![("cover:cover-1".to_string(), 1.0)]);
    // At 46..50 both sound: the first falls, the second rises; the picture changes at 50.
    let crossing = a2_at(&playlist, 48);
    assert_eq!(crossing.len(), 2);
    assert!((crossing.iter().map(|(_, fade)| fade).sum::<f32>() - 1.0).abs() < 1e-6, "{crossing:?}");
    let picture = |frame: i64| {
        let item = playlist.items.iter().find(|item| item.record_range.in_frame <= frame && frame < item.record_range.out_frame).unwrap();
        item.sources.iter().find(|source| source.video_layer == Some(ProgramVideoLayer::Cover)).map(|source| source.clip_id.clone())
    };
    assert_eq!(picture(49).as_deref(), Some("first"));
    assert_eq!(picture(50).as_deref(), Some("second"));
    // The last cover falls over its last frames, nothing after it.
    assert!(a2_at(&playlist, 78)[0].1 < 1.0);
    assert!(a2_at(&playlist, 85).is_empty());
    // A1 is never faded.
    let a1 = playlist.items.iter().flat_map(|item| &item.sources).flat_map(|source| &source.audio_routes).filter(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A1);
    assert!(a1.into_iter().all(|route| !route.fades()));
}

#[test]
fn a_cover_with_no_frames_before_its_in_rises_over_its_own_frames_and_its_own_j_is_longer() {
    let timebase = tb(50);
    let mut base = segment("segment-1", "tonovi", "base", (0, 100), (10, 110), timebase);
    let mut late = cover("cover-2", "second", (60, 90), (200, 230), timebase);
    late.a2_lead_frames = 10;
    base.covers = vec![cover("cover-1", "first", (20, 50), (0, 30), timebase), late];
    let mut build = input(100, vec![base]);
    build.a2_crossfade_frames = 4;
    let playlist = build_flat_program_playlist(&build, &mut resolver(&["base", "first", "second"], timebase)).unwrap();
    // No frames before IN 0: it rises over 20..24 on its own picture.
    assert_eq!(a2_at(&playlist, 19), Vec::<(String, f32)>::new());
    assert!(a2_at(&playlist, 21)[0].1 < 1.0);
    // The second's own J of 10 frames: its sound from 50, under the statement.
    assert_eq!(a2_at(&playlist, 50)[0].0, "cover:cover-2:lead");
    assert!(a2_at(&playlist, 49).iter().all(|(id, _)| id != "cover:cover-2:lead"));
}

/// Locked (user 2026-10-21): a cover's own L keeps its sound after its picture and the next
/// cover's sound rises over the same frames; with a J too the window spans both sides of the
/// cut. An L with no cover after it goes on under the story and falls.
#[test]
fn a_cover_l_keeps_its_sound_after_its_picture_and_crosses_with_the_next() {
    let timebase = tb(50);
    let mut base = segment("segment-1", "tonovi", "base", (0, 100), (10, 110), timebase);
    let mut first = cover("cover-1", "first", (20, 50), (100, 130), timebase);
    first.a2_tail_frames = 6;
    let mut second = cover("cover-2", "second", (50, 70), (200, 220), timebase);
    second.a2_lead_frames = 3;
    second.a2_tail_frames = 8;
    base.covers = vec![first, second];
    let mut build = input(100, vec![base]);
    build.a2_crossfade_frames = 4;
    let playlist = build_flat_program_playlist(&build, &mut resolver(&["base", "first", "second"], timebase)).unwrap();
    // The window is 47..56: the second comes in at 47 (its J), the first goes on to 56 (its L).
    assert!(a2_at(&playlist, 46).iter().all(|(id, _)| id == "cover:cover-1"));
    assert!(a2_at(&playlist, 48).iter().any(|(id, _)| id == "cover:cover-2:lead"));
    let after_cut = a2_at(&playlist, 53);
    assert!(after_cut.iter().any(|(id, _)| id == "cover:cover-1:tail"), "{after_cut:?}");
    assert!((after_cut.iter().map(|(_, fade)| fade).sum::<f32>() - 1.0).abs() < 1e-6, "{after_cut:?}");
    assert!(a2_at(&playlist, 57).iter().all(|(id, _)| id == "cover:cover-2"));
    // The second's L goes on under the statement after 70 and falls to 78.
    assert_eq!(a2_at(&playlist, 74)[0].0, "cover:cover-2:tail");
    assert!(a2_at(&playlist, 74)[0].1 < 1.0);
    assert!(a2_at(&playlist, 79).is_empty());
}

/// Locked (user 2026-10-21): the picture J of a Ton segment shows the speaker before his sound,
/// over the end of the segment before, as long as wanted within the frames its source has
/// before its IN; the sound of both segments stays.
#[test]
fn a_ton_segment_with_a_picture_j_starts_its_picture_before_its_sound() {
    let timebase = tb(50);
    let off = segment("off", "offovi", "voice", (0, 50), (100, 150), timebase);
    let mut ton = segment("ton", "tonovi", "speaker", (50, 100), (10, 60), timebase);
    ton.v_lead_frames = 30;
    let build = input(100, vec![off, ton]);
    let playlist = build_flat_program_playlist(&build, &mut resolver(&["voice", "speaker"], timebase)).unwrap();
    let at = |frame: i64| playlist.items.iter().find(|item| item.record_range.in_frame <= frame && frame < item.record_range.out_frame).unwrap();
    let picture = |frame: i64| {
        at(frame).sources.iter().find(|source| source.video_layer == Some(ProgramVideoLayer::Base)).map(|source| (source.clip_id.clone(), source.source_range.source_in))
    };
    let sound = |frame: i64| {
        at(frame).sources.iter().find(|source| source.audio_routes.iter().any(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A1)).map(|source| source.clip_id.clone())
    };
    // Its source has 10 frames before IN: the picture comes 10 frames early, from frame 0.
    assert_eq!(picture(39), None, "the Off is black before");
    assert_eq!(picture(40), Some(("speaker".into(), 0)));
    assert_eq!(sound(45).as_deref(), Some("voice"), "the Off is still heard");
    assert_eq!(sound(55).as_deref(), Some("speaker"));
}

#[test]
fn an_off_with_a1_off_is_black_and_silence_also_under_a_cover() {
    // The lead-in before an Off (user 2026-10-24) and an Off the user switched off: live, its
    // empty sound refused the whole story.
    let timebase = tb(50);
    let mut lead = segment("lead_in_0", "offovi", "base", (0, 100), (10, 110), timebase);
    lead.a1_off = true;
    lead.covers = vec![cover("cover-1", "cover", (0, 60), (100, 160), timebase)];
    let ton = segment("segment-1", "tonovi", "base", (100, 200), (10, 110), timebase);
    let playlist = build_flat_program_playlist(&input(200, vec![lead, ton]), &mut resolver(&["base", "cover"], timebase)).unwrap();
    assert_eq!(playlist.items[0].sources.len(), 1, "the cover only, no A1");
    assert_eq!(playlist.items[0].sources[0].video_layer, Some(ProgramVideoLayer::Cover));
    assert!(playlist.items[1].sources.is_empty(), "black and silence");
}

#[test]
fn a_subsegment_join_crosses_its_sound_and_a_pause_fades_its_edges() {
    let timebase = tb(50);
    // Two parts of one clip joined at 100 (a crossfade of 8), a third after a pause of 10.
    let parts = vec![
        segment("piece-0", "tonovi", "a", (0, 100), (100, 200), timebase),
        segment("piece-1", "tonovi", "a", (100, 200), (300, 400), timebase),
        ProgramSegmentInput { a1_off: true, ..segment("pause-2", "offovi", "a", (200, 210), (0, 10), timebase) },
        segment("piece-2", "tonovi", "a", (210, 260), (50, 100), timebase),
    ];
    let mut playlist = build_flat_program_playlist(&input(260, parts), &mut resolver(&["a"], timebase)).unwrap();
    join_a1(&mut playlist.items, &[A1Join { end: 100, start: 100, frames: 8 }, A1Join { end: 200, start: 210, frames: 8 }]).unwrap();
    playlist.validate().unwrap();
    let at = |frame: i64| playlist.items.iter().find(|item| item.record_range.in_frame <= frame && frame < item.record_range.out_frame).unwrap();
    // Over 96..104 both parts are heard on A1: the one before falls on past its OUT, the one after rises from before its IN.
    let crossing = at(97);
    let a1: Vec<_> = crossing.sources.iter().filter(|source| source.audio_routes.iter().any(|route| route.output_channel == PROGRAM_AUDIO_OUTPUT_A1)).collect();
    assert_eq!(a1.len(), 2, "two sounds cross at the join");
    let lead = a1.iter().find(|source| source.source_id.ends_with(":lead")).unwrap();
    assert_eq!(lead.source_range.source_in, 300 - 4 + (crossing.record_range.in_frame - 96));
    assert_eq!(lead.audio_routes[0].fade_in, Some((96, 104)));
    let after = at(102);
    let tail = after.sources.iter().find(|source| source.source_id.ends_with(":tail")).unwrap();
    assert_eq!(tail.source_range.source_in, 200 + (after.record_range.in_frame - 100), "the part before goes on after its OUT");
    // The pause is silence; the part before it fades out, the part after it fades in.
    assert!(at(205).sources.is_empty(), "a pause is black and silence");
    let fade_out = at(198).sources.iter().flat_map(|source| &source.audio_routes).find_map(|route| route.fade_out);
    assert_eq!(fade_out, Some((196, 200)));
    let fade_in = at(211).sources.iter().flat_map(|source| &source.audio_routes).find_map(|route| route.fade_in);
    assert_eq!(fade_in, Some((210, 214)));
}

#[test]
fn a_cut_join_leaves_the_sound_as_it_is() {
    let timebase = tb(50);
    let parts = vec![
        segment("piece-0", "tonovi", "a", (0, 100), (100, 200), timebase),
        segment("piece-1", "tonovi", "a", (100, 200), (300, 400), timebase),
    ];
    let mut playlist = build_flat_program_playlist(&input(200, parts), &mut resolver(&["a"], timebase)).unwrap();
    let before = playlist.clone();
    join_a1(&mut playlist.items, &[A1Join { end: 100, start: 100, frames: 0 }]).unwrap();
    assert_eq!(playlist, before);
}

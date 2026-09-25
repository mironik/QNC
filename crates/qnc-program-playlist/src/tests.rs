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
        covers: Vec::new(),
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
        active: true,
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
            output_channel: PROGRAM_AUDIO_OUTPUT_A1
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
    assert!(error.contains("ne odgovara spremljenom probe timebaseu"));
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
            }],
            source_audio_channels: 2,
            source_audio_format: new_cover.audio_format,
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

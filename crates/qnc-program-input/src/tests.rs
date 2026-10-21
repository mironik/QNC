use super::*;
use qnc_media_metadata as m;
use qnc_player_input::{AudioChannel, VideoInput};

fn part(id: &str, kind: &str, clip: &str, range: (u64, u64), active: bool) -> ProgramSegment {
    ProgramSegment {
        segment_id: id.into(),
        kind: kind.into(),
        sort_index: 0,
        clip_id: clip.into(),
        in_frame: range.0,
        out_frame: range.1,
        fps_num: 50,
        fps_den: 1,
        active,
        a1_source_channel: 0,
        a1_level_db: 0,
        a1_off: false,
        duration_frames: range.1 - range.0,
        v_lead_frames: 0,
    }
}

fn cover(id: &str, program: (u64, u64), shot: &str) -> ProgramCover {
    ProgramCover {
        cover_id: id.into(),
        slot_id: "a|b".into(),
        clip_id: "broll".into(),
        virtual_shot_id: shot.into(),
        program_start_frame: program.0,
        program_end_frame: program.1,
        source_in_frame: 300,
        source_out_frame: 340,
        fps_num: 50,
        fps_den: 1,
        a2_source_channel: 1,
        a2_level_db: 0,
        a2_off: false,
        a2_lead_frames: 0,
        a2_tail_frames: 0,
    }
}

#[test]
fn active_segments_follow_each_other_and_keep_their_covers_and_channels() {
    let mut talk = part("talk", "tonovi", "a", (100, 130), true);
    talk.a1_source_channel = 1;
    let segments = vec![
        talk,
        part("gone", "offovi", "b", (0, 500), false),
        part("off", "offovi", "b", (10, 50), true),
    ];
    let covers = vec![
        cover("inside-off", (40, 60), "shot"),
        cover("no-shot", (0, 10), ""),
    ];
    let input = build_input("p1", (50, 1), &segments, &covers, 2).unwrap();

    assert_eq!(input.playlist_id, "program:p1");
    assert_eq!(input.duration_frames, 70);
    assert_eq!(input.program_timebase, FrameTimebase::new(50, 1).unwrap());
    assert_eq!(input.audio_layout.channel_count, 2);
    assert_eq!(input.segments.len(), 2, "a deleted segment is not played");
    let (talk, off) = (&input.segments[0], &input.segments[1]);
    assert_eq!(talk.record_range, ProgramFrameRange::new(0, 30).unwrap());
    assert_eq!(
        (talk.source_range.source_in, talk.source_range.source_out),
        (100, 130)
    );
    assert_eq!(
        talk.a1_source_channel, 1,
        "the channel chosen on the Wrap segment"
    );
    assert!(talk.covers.is_empty());
    assert_eq!(off.record_range, ProgramFrameRange::new(30, 70).unwrap());
    assert_eq!(
        off.covers
            .iter()
            .map(|c| c.cover_id.as_str())
            .collect::<Vec<_>>(),
        vec!["inside-off"],
        "only covers over the segment, and only with their virtual shot (v5 streamable)"
    );
    assert_eq!(off.covers[0].a2_source_channel, 1);
    assert!(build_input("p1", (50, 1), &[], &[], 2).is_err());
}

fn fact<T>(value: T) -> Option<m::Fact<T>> {
    Some(m::Fact {
        value,
        evidence_id: "saved".into(),
        locator: "/saved".into(),
    })
}

fn stream(index: u32, details: m::StreamDetails) -> m::MediaStream {
    m::MediaStream {
        index: fact(index),
        codec: None,
        profile: None,
        time_base: None,
        start_pts: None,
        duration_ts: None,
        details,
    }
}

fn video_stream(width: u32) -> m::MediaStream {
    stream(
        0,
        m::StreamDetails::Video(Box::new(m::VideoMetadata {
            width: fact(width),
            height: fact(width * 9 / 16),
            frame_rate: fact(FrameTimebase::new(50, 1).unwrap()),
            frame_rate_mode: fact(m::FrameRateMode::Constant),
            frame_count: fact(m::FrameCount::Exact(500)),
            scan_mode: fact(m::ScanMode::InterlacedTopFieldFirst),
            pixel_format: None,
            sample_aspect_ratio: None,
            rotation_degrees: None,
            color: m::ColorMetadata {
                primaries: None,
                transfer: None,
                matrix: None,
                range: None,
            },
        })),
    )
}

fn audio_stream(index: u32, rate: u32, channels: u32) -> m::MediaStream {
    stream(
        index,
        m::StreamDetails::Audio(Box::new(m::AudioMetadata {
            sample_rate_hz: fact(rate),
            channels: fact(channels),
            sample_format: None,
            channel_layout: None,
            bits_per_sample: None,
        })),
    )
}

fn representation(uri: &str, streams: Vec<m::MediaStream>) -> m::MediaRepresentation {
    m::MediaRepresentation {
        media_uri: uri.into(),
        container: None,
        duration_seconds: None,
        streams_complete: None,
        streams,
        tags: Default::default(),
    }
}

fn layout(channels: &[(u32, u32)]) -> StreamLayout {
    StreamLayout {
        video: Some(VideoInput {
            stream_index: 0,
            timebase: FrameTimebase::new(50, 1).unwrap(),
            duration_frames: 500,
            frame_rate_mode: m::FrameRateMode::Constant,
        }),
        audio_channels: channels
            .iter()
            .map(|&(stream_index, channel_index)| AudioChannel {
                stream_index,
                channel_index,
            })
            .collect(),
    }
}

#[test]
fn media_facts_come_from_the_saved_record_picture_from_proxy_sound_from_original() {
    let proxy = representation(
        "qnc://local/source/c/file/a_proxy.mp4",
        vec![video_stream(960)],
    );
    let original = representation(
        "qnc://local/source/c/file/a.mxf",
        vec![
            video_stream(1920),
            audio_stream(1, 48_000, 1),
            audio_stream(2, 48_000, 1),
            audio_stream(3, 48_000, 2),
        ],
    );
    let resolved = resolved_media(
        "a",
        &proxy,
        &original,
        &layout(&[(1, 0), (2, 0), (3, 0), (3, 1)]),
    )
    .unwrap();

    assert!(resolved.media.media_uri.ends_with("a_proxy.mp4"));
    assert!(resolved.audio_media.media_uri.ends_with("a.mxf"));
    assert_eq!(resolved.duration_frames, 500);
    let video = resolved.video_format.unwrap();
    assert_eq!((video.width, video.height), (960, 540));
    assert_eq!(video.scan_mode, ScanMode::InterlacedTopFieldFirst);
    assert_eq!(resolved.audio_channels, 4, "a camera with four channels");
    assert_eq!(resolved.audio_format.unwrap().sample_rate_hz, 48_000);
}

#[test]
fn missing_or_mixed_facts_are_errors() {
    let original = representation(
        "qnc://local/source/c/file/a.mxf",
        vec![
            video_stream(1920),
            audio_stream(1, 48_000, 1),
            audio_stream(2, 44_100, 1),
        ],
    );
    let error = resolved_media("a", &original, &original, &layout(&[(1, 0), (2, 0)])).unwrap_err();
    assert!(error.contains("different sample rates"), "{error}");

    let mut no_video = layout(&[]);
    no_video.video = None;
    assert!(resolved_media("a", &original, &original, &no_video).is_err());

    let silent = resolved_media("a", &original, &original, &layout(&[])).unwrap();
    assert!(!silent.has_audio && silent.audio_format.is_none());
}

#[test]
fn a_source_of_half_the_story_rate_is_counted_in_story_frames() {
    // A 50p story: a 1080i50 segment (25 frames) 10..20 is 20 story frames (its fields),
    // its source counted at the story rate from 20; a 25 cover over it likewise.
    let mut xdcam = part("xdcam", "tonovi", "x", (10, 20), true);
    (xdcam.fps_num, xdcam.duration_frames) = (25, 20);
    let mut over = cover("over", (2, 8), "shot");
    (over.fps_num, over.source_in_frame) = (25, 5);
    let input = build_input("p1", (50, 1), &[xdcam], &[over], 2).unwrap();
    let segment = &input.segments[0];
    assert_eq!(segment.record_range, ProgramFrameRange::new(0, 20).unwrap());
    assert_eq!((segment.source_range.source_in, segment.source_range.source_out), (20, 40));
    assert_eq!(segment.source_range.timebase, FrameTimebase::new(50, 1).unwrap());
    let cover = &segment.covers[0];
    assert_eq!((cover.source_range.source_in, cover.source_range.source_out), (10, 16));
    // A rate not 2:1 is refused.
    let mut other = part("other", "tonovi", "y", (0, 10), true);
    other.fps_num = 30;
    assert!(build_input("p1", (50, 1), &[other], &[], 2).is_err());
}

#[test]
fn a_cover_longer_than_its_clip_ends_with_its_clip() {
    struct Lengths;
    impl ProgramMediaResolver for Lengths {
        fn resolve(&mut self, clip_id: &str) -> Result<ResolvedProgramMedia, String> {
            let media = qnc_program_playlist::MediaRef { clip_id: clip_id.into(), media_uri: "qnc://local/source/s/file/a".into() };
            Ok(ResolvedProgramMedia {
                media: media.clone(),
                audio_media: media,
                duration_frames: 320,
                timebase: FrameTimebase::new(50, 1).unwrap(),
                video_format: None,
                has_audio: false,
                audio_channels: 0,
                audio_format: None,
                timecode_start: None,
            })
        }
    }
    // The clip has 320 frames, the cover starts at 300: 20 are left for its 40-frame slot.
    let fitted = fit_covers(vec![cover("c", (100, 140), "shot"), cover("d", (200, 240), "shot")], &mut Lengths, (50, 1)).unwrap();
    assert_eq!(fitted.iter().map(|c| (c.program_start_frame, c.program_end_frame)).collect::<Vec<_>>(), vec![(100, 120), (200, 220)]);
    let mut late = cover("e", (0, 10), "shot");
    late.source_in_frame = 320;
    assert!(fit_covers(vec![late], &mut Lengths, (50, 1)).unwrap().is_empty(), "nothing left to play");
}

#[test]
fn a_sound_draft_changes_only_its_own_segment_or_cover() {
    let mut segments = vec![part("talk", "tonovi", "a", (0, 50), true), part("off", "offovi", "b", (0, 50), true)];
    let mut covers = vec![cover("c1", (10, 20), "s1"), cover("c2", (30, 40), "s2")];
    SoundDraft { lane: 0, id: "off".into(), channel: 2, level_db: -6, off: false, overlaps: Vec::new(), picture_lead: None }.apply(&mut segments, &mut covers);
    SoundDraft { lane: 1, id: "c2".into(), channel: 0, level_db: 3, off: true, overlaps: Vec::new(), picture_lead: None }.apply(&mut segments, &mut covers);
    assert_eq!((segments[0].a1_source_channel, segments[0].a1_level_db), (0, 0), "the other segment stays");
    assert_eq!((segments[1].a1_source_channel, segments[1].a1_level_db, segments[1].a1_off), (2, -6, false));
    assert_eq!((covers[0].a2_level_db, covers[0].a2_off), (0, false), "the other cover stays");
    assert_eq!((covers[1].a2_source_channel, covers[1].a2_level_db, covers[1].a2_off), (0, 3, true));
}

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
        duration_frames: range.1 - range.0,
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
    assert!(error.contains("razlicitim sample rateom"), "{error}");

    let mut no_video = layout(&[]);
    no_video.video = None;
    assert!(resolved_media("a", &original, &original, &no_video).is_err());

    let silent = resolved_media("a", &original, &original, &layout(&[])).unwrap();
    assert!(!silent.has_audio && silent.audio_format.is_none());
}

#[test]
fn a_transient_cover_takes_the_window_and_the_stored_covers_there_give_way() {
    let covers = vec![
        cover("before", (0, 10), "shot"),
        cover("inside", (15, 25), "shot"),
    ];
    let transient = TransientCover {
        clip_id: "sync".into(),
        source_in: 40,
        timebase: (50, 1),
        a2_source_channel: 1,
    };
    let covers = with_transient_cover(&covers, (12, 30), &transient);
    let ids: Vec<&str> = covers.iter().map(|c| c.cover_id.as_str()).collect();
    assert_eq!(ids, vec!["before", "sync-cover-preview"]);
    let sync = &covers[1];
    assert_eq!((sync.program_start_frame, sync.program_end_frame), (12, 30));
    assert_eq!((sync.source_in_frame, sync.source_out_frame), (40, 58));
    assert_eq!(sync.a2_source_channel, 1, "the channel chosen for A2");
    let segments = vec![part("off", "offovi", "b", (0, 40), true)];
    let input = build_input("p1", (50, 1), &segments, &covers, 2).unwrap();
    assert_eq!(
        input.segments[0].covers.len(),
        2,
        "played like a stored cover"
    );
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

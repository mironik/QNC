use super::*;
use qnc_player_input::AudioChannel;
use qnc_program_playlist as p;

fn timebase() -> p::FrameTimebase {
    p::FrameTimebase::new(50, 1).unwrap()
}

fn source(
    clip: &str,
    source_in: i64,
    frames: i64,
    layer: Option<p::ProgramVideoLayer>,
    route: Option<(u16, u16)>,
) -> p::FlatProgramSource {
    p::FlatProgramSource {
        source_id: format!("{clip}:{source_in}"),
        clip_id: clip.into(),
        virtual_shot_id: String::new(),
        media: p::MediaRef {
            clip_id: clip.into(),
            media_uri: format!("qnc://local/source/card/file/{clip}.mxf"),
        },
        source_range: p::FrameRange {
            source_in,
            source_out: source_in + frames,
            timebase: timebase(),
        },
        source_duration_frames: 1000,
        video_layer: layer,
        source_video_format: layer.map(|_| p::ProbedVideoFormat {
            width: 1920,
            height: 1080,
            scan_mode: p::ScanMode::Progressive,
        }),
        audio_routes: route
            .map(|(source_channel, output_channel)| p::ProgramAudioRoute {
                source_channel,
                output_channel,
            })
            .into_iter()
            .collect(),
        source_audio_channels: if route.is_some() { 2 } else { 0 },
        source_audio_format: route.map(|_| p::ProbedAudioFormat {
            sample_rate_hz: 48_000,
            channel_count: 2,
        }),
        source_timecode_start: None,
    }
}

fn item(start: i64, sources: Vec<p::FlatProgramSource>) -> p::FlatProgramItem {
    p::FlatProgramItem {
        item_id: format!("item:{start}"),
        record_range: p::ProgramFrameRange::new(start, start + 10).unwrap(),
        sources,
    }
}

/// Ton `a` (A1 = its channel 2), then `a` under a cover `b` (A2), then Off `c`.
fn playlist() -> p::FlatProgramPlaylist {
    let base = Some(p::ProgramVideoLayer::Base);
    let cover = Some(p::ProgramVideoLayer::Cover);
    p::FlatProgramPlaylist {
        playlist_id: "program:p1".into(),
        project_id: "p1".into(),
        revision: 0,
        program_timebase: timebase(),
        audio_layout: p::ProgramAudioLayout::discrete(2).unwrap(),
        duration_frames: 30,
        items: vec![
            item(
                0,
                vec![
                    source("a", 100, 10, base, None),
                    source("a", 100, 10, None, Some((1, 0))),
                ],
            ),
            item(
                10,
                vec![
                    source("a", 110, 10, None, Some((1, 0))),
                    source("b", 200, 10, cover, Some((0, 1))),
                ],
            ),
            item(20, vec![source("c", 0, 10, None, Some((0, 0)))]),
        ],
    }
}

fn sound(clip: &str, rate: u32) -> Result<ClipSound> {
    let channel = |stream_index, channel_index| AudioChannel {
        stream_index,
        channel_index,
    };
    Ok(match clip {
        "a" => ClipSound {
            channels: vec![channel(1, 0), channel(2, 0)],
            streams: vec![(1, 1), (2, 1)],
            rate: Some(rate),
        },
        "b" => ClipSound {
            channels: vec![channel(1, 0), channel(1, 1)],
            streams: vec![(1, 2)],
            rate: Some(rate),
        },
        _ => ClipSound {
            channels: vec![channel(1, 0)],
            streams: vec![(1, 1)],
            rate: Some(rate),
        },
    })
}

#[test]
fn the_top_picture_layer_plays_and_off_is_black() {
    let (video, _) = program_spans(
        &playlist(),
        &["a", "b", "c"],
        |clip| sound(clip, 48_000),
        48_000,
    )
    .unwrap();
    assert_eq!(
        video,
        vec![
            VideoSpan {
                record_in: 0,
                record_out: 10,
                video: Some((0, 100))
            },
            VideoSpan {
                record_in: 10,
                record_out: 20,
                video: Some((1, 200))
            },
            VideoSpan {
                record_in: 20,
                record_out: 30,
                video: None
            },
        ],
        "the cover picture over the segment; Off has no picture"
    );
}

#[test]
fn buses_take_the_chosen_saved_channel_to_a1_and_a2() {
    let (_, audio) = program_spans(
        &playlist(),
        &["a", "b", "c"],
        |clip| sound(clip, 48_000),
        48_000,
    )
    .unwrap();
    let a1_of_a = Bus {
        output: 0,
        clip: 0,
        source_in: 100,
        stream_index: 2,
        channel_index: 0,
        stream_channels: 1,
    };
    assert_eq!(
        audio[0].buses,
        vec![a1_of_a],
        "channel 2 of `a` is its second stream"
    );
    assert_eq!(
        audio[1].buses,
        vec![
            Bus {
                source_in: 110,
                ..a1_of_a
            },
            Bus {
                output: 1,
                clip: 1,
                source_in: 200,
                stream_index: 1,
                channel_index: 0,
                stream_channels: 2,
            },
        ],
        "the segment keeps A1 under the cover; the cover is heard on A2"
    );
    assert_eq!(audio[2].buses[0].output, 0, "Off is heard on A1");
}

#[test]
fn another_rate_or_a_missing_channel_is_an_error_not_a_guess() {
    let error = program_spans(
        &playlist(),
        &["a", "b", "c"],
        |clip| sound(clip, 44_100),
        48_000,
    )
    .unwrap_err();
    assert!(error.to_string().contains("sample rate"), "{error}");
    let mut wide = playlist();
    wide.items[0].sources[1].audio_routes[0].source_channel = 5;
    assert!(program_spans(&wide, &["a", "b", "c"], |clip| sound(clip, 48_000), 48_000).is_err());
    assert!(program_spans(&playlist(), &["a", "b"], |clip| sound(clip, 48_000), 48_000).is_err());
}

#[test]
fn a_cover_has_its_own_lane_so_its_sound_opens_before_the_cut() {
    let bus = |clip, source_in| Bus {
        output: 1,
        clip,
        source_in,
        stream_index: 1,
        channel_index: 0,
        stream_channels: 4,
    };
    let span = |record_in, bus| AudioSpan {
        record_in,
        record_out: record_in + 50,
        buses: vec![bus],
    };
    let (first, next_cover) = (span(100, bus(2, 400)), span(150, bus(2, 900)));
    assert_ne!(
        lane_key(&first, &first.buses[0]),
        lane_key(&next_cover, &next_cover.buses[0]),
        "the same channel of the same clip from another place is another lane"
    );
    let continued = span(150, bus(2, 450));
    assert_eq!(
        lane_key(&first, &first.buses[0]),
        lane_key(&continued, &continued.buses[0]),
        "a cut that continues the same source keeps its lane, no reopen"
    );
}

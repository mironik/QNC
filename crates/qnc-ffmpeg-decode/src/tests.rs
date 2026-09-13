#![cfg(test)]
use super::*;
use qnc_media_metadata::*;
use std::path::Path;
fn fact<T>(value: T) -> Fact<T> {
    Fact {
        value,
        evidence_id: "saved".into(),
        locator: "saved".into(),
    }
}
pub(crate) fn fixture() -> DecodeRequest {
    DecodeRequest {
        version: VERSION.into(),
        stream_index: 0,
        start: None,
        media: MediaRepresentation {
            media_uri: "qnc://local/source/test/file/clip%2Emkv".into(),
            container: Some(fact("matroska".into())),
            duration_seconds: Some(fact(Rational {
                numerator: 1,
                denominator: 5,
            })),
            streams_complete: Some(fact(true)),
            tags: Default::default(),
            streams: vec![qnc_media_metadata::MediaStream {
                index: Some(fact(0)),
                codec: Some(fact(Signal::Known("ffv1".into()))),
                profile: None,
                time_base: Some(fact(Rational {
                    numerator: 1,
                    denominator: 1000,
                })),
                start_pts: Some(fact(0)),
                duration_ts: Some(fact(200)),
                details: StreamDetails::Video(Box::new(VideoMetadata {
                    width: Some(fact(16)),
                    height: Some(fact(16)),
                    frame_rate: Some(fact(FrameTimebase {
                        fps_num: 25,
                        fps_den: 1,
                    })),
                    frame_rate_mode: Some(fact(FrameRateMode::Unknown)),
                    frame_count: Some(fact(FrameCount::Exact(5))),
                    scan_mode: Some(fact(ScanMode::Progressive)),
                    pixel_format: Some(fact("yuv420p".into())),
                    sample_aspect_ratio: Some(fact(Rational {
                        numerator: 1,
                        denominator: 1,
                    })),
                    rotation_degrees: Some(fact(Signal::Known(0))),
                    color: ColorMetadata {
                        primaries: None,
                        transfer: None,
                        matrix: None,
                        range: None,
                    },
                })),
            }],
        },
    }
}
#[test]
fn preserves_unknown_frame_rate_and_native_10_bit_packet_size() {
    let mut request = fixture();
    let StreamDetails::Video(v) = &mut request.media.streams[0].details else {
        unreachable!()
    };
    v.pixel_format = Some(fact("yuv422p10le".into()));
    v.width = Some(fact(1920));
    v.height = Some(fact(1080));
    let plan =
        DecodePlan::new(&request, &DecoderConfig::new(FfmpegAdapter::new("ffmpeg"))).unwrap();
    assert_eq!(plan.exact_packet, Some(1920 * 1080 * 4));
    let StreamDetails::Video(v) = &request.media.streams[0].details else {
        unreachable!()
    };
    assert_eq!(
        v.frame_rate_mode.as_ref().unwrap().value,
        FrameRateMode::Unknown
    );
}
#[test]
fn dimensions_planar_rounding_and_unsupported_formats() {
    assert_eq!(video_bytes(3, 3, "yuv420p").unwrap(), 17);
    assert_eq!(video_bytes(3, 3, "yuv422p10le").unwrap(), 42);
    assert!(video_bytes(0, 1080, "yuv420p").is_err());
    assert!(video_bytes(16, 16, "unknown").is_err());
}
#[test]
fn missing_metadata_and_memory_budget_fail_before_process_start() {
    let config = DecoderConfig::new(FfmpegAdapter::new("not-an-executable"));
    let mut r = fixture();
    r.media.streams[0].codec = None;
    assert!(r.validate(&config).is_err());
    let mut r = fixture();
    r.media.streams.push(r.media.streams[0].clone());
    assert!(r.validate(&config).is_err());
    let mut r = fixture();
    r.media.container = None;
    assert!(r.validate(&config).is_err());
    let mut r = fixture();
    r.stream_index = 1;
    assert!(r.validate(&config).is_err());
    let mut r = fixture();
    r.version = "other".into();
    assert!(r.validate(&config).is_err());
    let mut c = config;
    c.memory_budget_bytes = 100;
    assert!(fixture().validate(&c).is_err());
}
#[test]
fn seek_requires_saved_duration_and_valid_rational_not_project_fps() {
    let mut r = fixture();
    let c = DecoderConfig::new(FfmpegAdapter::new("ffmpeg"));
    r.start = Some(Rational {
        numerator: 1,
        denominator: 10,
    });
    assert!(r.validate(&c).is_ok());
    r.start = Some(Rational {
        numerator: 1,
        denominator: 0,
    });
    assert!(r.validate(&c).is_err());
    r.start = Some(Rational {
        numerator: 1,
        denominator: 5,
    });
    assert!(r.validate(&c).is_err());
    r.start = Some(Rational {
        numerator: 0,
        denominator: 1,
    });
    r.media.duration_seconds = None;
    assert!(r.validate(&c).is_err());
}
#[test]
fn packet_records_are_strict_and_preserve_signed_integer_pts() {
    let h = parse_record("QNC_PACKET 2 -5 1/48000 384\n")
        .unwrap()
        .unwrap();
    assert_eq!(h.pts, -5);
    assert_eq!(h.time_base.denominator, 48000);
    for line in [
        "QNC_PACKET 0 N/A 1/1 8",
        "QNC_PACKET 0 1 1/0 8",
        "QNC_PACKET 0 1 1/1 0",
        "QNC_PACKET 0 9223372036854775807 1/1 8",
        "QNC_PACKET 0 1 1/1 8 extra",
    ] {
        assert!(parse_record(line).is_err(), "{line}");
    }
    assert!(parse_record("decoder diagnostic").unwrap().is_none());
}

#[test]
fn stats_reader_rebases_ffmpeg_packet_numbers_to_decode_session_ordinals() {
    let mut reader = StatsReader::default();

    let first = reader
        .parse("QNC_PACKET 22 0 1/48000 384\n")
        .unwrap()
        .unwrap();
    let second = reader
        .parse("QNC_PACKET 40 1920 1/48000 384\n")
        .unwrap()
        .unwrap();

    assert_eq!(first.ordinal, 0);
    assert_eq!(second.ordinal, 1);
    assert_eq!(second.pts, 1920);
}

#[test]
fn command_forces_saved_codec_and_disables_rate_pixel_and_network_fallbacks() {
    let r = fixture();
    let c = DecoderConfig::new(FfmpegAdapter::new("ffmpeg"));
    let plan = DecodePlan::new(&r, &c).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mkv");
    std::fs::write(&path, b"fixture").unwrap();
    let ep = qnc_media_stream::CodecEndpoint::for_local_file(&path, &r.media.media_uri).unwrap();
    let cmd = FfmpegAdapter::new("ffmpeg")
        .command(&r, &plan, &ep, "8-abc")
        .unwrap();
    let args: Vec<_> = cmd
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert!(!args.contains(&"-r".into()));
    for pair in [
        ["-c:0", "ffv1"],
        ["-vf", "setfield=prog,format=yuv420p"],
        ["-pix_fmt", "+yuv420p"],
        ["-enc_time_base", "demux"],
        ["-stats_mux_pre", "pipe:2"],
        ["-protocol_whitelist", "file"],
        ["-flush_packets", "1"],
    ] {
        assert!(args.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1]));
    }
    assert!(args.iter().any(|arg| arg == &path.to_string_lossy()));
    for forbidden in [
        "-xerror",
        "-headers",
        "-request_size",
        "-initial_request_size",
        "-short_seek_size",
        "-multiple_requests",
        "-reconnect",
        "-nofind_stream_info",
    ] {
        assert!(!args.contains(&forbidden.into()), "{forbidden}");
    }
}

#[test]
fn local_seekable_endpoint_uses_file_protocol_without_tcp_bridge() {
    let r = fixture();
    let c = DecoderConfig::new(FfmpegAdapter::new("ffmpeg"));
    let plan = DecodePlan::new(&r, &c).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mkv");
    std::fs::write(&path, b"fixture").unwrap();
    let ep = qnc_media_stream::CodecEndpoint::for_local_file(&path, &r.media.media_uri).unwrap();
    let command = FfmpegAdapter::new("ffmpeg")
        .command(&r, &plan, &ep, "8-abc")
        .unwrap();
    let args: Vec<_> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let path_arg = path.to_string_lossy().into_owned();

    assert!(
        args.windows(2)
            .any(|w| w[0] == "-protocol_whitelist" && w[1] == "file")
    );
    assert!(args.iter().any(|arg| arg == &path_arg));
    assert!(!args.iter().any(|arg| arg.starts_with("tcp://")));
}

#[test]
fn ffmpeg_adapter_rejects_session_private_tcp_bridge() {
    let r = fixture();
    let c = DecoderConfig::new(FfmpegAdapter::new("ffmpeg"));
    let plan = DecodePlan::new(&r, &c).unwrap();
    let ep = qnc_media_stream::CodecEndpoint::for_loopback(
        "127.0.0.1:1".parse().unwrap(),
        &r.media.media_uri,
    )
    .unwrap();

    let error = FfmpegAdapter::new("ffmpeg")
        .command(&r, &plan, &ep, "8-abc")
        .unwrap_err();

    assert_eq!(error.kind, ErrorKind::Unsupported);
    assert!(error.to_string().contains("seekable codec endpoint"));
}

#[test]
fn mxf_uses_seekable_file_endpoint() {
    let mut r = fixture();
    r.media.container.as_mut().unwrap().value = "mxf".into();
    let c = DecoderConfig::new(FfmpegAdapter::new("ffmpeg"));
    let plan = DecodePlan::new(&r, &c).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clip.mxf");
    std::fs::write(&path, b"fixture").unwrap();
    let ep = qnc_media_stream::CodecEndpoint::for_local_file(&path, &r.media.media_uri).unwrap();
    let command = FfmpegAdapter::new("ffmpeg")
        .command(&r, &plan, &ep, "8-abc")
        .unwrap();
    let args: Vec<_> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert!(
        args.windows(2)
            .any(|w| w[0] == "-protocol_whitelist" && w[1] == "file")
    );
    assert!(args.iter().any(|arg| arg == &path.to_string_lossy()));
    assert!(!args.contains(&"-headers".into()));
    assert!(!args.contains(&"-nofind_stream_info".into()));
}

#[test]
fn filmstrip_random_seek_is_one_ffmpeg_command_with_multiple_inputs() {
    let frames = vec![
        FfmpegFilmstripFrame { seek_sec: 0.0 },
        FfmpegFilmstripFrame { seek_sec: 12.5 },
    ];
    let args = filmstrip_random_seek_args(
        Path::new("source.mxf"),
        &frames,
        [112, 64],
        Path::new("filmstrip-tmp"),
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>();

    assert_eq!(
        rendered.iter().filter(|arg| arg.as_ref() == "-ss").count(),
        2
    );
    assert_eq!(
        rendered.iter().filter(|arg| arg.as_ref() == "-i").count(),
        2
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|arg| arg.as_ref() == "-noaccurate_seek")
            .count(),
        2
    );
    assert!(rendered.iter().any(|arg| arg.as_ref() == "12.500000"));
    assert!(!rendered.iter().any(|arg| arg.contains("fps=")));
    assert!(!rendered.iter().any(|arg| arg.as_ref() == "-skip_frame"));
}

#[test]
fn filmstrip_keyframe_seek_targets_intra_frames_without_full_clip_scan() {
    let frames = vec![
        FfmpegFilmstripFrame { seek_sec: 0.0 },
        FfmpegFilmstripFrame { seek_sec: 12.5 },
    ];
    let args = filmstrip_keyframe_seek_args(
        Path::new("source.mxf"),
        &frames,
        [112, 64],
        Path::new("filmstrip-tmp"),
    );
    let rendered = args
        .iter()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>();

    assert_eq!(
        rendered.iter().filter(|arg| arg.as_ref() == "-ss").count(),
        2
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|arg| arg.as_ref() == "-skip_frame")
            .count(),
        2
    );
    assert_eq!(
        rendered
            .iter()
            .filter(|arg| arg.as_ref() == "nokey")
            .count(),
        2
    );
    assert!(!rendered.iter().any(|arg| arg.contains("select=")));
    assert!(!rendered.iter().any(|arg| arg.contains("60")));
}

fn synthetic() -> (tempfile::TempDir, DecodeRequest) {
    let dir = tempfile::tempdir().unwrap();
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-nofind_stream_info",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=16x16:rate=25:duration=0.2",
            "-c:v",
            "ffv1",
            "-f",
            "matroska",
        ])
        .arg(dir.path().join("clip.mkv"))
        .status()
        .unwrap();
    assert!(status.success());
    (dir, fixture())
}
fn open(dir: &std::path::Path, request: DecodeRequest) -> Decoder {
    let source = qnc_media_stream::LocalSource::new("qnc://local/source/test", dir).unwrap();
    let media = qnc_media_stream::MediaStream::local(&source, &request.media.media_uri).unwrap();
    let stamp = media.info().storage_stamp.clone();
    let endpoint = qnc_media_stream::CodecEndpoint::for_local_file(
        dir.join("clip.mkv"),
        &request.media.media_uri,
    )
    .unwrap();
    Decoder::open_endpoint(
        request,
        endpoint,
        stamp,
        DecoderConfig::new(FfmpegAdapter::new("ffmpeg")),
    )
    .unwrap()
}
#[test]
#[ignore = "requires FFmpeg; explicit integration run"]
fn real_continuous_decode_has_pts_exact_eof_and_reaps_process() {
    let (dir, r) = synthetic();
    let mut d = open(dir.path(), r);
    let pid = d.process_id().unwrap();
    let mut count = 0;
    let mut last = None;
    while let Some(packet) = d.next_packet().unwrap() {
        assert_eq!(d.process_id(), Some(pid));
        assert_eq!(packet.ordinal, count);
        assert_eq!(packet.bytes.len(), 384);
        assert!(last.is_none_or(|pts| pts < packet.pts));
        last = Some(packet.pts);
        count += 1;
    }
    assert_eq!(count, 5);
    assert_eq!(d.process_id(), None);
    assert!(d.next_packet().unwrap().is_none());
}

#[test]
#[ignore = "requires FFmpeg; explicit integration run"]
fn real_nonblocking_receive_preserves_packets_and_eof_without_extra_worker() {
    use std::{
        task::Poll,
        time::{Duration, Instant},
    };
    let (dir, request) = synthetic();
    let mut decoder = open(dir.path(), request);
    let pid = decoder.process_id().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut count = 0;
    loop {
        assert!(Instant::now() < deadline);
        match decoder.try_next_packet().unwrap() {
            Poll::Pending => std::thread::sleep(Duration::from_millis(1)),
            Poll::Ready(Some(packet)) => {
                assert_eq!(decoder.process_id(), Some(pid));
                assert_eq!(packet.ordinal, count);
                count += 1;
            }
            Poll::Ready(None) => break,
        }
    }
    assert_eq!(count, 5);
    assert_eq!(decoder.process_id(), None);
    assert!(decoder.try_next_packet().unwrap().is_ready());
    decoder.cancel();
}
#[test]
#[ignore = "requires FFmpeg; explicit integration run"]
fn real_cancel_with_full_queue_and_independent_sessions() {
    let (dir, r) = synthetic();
    let mut first = open(dir.path(), r.clone());
    let mut second = open(dir.path(), r);
    assert_ne!(first.process_id(), second.process_id());
    std::thread::sleep(std::time::Duration::from_millis(300));
    let start = std::time::Instant::now();
    first.cancel();
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    assert_eq!(first.process_id(), None);
    assert_eq!(first.next_packet().unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(second.next_packet().unwrap().unwrap().ordinal, 0);
    second.cancel();
}
#[test]
#[ignore = "requires FFmpeg; explicit integration run"]
fn real_corrupt_media_is_error_not_clean_eof() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("clip.mkv"), b"bad").unwrap();
    let mut d = open(dir.path(), fixture());
    assert!(d.next_packet().is_err());
    assert_eq!(d.process_id(), None);
}

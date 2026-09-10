#![cfg(test)]
use super::*;
use qnc_media_metadata::*;
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
fn command_forces_saved_codec_and_disables_discovery_rate_and_pixel_fallbacks() {
    let r = fixture();
    let c = DecoderConfig::new(FfmpegAdapter::new("ffmpeg"));
    let plan = DecodePlan::new(&r, &c).unwrap();
    let ep = qnc_media_stream::HttpEndpoint::for_owner_endpoint(
        "http://127.0.0.1:1",
        &r.media.media_uri,
        "test-token",
    )
    .unwrap();
    let cmd = FfmpegAdapter::new("ffmpeg")
        .command(&r, &plan, &ep, "8-abc")
        .unwrap();
    let args: Vec<_> = cmd
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert!(args.contains(&"-nofind_stream_info".into()));
    assert!(!args.contains(&"-r".into()));
    for pair in [
        ["-c:0", "ffv1"],
        ["-pix_fmt", "+yuv420p"],
        ["-enc_time_base", "demux"],
        ["-stats_mux_pre", "pipe:2"],
        ["-short_seek_size", "1"],
        ["-flush_packets", "1"],
    ] {
        assert!(args.windows(2).any(|w| w[0] == pair[0] && w[1] == pair[1]));
    }
}

#[test]
fn mxf_small_interleaved_packets_reuse_the_bounded_http_read_window() {
    let mut r = fixture();
    r.media.container.as_mut().unwrap().value = "mxf".into();
    let c = DecoderConfig::new(FfmpegAdapter::new("ffmpeg"));
    let plan = DecodePlan::new(&r, &c).unwrap();
    let ep = qnc_media_stream::HttpEndpoint::for_owner_endpoint(
        "http://127.0.0.1:1",
        &r.media.media_uri,
        "test-token",
    )
    .unwrap();
    let command = FfmpegAdapter::new("ffmpeg")
        .command(&r, &plan, &ep, "8-abc")
        .unwrap();
    let args: Vec<_> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    for key in ["-request_size", "-initial_request_size", "-short_seek_size"] {
        assert!(args.windows(2).any(|w| w[0] == key && w[1] == "1048576"));
    }
    assert!(
        args.windows(2)
            .any(|w| w[0] == "-multiple_requests" && w[1] == "1")
    );
    assert!(args.contains(&"-nofind_stream_info".into()));
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
    Decoder::open(
        request,
        media,
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

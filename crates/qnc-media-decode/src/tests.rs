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
    let plan = DecodePlan::new(&request, &config()).unwrap();
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
    assert_eq!(crate::plan::video_bytes(3, 3, "yuv420p").unwrap(), 17);
    assert_eq!(crate::plan::video_bytes(3, 3, "yuv422p10le").unwrap(), 42);
    assert!(crate::plan::video_bytes(0, 1080, "yuv420p").is_err());
    assert!(crate::plan::video_bytes(16, 16, "unknown").is_err());
}
#[test]
fn missing_metadata_and_memory_budget_fail_before_process_start() {
    let config = config();
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
    let c = config();
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

fn config() -> DecoderConfig {
    DecoderConfig::new(ExternalAdapter {
        adapter_id: "test.decoder".into(),
        executable: "not-launched".into(),
        args: vec![],
    })
}

fn protocol_decoder(mode: &str) -> (tempfile::TempDir, Decoder) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("clip.mkv"), b"transport fixture").unwrap();
    let exe = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("protocol_fixture{}", std::env::consts::EXE_SUFFIX));
    assert!(
        exe.is_file(),
        "build --example protocol_fixture before this explicit integration run"
    );
    let mut c = DecoderConfig::new(ExternalAdapter {
        adapter_id: "test.decoder".into(),
        executable: exe,
        args: vec![mode.into()],
    });
    c.read_timeout = std::time::Duration::from_secs(1);
    let source = qnc_media_stream::LocalSource::new("qnc://local/source/test", dir.path()).unwrap();
    let r = fixture();
    let stream = qnc_media_stream::MediaStream::local(&source, &r.media.media_uri).unwrap();
    let decoder = Decoder::open(r, stream, c).unwrap();
    (dir, decoder)
}
#[test]
#[ignore = "requires separately built protocol_fixture; no media probing"]
fn external_process_protocol_packets_identity_and_exact_eof() {
    let (_dir, mut decoder) = protocol_decoder("good");
    let pid = decoder.process_id();
    for ordinal in 0..5 {
        let p = decoder.next_packet().unwrap().unwrap();
        assert_eq!(p.ordinal, ordinal);
        assert_eq!(p.pts, ordinal as i64 * 40);
        assert_eq!(p.bytes, vec![ordinal as u8; 384]);
        assert_eq!(decoder.process_id(), pid);
    }
    assert!(decoder.next_packet().unwrap().is_none());
    assert!(decoder.process_id().is_none());
}
#[test]
#[ignore = "requires separately built protocol_fixture; no media probing"]
fn external_process_errors_and_timeouts_reap_children_without_fallback() {
    for mode in ["bad-version", "wrong-count", "exit-failure", "hang"] {
        let (_dir, mut decoder) = protocol_decoder(mode);
        while let Ok(Some(_)) = decoder.next_packet() {}
        assert!(decoder.next_packet().is_err(), "{mode}");
        assert!(decoder.process_id().is_none(), "{mode}");
    }
}
#[test]
#[ignore = "requires separately built protocol_fixture; no media probing"]
fn external_cancel_is_bounded_and_sessions_are_independent() {
    let (_one, mut first) = protocol_decoder("good");
    let (_two, mut second) = protocol_decoder("good");
    std::thread::sleep(std::time::Duration::from_millis(150));
    let started = std::time::Instant::now();
    first.cancel();
    assert!(started.elapsed() < std::time::Duration::from_secs(3));
    assert!(first.process_id().is_none());
    assert_eq!(second.next_packet().unwrap().unwrap().ordinal, 0);
    second.cancel();
}

use super::*;
const URI: &str = "qnc://local/source/card/Clip/a.MP4";
const DOC: &str = "qnc://local/artifact/probe-a";
fn fixture() -> Value {
    serde_json::from_str(include_str!("../tests/fixtures/full.json")).unwrap()
}
fn parse(v: &Value) -> Parsed {
    read(&v.to_string(), URI, DOC, "probe").unwrap()
}
fn clip(p: Parsed) -> ClipMetadata {
    ClipMetadata {
        contract_id: CONTRACT_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        clip_id: "clip".into(),
        evidence: vec![p.evidence],
        original: p.media,
        proxy: None,
    }
}

#[test]
fn all_streams_with_individual_timing_and_aac_not_pcm() {
    let p = parse(&fixture());
    assert_eq!(p.media.streams.len(), 4);
    assert_eq!(p.media.streams[0].start_pts.as_ref().unwrap().value, -512);
    assert_eq!(p.media.streams[2].start_pts.as_ref().unwrap().value, 1024);
    let StreamDetails::Audio(a) = &p.media.streams[1].details else {
        panic!()
    };
    assert_eq!(
        a.sample_format.as_ref().unwrap().value,
        Signal::Known("fltp".into())
    );
    assert!(a.bits_per_sample.is_none());
    assert_eq!(a.channels.as_ref().unwrap().value, 1);
    assert!(inspect(&clip(p)).is_complete());
}
#[test]
fn equal_average_and_nominal_rates_do_not_assert_constant_rate() {
    let p = parse(&fixture());
    let StreamDetails::Video(v) = &p.media.streams[0].details else {
        panic!()
    };
    assert_eq!(
        v.frame_rate_mode.as_ref().unwrap().value,
        FrameRateMode::Unknown
    );
    assert_eq!(v.exact_frame_count(), Some(100));
}
#[test]
fn inferred_count_remains_estimated_and_missing_rotation_is_not_zero() {
    let mut f = fixture();
    f["streams"][0].as_object_mut().unwrap().remove("nb_frames");
    f["streams"][0]
        .as_object_mut()
        .unwrap()
        .remove("side_data_list");
    let p = parse(&f);
    let StreamDetails::Video(v) = &p.media.streams[0].details else {
        panic!()
    };
    assert_eq!(
        v.frame_count.as_ref().unwrap().value,
        FrameCount::Estimated(100)
    );
    assert_eq!(v.exact_frame_count(), None);
    assert_eq!(
        v.rotation_degrees.as_ref().unwrap().value,
        Signal::Unspecified
    );
}
#[test]
fn absent_fps_and_colors_do_not_get_defaults() {
    let mut f = fixture();
    f["streams"][0]["avg_frame_rate"] = "0/0".into();
    f["streams"][0]["r_frame_rate"] = "0/0".into();
    f["streams"][0]
        .as_object_mut()
        .unwrap()
        .remove("color_primaries");
    f["streams"][0]["color_range"] = "unknown".into();
    let p = parse(&f);
    let StreamDetails::Video(v) = &p.media.streams[0].details else {
        panic!()
    };
    assert!(v.frame_rate.is_none());
    assert!(v.color.primaries.is_none());
    assert_eq!(v.color.range.as_ref().unwrap().value, Signal::Unspecified);
    assert!(!inspect(&clip(p)).is_complete());
}
#[test]
fn exact_decimal_rationals_and_ntsc_are_not_float_rounded() {
    assert_eq!(
        rational("123.456789"),
        Some(Rational {
            numerator: 123456789,
            denominator: 1000000
        })
    );
    let mut f = fixture();
    f["streams"][0]["avg_frame_rate"] = "30000/1001".into();
    let p = parse(&f);
    let StreamDetails::Video(v) = &p.media.streams[0].details else {
        panic!()
    };
    assert_eq!(
        v.frame_rate.as_ref().unwrap().value,
        FrameTimebase {
            fps_num: 30000,
            fps_den: 1001
        }
    );
    assert!(rational("18446744073709551615").is_none());
}
#[test]
fn malformed_counts_duplicate_indices_and_error_reports_reject() {
    for value in ["1.5", "-1", "18446744073709551616"] {
        let mut f = fixture();
        f["streams"][0]["nb_frames"] = value.into();
        assert!(read(&f.to_string(), URI, DOC, "p").is_err());
    }
    let mut f = fixture();
    f["streams"][1]["index"] = 0.into();
    assert!(read(&f.to_string(), URI, DOC, "p").is_err());
    let mut f = fixture();
    f["error"] = serde_json::json!({"code":-1});
    assert!(read(&f.to_string(), URI, DOC, "p").is_err());
}
#[test]
fn no_audio_is_complete_inventory_not_fake_audio() {
    let mut f = fixture();
    f["streams"].as_array_mut().unwrap().truncate(1);
    let p = parse(&f);
    assert_eq!(p.media.streams.len(), 1);
    assert!(inspect(&clip(p)).is_complete());
}
#[test]
fn audio_only_has_no_video_timebase() {
    let mut f = fixture();
    f["streams"] = serde_json::json!([f["streams"][1]]);
    let p = parse(&f);
    assert!(matches!(
        p.media.streams[0].details,
        StreamDetails::Audio(_)
    ));
    assert!(inspect(&clip(p)).is_complete());
}
#[test]
fn preserves_tags_empty_values_and_provenance() {
    let p = parse(&fixture());
    assert_eq!(p.media.tags["ffprobe:/format/tags/empty"].value, "");
    assert_eq!(
        p.media.tags["creation_time"].locator,
        "/format/tags/creation_time"
    );
    assert_eq!(
        p.media.streams[0].codec.as_ref().unwrap().evidence_id,
        "probe"
    );
}
#[test]
fn binding_limits_and_all_environments() {
    for uri in [
        "qnc://local/source/card/a",
        "qnc://lan/host/source/card/a",
        "qnc://intranet/host/source/card/a",
    ] {
        let mut f = fixture();
        f["format"]["filename"] = uri.into();
        assert!(read(&f.to_string(), uri, DOC, "p").is_ok());
    }
    assert!(read(&fixture().to_string(), "C:\\a.mp4", DOC, "p").is_err());
    assert!(read(
        &fixture().to_string(),
        "qnc://local/source/other/a",
        DOC,
        "p"
    )
    .is_err());
    assert!(matches!(
        read(&" ".repeat(MAX_DOCUMENT_BYTES + 1), URI, DOC, "p"),
        Err(Error::TooLarge)
    ));
}

#[test]
fn unrecognized_ancillary_codec_is_retained_with_tag_and_stream_identity() {
    let mut f = fixture();
    f["streams"][3]["codec_name"] = "unknown".into();
    f["streams"][3]["codec_tag_string"] = "rtmd".into();
    f["streams"][3]["codec_tag"] = "0x646d7472".into();
    let p = parse(&f);
    assert_eq!(p.media.streams.len(), 4);
    assert_eq!(
        p.media.streams[3].codec.as_ref().unwrap().value,
        Signal::Unspecified
    );
    assert!(
        matches!(&p.media.streams[3].details, StreamDetails::Other { stream_type } if stream_type.value == "data")
    );
    let tag = &p.media.tags["ffprobe:/streams/3/codec_tag_string"];
    assert_eq!(tag.value, "rtmd");
    assert_eq!(tag.locator, "/streams/3/codec_tag_string");
    assert_eq!(tag.evidence_id, "probe");
    assert!(inspect(&clip(p)).is_complete());

    f["streams"][3]
        .as_object_mut()
        .unwrap()
        .remove("codec_name");
    let missing = inspect(&clip(parse(&f)));
    assert!(missing
        .issues
        .iter()
        .any(|i| i.path == "original.streams[3].codec"));
}

#[test]
fn unknown_audio_video_codec_still_blocks_completeness() {
    for i in [0, 1] {
        let mut f = fixture();
        f["streams"][i]["codec_name"] = "unknown".into();
        let report = inspect(&clip(parse(&f)));
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.path == format!("original.streams[{i}].codec")
                && issue.code == IssueCode::Missing));
    }
}

#[test]
fn declared_unreadable_transform_is_not_reported_as_absent() {
    for side in [
        serde_json::json!({"side_data_type":"Display Matrix"}),
        serde_json::json!({"displaymatrix":"unreadable"}),
        serde_json::json!({"rotation":"N/A"}),
    ] {
        let mut f = fixture();
        f["streams"][0]["side_data_list"] = serde_json::json!([side]);
        let p = parse(&f);
        let StreamDetails::Video(v) = &p.media.streams[0].details else {
            panic!()
        };
        assert!(v.rotation_degrees.is_none());
        assert!(!inspect(&clip(p)).is_complete());
    }
    let mut f = fixture();
    f["streams"][0]
        .as_object_mut()
        .unwrap()
        .remove("side_data_list");
    f["streams"][0]["tags"] = serde_json::json!({"rotate":"90"});
    let p = parse(&f);
    let StreamDetails::Video(v) = &p.media.streams[0].details else {
        panic!()
    };
    assert!(v.rotation_degrees.is_none());
    assert_eq!(p.media.tags["ffprobe:/streams/0/tags/rotate"].value, "90");
}

#[test]
fn rotation_is_explicit_and_malformed_or_conflicting_side_data_is_rejected() {
    for rotation in [0, -90, 180] {
        let mut f = fixture();
        f["streams"][0]["side_data_list"] = serde_json::json!([{"rotation":rotation}]);
        let p = parse(&f);
        let StreamDetails::Video(v) = &p.media.streams[0].details else {
            panic!()
        };
        assert_eq!(
            v.rotation_degrees.as_ref().unwrap().value,
            Signal::Known(rotation)
        );
    }
    for sides in [
        serde_json::json!({"rotation":90}),
        serde_json::json!([null]),
        serde_json::json!([{"rotation":90},{"rotation":180}]),
        serde_json::json!([{"rotation":"broken"}]),
    ] {
        let mut f = fixture();
        f["streams"][0]["side_data_list"] = sides;
        assert!(read(&f.to_string(), URI, DOC, "p").is_err());
    }
}

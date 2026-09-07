use std::collections::BTreeMap;

use qnc_media_metadata::*;

fn fact<T>(id: &str, value: T) -> Fact<T> {
    Fact {
        value,
        evidence_id: id.into(),
        locator: "/fixture/field".into(),
    }
}

fn some<T>(id: &str, value: T) -> Option<Fact<T>> {
    Some(fact(id, value))
}

fn text(id: &str, value: &str) -> Option<Fact<String>> {
    some(id, value.to_string())
}

fn signal(id: &str, value: &str) -> Option<Fact<Signal<String>>> {
    some(id, Signal::Known(value.to_string()))
}

fn ratio(numerator: i64, denominator: i64) -> Rational {
    Rational {
        numerator,
        denominator,
    }
}

fn video(id: &str) -> MediaStream {
    MediaStream {
        index: some(id, 0),
        codec: signal(id, "h264"),
        profile: text(id, "High 4:2:2"),
        time_base: some(id, ratio(1, 50)),
        start_pts: some(id, 0),
        duration_ts: some(id, 500),
        details: StreamDetails::Video(Box::new(VideoMetadata {
            width: some(id, 1920),
            height: some(id, 1080),
            frame_rate: some(id, FrameTimebase::new(50, 1).unwrap()),
            frame_rate_mode: some(id, FrameRateMode::Constant),
            frame_count: some(id, FrameCount::Exact(500)),
            scan_mode: some(id, ScanMode::Progressive),
            pixel_format: text(id, "yuv422p10le"),
            sample_aspect_ratio: some(id, ratio(1, 1)),
            rotation_degrees: some(id, Signal::Known(0)),
            color: ColorMetadata {
                primaries: signal(id, "bt709"),
                transfer: signal(id, "bt709"),
                matrix: signal(id, "bt709"),
                range: signal(id, "tv"),
            },
        })),
    }
}

fn audio(id: &str, index: u32) -> MediaStream {
    MediaStream {
        index: some(id, index),
        codec: signal(id, "pcm_s24le"),
        profile: None,
        time_base: some(id, ratio(1, 48000)),
        start_pts: some(id, 0),
        duration_ts: some(id, 480000),
        details: StreamDetails::Audio(Box::new(AudioMetadata {
            sample_rate_hz: some(id, 48000),
            channels: some(id, 1),
            sample_format: signal(id, "s32"),
            channel_layout: signal(id, "mono"),
            bits_per_sample: some(id, 24),
        })),
    }
}

fn representation(id: &str, uri: &str) -> MediaRepresentation {
    MediaRepresentation {
        media_uri: uri.into(),
        container: text(id, "mxf"),
        duration_seconds: some(id, ratio(10, 1)),
        streams_complete: some(id, true),
        streams: vec![video(id), audio(id, 1), audio(id, 2)],
        tags: BTreeMap::from([(
            "creation_time".into(),
            fact(id, "2026-09-03T15:28:19+02:00".into()),
        )]),
    }
}

fn record(environment: &str, proxy: bool) -> ClipMetadata {
    let original_uri = format!("qnc://{environment}/media/original-1");
    let proxy_uri = format!("qnc://{environment}/media/proxy-1");
    let mut record = ClipMetadata {
        contract_id: CONTRACT_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        clip_id: "clip-1".into(),
        evidence: vec![Evidence {
            id: "original-camera".into(),
            kind: EvidenceKind::CameraMetadata,
            document_uri: format!("qnc://{environment}/artifact/camera-index-1"),
            media_uri: original_uri.clone(),
        }],
        original: representation("original-camera", &original_uri),
        proxy: None,
    };
    if proxy {
        record.evidence.push(Evidence {
            id: "proxy-probe".into(),
            kind: EvidenceKind::Ffprobe,
            document_uri: format!("qnc://{environment}/artifact/proxy-probe-1"),
            media_uri: proxy_uri.clone(),
        });
        let mut proxy = representation("proxy-probe", &proxy_uri);
        proxy.container = text("proxy-probe", "mov,mp4,m4a,3gp,3g2,mj2");
        proxy.streams.truncate(2);
        proxy.streams[0].codec = signal("proxy-probe", "hevc");
        if let StreamDetails::Video(v) = &mut proxy.streams[0].details {
            v.width = some("proxy-probe", 1280);
            v.height = some("proxy-probe", 720);
            v.pixel_format = text("proxy-probe", "yuv420p");
        }
        proxy.streams[1].codec = signal("proxy-probe", "aac");
        if let StreamDetails::Audio(a) = &mut proxy.streams[1].details {
            a.channels = some("proxy-probe", 2);
            a.sample_format = signal("proxy-probe", "fltp");
            a.channel_layout = signal("proxy-probe", "stereo");
            a.bits_per_sample = None;
        }
        record.proxy = Some(proxy);
    }
    record
}

fn video_mut(record: &mut ClipMetadata) -> &mut VideoMetadata {
    match &mut record.original.streams[0].details {
        StreamDetails::Video(video) => video,
        _ => panic!("fixture video"),
    }
}

fn issue(record: &ClipMetadata, path: &str, code: IssueCode) {
    let report = inspect(record);
    assert!(!report.is_complete());
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.path == path && i.code == code),
        "{report:?}"
    );
}

#[test]
fn one_contract_round_trips_in_all_transport_environments() {
    for environment in ["local", "lan/studio", "intranet/archive"] {
        let record = record(environment, true);
        let json = serde_json::to_string(&record).unwrap();
        let decoded: ClipMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(record, decoded);
        let report = inspect(&decoded);
        assert!(report.is_complete(), "{report:?}");
    }
}

#[test]
fn original_does_not_require_a_proxy() {
    let record = record("local", false);
    assert!(record.proxy.is_none());
    assert!(inspect(&record).is_complete());
}

#[test]
fn empty_optional_camera_tag_is_preserved_without_weakening_required_fields() {
    let mut record = record("local", false);
    record.original.tags.insert(
        "sony.index:mediaName".into(),
        fact("original-camera", String::new()),
    );
    assert!(inspect(&record).is_complete());
    assert_eq!(record.original.tags["sony.index:mediaName"].value, "");
    record.original.container = text("original-camera", "");
    assert!(!inspect(&record).is_complete());
    assert!(inspect(&record)
        .issues
        .iter()
        .any(|i| i.path == "original.container" && i.code == IssueCode::Invalid));
}

#[test]
fn proxy_codec_and_audio_stay_attached_to_the_same_clip() {
    let record = record("local", true);
    let proxy = record.proxy.as_ref().unwrap();
    assert_ne!(record.original.container, proxy.container);
    assert_eq!(
        record.original.streams[1].codec.as_ref().unwrap().value,
        Signal::Known("pcm_s24le".into())
    );
    assert_eq!(
        proxy.streams[1].codec.as_ref().unwrap().value,
        Signal::Known("aac".into())
    );
    let StreamDetails::Audio(audio) = &proxy.streams[1].details else {
        panic!()
    };
    assert!(audio.bits_per_sample.is_none());
    assert_eq!(
        audio.sample_format.as_ref().unwrap().value,
        Signal::Known("fltp".into())
    );
    assert!(inspect(&record).is_complete());
}

#[test]
fn stream_indices_channels_and_offsets_are_not_aggregated_or_rewritten() {
    let mut record = record("local", false);
    record.original.streams[1].start_pts.as_mut().unwrap().value = -1024;
    if let StreamDetails::Audio(a) = &mut record.original.streams[2].details {
        a.channels.as_mut().unwrap().value = 16;
        a.channel_layout.as_mut().unwrap().value = Signal::Unspecified;
    }
    let before = record.clone();
    assert!(inspect(&record).is_complete());
    assert_eq!(record, before);
    let indices: Vec<_> = record
        .original
        .streams
        .iter()
        .map(|s| s.index.as_ref().unwrap().value)
        .collect();
    assert_eq!(indices, [0, 1, 2]);
}

#[test]
fn confirmed_video_without_audio_needs_no_fake_audio_format() {
    let mut record = record("local", false);
    record.original.streams.truncate(1);
    assert!(inspect(&record).is_complete());
    record.original.streams_complete = None;
    issue(&record, "original.streams_complete", IssueCode::Missing);
    record.original.streams_complete = some("original-camera", false);
    issue(&record, "original.streams_complete", IssueCode::Missing);
}

#[test]
fn audio_only_needs_no_fake_video_fps() {
    let mut record = record("local", false);
    record.original.streams.remove(0);
    assert!(inspect(&record).is_complete());
    let json = serde_json::to_string(&record).unwrap();
    assert!(!json.contains("frame_rate"));
    assert!(!json.contains("width"));
}

#[test]
fn rational_fps_is_preserved_without_float_conversion() {
    let mut record = record("local", false);
    video_mut(&mut record).frame_rate =
        some("original-camera", FrameTimebase::new(30000, 1001).unwrap());
    let json = serde_json::to_string(&record).unwrap();
    let mut decoded: ClipMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(
        video_mut(&mut decoded).frame_rate.as_ref().unwrap().value,
        FrameTimebase::new(30000, 1001).unwrap()
    );
    assert!(inspect(&decoded).is_complete());
}

#[test]
fn unspecified_color_is_not_a_rec709_default() {
    let mut record = record("local", false);
    video_mut(&mut record).color.primaries = some("original-camera", Signal::Unspecified);
    assert!(inspect(&record).is_complete());
    video_mut(&mut record).color.primaries = None;
    issue(
        &record,
        "original.streams[0].video.color.primaries",
        IssueCode::Missing,
    );
    video_mut(&mut record).color.primaries = signal("original-camera", "unknown");
    issue(
        &record,
        "original.streams[0].video.color.primaries",
        IssueCode::Invalid,
    );
}

#[test]
fn missing_and_invalid_fields_are_distinct() {
    let mut record = record("local", false);
    video_mut(&mut record).frame_rate = None;
    video_mut(&mut record).width.as_mut().unwrap().value = 0;
    issue(
        &record,
        "original.streams[0].video.frame_rate",
        IssueCode::Missing,
    );
    issue(
        &record,
        "original.streams[0].video.width",
        IssueCode::Invalid,
    );
}

#[test]
fn absent_proxy_fact_is_not_filled_from_original() {
    let mut record = record("local", true);
    record.proxy.as_mut().unwrap().container = None;
    let before = record.clone();
    issue(&record, "proxy.container", IssueCode::Missing);
    assert_eq!(record, before);
}

#[test]
fn copying_original_fact_to_proxy_is_rejected() {
    let mut record = record("local", true);
    record.proxy.as_mut().unwrap().container = record.original.container.clone();
    issue(&record, "proxy.container", IssueCode::Invalid);
}

#[test]
fn same_camera_document_can_describe_two_different_representations() {
    let mut record = record("local", true);
    record.evidence[1].document_uri = record.evidence[0].document_uri.clone();
    record.evidence[1].kind = EvidenceKind::CameraMetadata;
    assert!(inspect(&record).is_complete());
}

#[test]
fn evidence_requires_valid_unique_id_reference_and_locator() {
    let mut record = record("local", false);
    record.original.container.as_mut().unwrap().evidence_id = "not-recorded".into();
    issue(&record, "original.container", IssueCode::Invalid);
    let mut record = self::record("local", false);
    record.evidence.push(record.evidence[0].clone());
    issue(&record, "evidence[1].id", IssueCode::Invalid);
    let mut record = self::record("local", false);
    record.original.container.as_mut().unwrap().locator.clear();
    issue(&record, "original.container.locator", IssueCode::Invalid);
}

#[test]
fn evidence_cannot_refer_to_unrelated_media() {
    let mut record = record("local", false);
    record.evidence[0].media_uri = "qnc://local/media/unrelated".into();
    issue(&record, "evidence[0].media_uri", IssueCode::Invalid);
}

#[test]
fn exact_proxy_frame_count_and_constant_rate_must_match() {
    let mut record = record("local", true);
    video_mut(&mut record).frame_count.as_mut().unwrap().value = FrameCount::Exact(501);
    issue(&record, "proxy.video.frame_count", IssueCode::Invalid);
    let mut record = self::record("local", true);
    video_mut(&mut record).frame_rate = some("original-camera", FrameTimebase::new(25, 1).unwrap());
    issue(&record, "proxy.video.frame_rate", IssueCode::Invalid);
}

#[test]
fn equivalent_rational_rates_do_not_false_conflict() {
    let mut record = record("local", true);
    video_mut(&mut record).frame_rate =
        some("original-camera", FrameTimebase::new(100, 2).unwrap());
    assert!(inspect(&record).is_complete());
}

#[test]
fn estimated_frame_count_and_variable_rate_do_not_claim_frame_accuracy() {
    let mut record = record("local", false);
    let video = video_mut(&mut record);
    assert_eq!(video.exact_frame_count(), Some(500));
    video.frame_count.as_mut().unwrap().value = FrameCount::Estimated(500);
    video.frame_rate_mode.as_mut().unwrap().value = FrameRateMode::Variable;
    assert_eq!(video.exact_frame_count(), None);
    assert!(inspect(&record).is_complete());
    let mut decoded: ClipMetadata =
        serde_json::from_str(&serde_json::to_string(&record).unwrap()).unwrap();
    assert_eq!(video_mut(&mut decoded).exact_frame_count(), None);
}

#[test]
fn raw_os_paths_are_rejected_in_media_and_evidence() {
    for path in [
        r"C:\media\clip.mxf",
        r"\\server\share\clip.mxf",
        "/mnt/card/clip.mxf",
        "/Volumes/card/clip.mxf",
        "file:///media/clip.mxf",
        "qnc://local/media/../clip",
        "qnc://local/media/C:/clip",
    ] {
        let mut record = record("local", false);
        record.original.media_uri = path.into();
        issue(&record, "original.media_uri", IssueCode::Invalid);
        let mut record = self::record("local", false);
        record.evidence[0].document_uri = path.into();
        issue(&record, "evidence[0].document_uri", IssueCode::Invalid);
    }
}

#[test]
fn duplicate_stream_indices_are_rejected_not_merged() {
    let mut record = record("local", false);
    record.original.streams[2].index.as_mut().unwrap().value = 1;
    issue(&record, "original.streams[2].index", IssueCode::Invalid);
}

#[test]
fn unknown_contract_or_version_is_rejected_without_migration() {
    let mut record = record("local", false);
    record.contract_id = "other.media".into();
    issue(&record, "contract_id", IssueCode::Invalid);
    record.contract_version = "0.0.1".into();
    issue(&record, "contract_version", IssueCode::Invalid);
    record.contract_id = CONTRACT_ID.into();
    record.contract_version = "0.1.0".into();
    issue(&record, "contract_version", IssueCode::Invalid);
}

#[test]
fn undeclared_rotation_is_distinct_from_zero_and_missing() {
    let mut record = record("local", false);
    let known = video_mut(&mut record).rotation_degrees.clone();
    video_mut(&mut record).rotation_degrees = some("original-camera", Signal::Unspecified);
    assert_ne!(video_mut(&mut record).rotation_degrees, known);
    assert!(inspect(&record).is_complete());
    video_mut(&mut record).rotation_degrees = None;
    issue(
        &record,
        "original.streams[0].video.rotation_degrees",
        IssueCode::Missing,
    );
}

#[test]
fn unknown_json_fields_and_missing_original_are_rejected() {
    let record = record("local", false);
    let mut json = serde_json::to_value(&record).unwrap();
    json["project_id"] = "not-part-of-this-contract".into();
    assert!(serde_json::from_value::<ClipMetadata>(json).is_err());
    let mut json = serde_json::to_value(&record).unwrap();
    json["original"]["streams"][0]["details"]["metadata"]["default_fps"] = 25.into();
    assert!(serde_json::from_value::<ClipMetadata>(json).is_err());
    let mut json = serde_json::to_value(&record).unwrap();
    json.as_object_mut().unwrap().remove("original");
    assert!(serde_json::from_value::<ClipMetadata>(json).is_err());
}

#[test]
fn rejects_zero_rates_durations_channels_and_frame_count_overflow() {
    let mut record = record("local", false);
    video_mut(&mut record)
        .frame_rate
        .as_mut()
        .unwrap()
        .value
        .fps_den = 0;
    video_mut(&mut record).frame_count.as_mut().unwrap().value = FrameCount::Exact(u64::MAX);
    record.original.streams[0]
        .duration_ts
        .as_mut()
        .unwrap()
        .value = 0;
    record
        .original
        .duration_seconds
        .as_mut()
        .unwrap()
        .value
        .denominator = 0;
    if let StreamDetails::Audio(audio) = &mut record.original.streams[1].details {
        audio.channels.as_mut().unwrap().value = 0;
        audio.sample_rate_hz.as_mut().unwrap().value = 0;
    }
    issue(
        &record,
        "original.streams[0].video.frame_rate",
        IssueCode::Invalid,
    );
    issue(
        &record,
        "original.streams[0].video.frame_count",
        IssueCode::Invalid,
    );
    issue(
        &record,
        "original.streams[0].duration_ts",
        IssueCode::Invalid,
    );
    issue(&record, "original.duration_seconds", IssueCode::Invalid);
    issue(
        &record,
        "original.streams[1].audio.channels",
        IssueCode::Invalid,
    );
    issue(
        &record,
        "original.streams[1].audio.sample_rate_hz",
        IssueCode::Invalid,
    );
}

#[test]
fn module_manifest_and_db_payload_reference_match_the_executable_contract() {
    let manifest_text = include_str!("../../../contracts/modules/media-metadata.module.json");
    let report = qnc_contracts::validate_module_manifest_json("media-metadata", manifest_text);
    assert!(report.is_ok(), "{report:?}");
    let manifest: serde_json::Value = serde_json::from_str(manifest_text).unwrap();
    let database: serde_json::Value = serde_json::from_str(include_str!(
        "../../../contracts/databases/ingest-content.database.json"
    ))
    .unwrap();
    let payload = &database["record_contracts"]["public_probe_records.probe_json"];
    assert_eq!(payload["contract_id"], CONTRACT_ID);
    assert_eq!(payload["contract_version"], CONTRACT_VERSION);
    assert_eq!(manifest["record_contract"], CONTRACT_ID);
    assert_eq!(manifest["runtime_crate"], "qnc-media-metadata");
    assert_eq!(manifest["database_write_policy"], "no_db_writes");
}

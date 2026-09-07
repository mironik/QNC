use super::*;
const URI: &str = "qnc://local/source/card/Clip/a.MP4";
fn parsed() -> Parsed {
    qnc_ffprobe_metadata::read(
        include_str!("../../qnc-ffprobe-metadata/tests/fixtures/full.json"),
        URI,
        "qnc://local/artifact/probe-a",
        "probe",
    )
    .unwrap()
}
fn camera() -> ClipMetadata {
    let p = parsed();
    let mut c = ClipMetadata {
        contract_id: CONTRACT_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        clip_id: "clip".into(),
        evidence: vec![Evidence {
            kind: EvidenceKind::CameraMetadata,
            ..p.evidence
        }],
        original: p.media,
        proxy: None,
    };
    c.evidence[0].id = "camera".into();
    let mut value = serde_json::to_value(&c).unwrap();
    fn rename(v: &mut Value) {
        match v {
            Value::Object(m) => {
                if let Some(id) = m.get_mut("evidence_id") {
                    *id = "camera".into();
                }
                for v in m.values_mut() {
                    rename(v);
                }
            }
            Value::Array(a) => {
                for v in a {
                    rename(v);
                }
            }
            _ => {}
        }
    }
    rename(&mut value);
    c = serde_json::from_value(value).unwrap();
    c.original.streams.truncate(1);
    c.original.streams_complete = None;
    c.original.streams[0].index = None;
    c.original.container.as_mut().unwrap().value = "mp4".into();
    c
}
#[test]
fn merges_unique_video_keeps_camera_facts_and_adds_all_audio() {
    let c = camera();
    let result = compose(&c, &[parsed()]).unwrap();
    assert_eq!(result.original.streams.len(), 4);
    assert_eq!(
        result.original.streams[0].codec,
        c.original.streams[0].codec
    );
    assert_eq!(
        result.original.streams[0]
            .index
            .as_ref()
            .unwrap()
            .evidence_id,
        "probe"
    );
    assert!(inspect(&result).is_complete());
    assert_eq!(c.original.streams.len(), 1);
}
#[test]
fn equivalent_rationals_and_exact_camera_count_are_preserved() {
    let mut c = camera();
    c.original.duration_seconds.as_mut().unwrap().value = Rational {
        numerator: 100,
        denominator: 25,
    };
    let StreamDetails::Video(v) = &mut c.original.streams[0].details else {
        panic!()
    };
    v.frame_rate_mode.as_mut().unwrap().value = FrameRateMode::Constant;
    let mut p = parsed();
    let StreamDetails::Video(v) = &mut p.media.streams[0].details else {
        panic!()
    };
    v.frame_count.as_mut().unwrap().value = FrameCount::Estimated(99);
    let r = compose(&c, &[p]).unwrap();
    let StreamDetails::Video(v) = &r.original.streams[0].details else {
        panic!()
    };
    assert_eq!(v.exact_frame_count(), Some(100));
    assert_eq!(
        v.frame_rate_mode.as_ref().unwrap().value,
        FrameRateMode::Constant
    );
}
#[test]
fn conflicts_return_both_facts_without_overwriting_camera() {
    let c = camera();
    let mut p = parsed();
    p.media.streams[0].codec.as_mut().unwrap().value = Signal::Known("hevc".into());
    let Err(Error::Conflicts(conflicts)) = compose(&c, &[p]) else {
        panic!()
    };
    assert!(conflicts
        .iter()
        .any(|c| c.existing["value"]["value"] == "h264" && c.incoming["value"]["value"] == "hevc"));
    assert_eq!(
        c.original.streams[0].codec.as_ref().unwrap().value,
        Signal::Known("h264".into())
    );
}
#[test]
fn no_positional_join_with_two_video_candidates() {
    let c = camera();
    let mut p = parsed();
    let mut video = p.media.streams[0].clone();
    video.index.as_mut().unwrap().value = 10;
    p.media.streams.push(video);
    assert_eq!(compose(&c, &[p]), Err(Error::AmbiguousStream));
}
#[test]
fn duplicate_probe_unrelated_uri_and_evidence_rejected() {
    let c = camera();
    let p = parsed();
    assert_eq!(
        compose(&c, &[p.clone(), p.clone()]),
        Err(Error::DuplicateProbe)
    );
    let mut p = p;
    p.evidence.id = "camera".into();
    assert_eq!(compose(&c, &[p]), Err(Error::InvalidInput));
}
#[test]
fn plan_reads_snapshot_and_never_reprobes_final_partial() {
    let c = camera();
    let mut s = Snapshot {
        binding: qnc_media_records::Binding {
            source_index_uri: "qnc://local/db/source_index".into(),
            source_record_id: "source-one".into(),
            original_uri: URI.into(),
            proxy_uri: None,
        },
        revision: 1,
        phase: Phase::Camera,
        completeness: qnc_media_records::Completeness::Partial,
        report: inspect(&c),
        metadata: c,
        recorded_at_unix_ms: 1,
    };
    assert_eq!(required_probes(&s).unwrap(), vec![URI]);
    s.phase = Phase::Final;
    assert_eq!(required_probes(&s), Err(Error::Finalized));
}
#[test]
fn complete_camera_snapshot_needs_no_probe() {
    let mut c = camera();
    c.original.streams_complete = Some(Fact {
        value: true,
        evidence_id: "camera".into(),
        locator: "/streams".into(),
    });
    c.original.streams[0].index = Some(Fact {
        value: 0,
        evidence_id: "camera".into(),
        locator: "/streams/0/index".into(),
    });
    let s = Snapshot {
        binding: qnc_media_records::Binding {
            source_index_uri: "qnc://local/db/source_index".into(),
            source_record_id: "one".into(),
            original_uri: URI.into(),
            proxy_uri: None,
        },
        revision: 1,
        phase: Phase::Camera,
        completeness: qnc_media_records::Completeness::Complete,
        report: inspect(&c),
        metadata: c,
        recorded_at_unix_ms: 1,
    };
    assert!(required_probes(&s).unwrap().is_empty());
}

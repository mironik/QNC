use super::*;
use qnc_media_metadata::{IssueCode, MetadataIssue};

/// A stored record of a Sony ILME-FX3A clip (original H.264, proxy HEVC), final and complete.
fn fx3a() -> Snapshot {
    serde_json::from_str(include_str!("fx3a_snapshot.json")).unwrap()
}

fn missing(snapshot: &mut Snapshot, path: &str) {
    snapshot.report.issues.push(MetadataIssue { path: path.into(), code: IssueCode::Missing, message: "required metadata is missing".into() });
}

const ORIGINAL: Needs = Needs { playback: Playback::Original, copies_proxy: false };
const PROXY: Needs = Needs { playback: Playback::Proxy, copies_proxy: false };
const PROXY_IF: Needs = Needs { playback: Playback::ProxyIfAvailable, copies_proxy: false };

#[test]
fn a_gap_in_the_proxy_stops_only_a_project_that_uses_the_proxy() {
    let mut clip = fx3a();
    assert_eq!(conflict(&clip, ORIGINAL), None);
    missing(&mut clip, "proxy.streams[0].video.scan_mode");
    assert_eq!(conflict(&clip, ORIGINAL), None, "the original is played: the proxy is not in the way");
    let gap = Some(Conflict::ProxyIncomplete { missing: vec!["proxy.streams[0].video.scan_mode".into()] });
    assert_eq!(conflict(&clip, PROXY), gap);
    assert_eq!(conflict(&clip, PROXY_IF), gap, "the clip has a proxy, so it would be played");
    assert_eq!(conflict(&clip, Needs { copies_proxy: true, ..ORIGINAL }), gap, "the copied proxy is played later");
}

#[test]
fn a_gap_in_the_original_stops_every_project() {
    let mut clip = fx3a();
    missing(&mut clip, "original.streams[0].video.scan_mode");
    for needs in [ORIGINAL, PROXY, PROXY_IF] {
        assert!(matches!(conflict(&clip, needs), Some(Conflict::OriginalIncomplete { .. })));
    }
}

#[test]
fn a_clip_without_a_proxy_stops_only_a_project_that_needs_one() {
    let mut clip = fx3a();
    clip.metadata.proxy = None;
    assert_eq!(conflict(&clip, ORIGINAL), None);
    assert_eq!(conflict(&clip, PROXY_IF), None, "proxy if available plays the original");
    assert_eq!(conflict(&clip, PROXY), Some(Conflict::ProxyMissing));
    assert_eq!(conflict(&clip, Needs { copies_proxy: true, ..ORIGINAL }), Some(Conflict::ProxyMissing));
}

#[test]
fn a_card_record_waits_for_its_completion_except_for_a_missing_proxy() {
    let mut clip = fx3a();
    clip.phase = Phase::Camera;
    clip.metadata.evidence.retain(|e| e.kind != EvidenceKind::Ffprobe);
    missing(&mut clip, "original.streams");
    assert_eq!(conflict(&clip, ORIGINAL), None);
    clip.metadata.proxy = None;
    assert_eq!(conflict(&clip, PROXY), Some(Conflict::ProxyMissing));
}

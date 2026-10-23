use super::*;
use qnc_camera_profiles::Profile;
use std::path::Path;

/// The profiles shipped with QNC.
fn profiles() -> Vec<Profile> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/cameras/sony");
    std::fs::read_dir(dir).unwrap().map(|e| serde_json::from_str(&std::fs::read_to_string(e.unwrap().path()).unwrap()).unwrap()).collect()
}

/// A stored record of a Sony ILME-FX3A clip, as the Sony format reads it (without a profile tag).
fn fx3a_metadata() -> ClipMetadata {
    let snapshot: serde_json::Value = serde_json::from_str(include_str!("../../qnc-import-fit/src/fx3a_snapshot.json")).unwrap();
    let mut metadata: ClipMetadata = serde_json::from_value(snapshot["metadata"].clone()).unwrap();
    metadata.original.tags.remove(RECORD_TAG);
    metadata
}

/// The Sony format with its record of one clip given.
struct Given(ClipMetadata);
impl RecordFormat for Given {
    fn index(&self) -> &dyn IndexReader { &qnc_sony_metadata::SonyIndexReader }
    fn documents(&self, _: &GroupProposal) -> Vec<SourceReference> { vec![] }
    fn thumbnail(&self, _: &GroupProposal) -> Option<SourceReference> { None }
    fn metadata(&self, _: &str, _: &GroupProposal, _: &[IndexDocument]) -> Result<ClipMetadata, String> { Ok(self.0.clone()) }
}

fn group() -> GroupProposal {
    serde_json::from_value(serde_json::json!({
        "root": { "source_uri": "qnc://local/source/card", "relative_path": "PRIVATE/M4ROOT" },
        "recording_identity": "x", "evidence": { "reader_id": "camera.sony.index.read", "document": { "source_uri": "qnc://local/source/card", "relative_path": "PRIVATE/M4ROOT/MEDIAPRO.XML" }, "locator": "/" },
        "original": { "source_uri": "qnc://local/source/card", "relative_path": "PRIVATE/M4ROOT/CLIP/C0012.MP4" }, "proxies": [], "related": []
    }))
    .unwrap()
}

fn sidecar(model: &str) -> IndexDocument {
    IndexDocument {
        reference: serde_json::from_value(serde_json::json!({ "source_uri": "qnc://local/source/card", "relative_path": "PRIVATE/M4ROOT/CLIP/C0012M01.XML" })).unwrap(),
        text: format!(r#"<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.20"><Device manufacturer="Sony" modelName="{model}"/></NonRealTimeMeta>"#),
    }
}

#[test]
fn the_record_names_the_profile_of_the_recognised_camera_or_the_default_one() {
    let adapter = ProfileAdapter::new(Arc::new(Given(fx3a_metadata())), &profiles()).unwrap();
    let read = adapter.metadata("clip-1", &group(), &[sidecar("ILME-FX3A")]).unwrap();
    let tag = &read.original.tags[RECORD_TAG];
    assert_eq!((tag.value.as_str(), tag.locator.as_str()), ("sony-ilme-fx3a@1", "camera-profile:recognised"));
    assert_eq!(adapter.sufficiency(&read), MetadataSufficiency::Declared);
    let unknown = adapter.metadata("clip-1", &group(), &[sidecar("ILME-FX30")]).unwrap();
    assert_eq!(unknown.original.tags[RECORD_TAG].value, "sony-mediaprofile-default@1");
    assert_eq!(unknown.original.tags[RECORD_TAG].locator, "camera-profile:default");
    assert_eq!(read.original.streams, fx3a_metadata().original.streams, "the format's record is kept as it is");
}

#[test]
fn a_profile_that_probes_once_probes_and_a_format_without_a_default_profile_is_refused() {
    let mut list = profiles();
    list.iter_mut().find(|p| p.id == "sony-ilme-fx3a").unwrap().procedure.metadata = MetadataRule::ProbeOnce;
    let adapter = ProfileAdapter::new(Arc::new(Given(fx3a_metadata())), &list).unwrap();
    let read = adapter.metadata("clip-1", &group(), &[sidecar("ILME-FX3A")]).unwrap();
    assert_eq!(adapter.sufficiency(&read), MetadataSufficiency::NeedsProbe);
    list.retain(|p| !p.default_for_reader);
    assert!(ProfileAdapter::new(Arc::new(Given(fx3a_metadata())), &list).is_err());
    assert!(ProfileAdapter::new(Arc::new(Given(fx3a_metadata())), &profiles()).unwrap().pattern_ids().contains(&"sony-m4root-sd"));
}

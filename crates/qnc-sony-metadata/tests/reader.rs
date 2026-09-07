use qnc_media_metadata::{
    inspect, EvidenceKind, FrameTimebase, MediaRepresentation, Signal, StreamDetails, VideoMetadata,
};
use qnc_sony_metadata::*;

const INDEX: &str = include_str!("fixtures/MEDIAPRO.XML");
const SIDECAR: &str = include_str!("fixtures/TEST-AM01.XML");

fn index(text: &str) -> Result<CameraIndex, String> {
    read_index(&XmlDocument {
        document_uri: "qnc://local/artifact/index-1".into(),
        text: text.into(),
    })
}

fn bound() -> ClipBinding {
    ClipBinding {
        clip_id: "clip-1".into(),
        original: BoundMedia {
            relative_path: "Clip/TEST A.MXF".into(),
            media_uri: "qnc://local/media/original-1".into(),
        },
        proxy: Some(BoundMedia {
            relative_path: "Sub/TEST AS03.MP4".into(),
            media_uri: "qnc://local/media/proxy-1".into(),
        }),
    }
}

fn sidecar(text: &str) -> SidecarDocument {
    SidecarDocument {
        relative_path: "Clip/TEST AM01.XML".into(),
        document: XmlDocument {
            document_uri: "qnc://local/artifact/sidecar-1".into(),
            text: text.into(),
        },
    }
}

fn read(text: &str) -> MetadataRead {
    read_metadata(&index(INDEX).unwrap(), 0, &bound(), Some(&sidecar(text))).unwrap()
}

fn video(media: &MediaRepresentation) -> &VideoMetadata {
    match &media.streams[0].details {
        StreamDetails::Video(video) => video,
        _ => panic!(),
    }
}

#[test]
fn index_uses_parent_links_not_matching_proxy_umid_or_suffix() {
    let index = index(INDEX).unwrap();
    assert_eq!(index.materials.len(), 2);
    let material = &index.materials[0];
    assert_eq!(material.original.relative_path, "Clip/TEST A.MXF");
    assert_eq!(material.proxies[0].relative_path, "Sub/TEST AS03.MP4");
    assert_ne!(
        material.original.attributes["umid"],
        material.proxies[0].attributes["umid"]
    );
    assert_eq!(material.related[1].kind, "JPG");
    assert_eq!(material.related[1].relative_path, "Thmbnl/TEST AT01.JPG");
}

#[test]
fn maps_normal_progressive_source_without_fabricating_full_probe() {
    let result = read(SIDECAR);
    let original = &result.metadata.original;
    let v = video(original);
    assert_eq!(v.width.as_ref().unwrap().value, 1920);
    assert_eq!(v.height.as_ref().unwrap().value, 1080);
    assert_eq!(
        v.frame_rate.as_ref().unwrap().value,
        FrameTimebase::new(50, 1).unwrap()
    );
    assert_eq!(v.exact_frame_count(), Some(500));
    assert!(original.duration_seconds.is_none());
    assert!(original.streams[0].index.is_none());
    assert!(original.streams[0].time_base.is_none());
    assert!(original.streams[0].start_pts.is_none());
    assert!(original.streams_complete.is_none());
    assert!(v.pixel_format.is_none());
    assert!(v.color.primaries.is_none());
    assert!(original
        .streams
        .iter()
        .all(|s| !matches!(s.details, StreamDetails::Audio(_))));
    assert!(!inspect(&result.metadata).is_complete());
    assert!(result
        .metadata
        .evidence
        .iter()
        .all(|e| e.kind == EvidenceKind::CameraMetadata));
}

#[test]
fn creation_offset_camera_serial_audio_ports_and_ltc_are_preserved_as_facts() {
    let result = read(SIDECAR);
    let tags = &result.metadata.original.tags;
    assert_eq!(tags["creation_time"].value, "2026-09-03T15:28:19+02:00");
    assert!(tags.values().any(|f| f.value == "TEST-CAMERA-SERIAL"));
    assert!(tags
        .iter()
        .any(|(k, f)| k.ends_with("LtcChangeTable[1]/@tcFps") && f.value == "25"));
    for channel in ["CH1", "CH2", "CH3", "CH4"] {
        assert!(tags.values().any(|f| f.value == channel));
    }
    assert_eq!(
        video(&result.metadata.original)
            .frame_rate
            .as_ref()
            .unwrap()
            .value
            .fps_num,
        50
    );
}

#[test]
fn foreign_namespace_attributes_cannot_overwrite_sony_facts() {
    let text = SIDECAR.replace(
        "</NonRealTimeMeta>",
        r#"<CreationDate xmlns="urn:test:foreign" value="foreign-date"/>
        <CreationDate xmlns="" value="unqualified-date"/>
        </NonRealTimeMeta>"#,
    );
    let result = read(&text);
    let tags = &result.metadata.original.tags;
    assert_eq!(tags["creation_time"].value, "2026-09-03T15:28:19+02:00");
    for (suffix, value) in [
        ("/CreationDate[1]/@value", "2026-09-03T15:28:19+02:00"),
        ("/{urn:test:foreign}CreationDate[1]/@value", "foreign-date"),
        ("/{}CreationDate[1]/@value", "unqualified-date"),
    ] {
        let matches: Vec<_> = tags
            .iter()
            .filter(|(key, _)| key.ends_with(suffix))
            .collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1.value, value);
        assert!(matches[0].1.locator.ends_with(suffix));
    }
}

#[test]
fn proxy_retains_its_container_codec_and_own_evidence() {
    let result = read(SIDECAR);
    let proxy = result.metadata.proxy.as_ref().unwrap();
    assert_eq!(proxy.container.as_ref().unwrap().value, "mp4");
    assert_eq!(
        proxy.streams[0].codec.as_ref().unwrap().value,
        Signal::Known("h264".into())
    );
    assert!(proxy.tags.values().any(|f| f.value == "AAC-LC"));
    assert!(proxy
        .tags
        .iter()
        .any(|(k, f)| k.ends_with("/@ch") && f.value == "2"));
    assert!(proxy.tags.values().all(|f| f.evidence_id == "index-proxy"));
    assert!(video(proxy).width.is_none());
    assert_eq!(video(proxy).exact_frame_count(), Some(500));
}

#[test]
fn missing_proxy_fps_duration_and_channels_are_not_inherited() {
    let index = index(INDEX).unwrap();
    let mut binding = bound();
    binding.original.relative_path = "Clip/TEST B.MXF".into();
    binding.proxy.as_mut().unwrap().relative_path = "Sub/TEST BS03.MP4".into();
    let mut sidecar = sidecar(
        &SIDECAR
            .replace("ORIGINAL-A", "ORIGINAL-B")
            .replace("value=\"500\"", "value=\"100\""),
    );
    sidecar.relative_path = "Clip/TEST BM01.XML".into();
    let result = read_metadata(&index, 1, &binding, Some(&sidecar)).unwrap();
    let proxy = result.metadata.proxy.unwrap();
    assert!(video(&proxy).frame_rate.is_none());
    assert!(video(&proxy).frame_count.is_none());
    assert!(proxy.duration_seconds.is_none());
    assert!(!proxy.tags.keys().any(|k| k.ends_with("/@ch")));
}

#[test]
fn original_without_proxy_is_supported() {
    let mut index = index(INDEX).unwrap();
    index.materials[0].proxies.clear();
    let mut binding = bound();
    binding.proxy = None;
    assert!(read_metadata(&index, 0, &binding, Some(&sidecar(SIDECAR)))
        .unwrap()
        .metadata
        .proxy
        .is_none());
}

#[test]
fn missing_or_wrong_bindings_are_not_silently_dropped() {
    let index = index(INDEX).unwrap();
    let mut binding = bound();
    binding.proxy = None;
    assert!(read_metadata(&index, 0, &binding, None).is_err());
    let mut binding = bound();
    binding.original.relative_path = "Sub/TEST AS03.MP4".into();
    assert!(read_metadata(&index, 0, &binding, None).is_err());
}

#[test]
fn rejects_wrong_sidecar_identity_reference_and_namespace() {
    let index = index(INDEX).unwrap();
    assert!(read_metadata(
        &index,
        0,
        &bound(),
        Some(&sidecar(&SIDECAR.replace("ORIGINAL-A", "PROXY-A")))
    )
    .is_err());
    let mut other = sidecar(SIDECAR);
    other.relative_path = "Clip/OTHER.XML".into();
    assert!(read_metadata(&index, 0, &bound(), Some(&other)).is_err());
    assert!(read_metadata(
        &index,
        0,
        &bound(),
        Some(&sidecar(&SIDECAR.replace("ver.2.20", "ver.9.99")))
    )
    .is_err());
}

#[test]
fn conflicting_duration_and_fps_remain_unresolved() {
    let result = read(&SIDECAR.replace("value=\"500\"", "value=\"501\""));
    assert!(result
        .notices
        .iter()
        .any(|n| n.code == "conflict" && n.field == "original.frame_count"));
    assert!(video(&result.metadata.original).frame_count.is_none());
    let result = read(&SIDECAR.replace("formatFps=\"50p\"", "formatFps=\"25p\""));
    assert!(result
        .notices
        .iter()
        .any(|n| n.code == "conflict" && n.field == "original.frame_rate"));
    assert!(video(&result.metadata.original).frame_rate.is_none());
}

#[test]
fn slow_quick_interlaced_and_ntsc_are_not_silently_normalized() {
    for text in [
        SIDECAR.replace("type=\"normal\"", "type=\"slow\""),
        SIDECAR.replace("captureFps=\"50.00p\"", "captureFps=\"100p\""),
        SIDECAR.replace("formatFps=\"50p\"", "formatFps=\"50i\""),
        SIDECAR.replace("formatFps=\"50p\"", "formatFps=\"29.97p\""),
    ] {
        assert!(video(&read(&text).metadata.original).frame_count.is_none());
    }
}

#[test]
fn missing_sidecar_does_not_discard_index_data() {
    let result = read_metadata(&index(INDEX).unwrap(), 0, &bound(), None).unwrap();
    assert_eq!(
        video(&result.metadata.original)
            .frame_rate
            .as_ref()
            .unwrap()
            .value
            .fps_num,
        50
    );
    assert!(video(&result.metadata.original).frame_count.is_none());
    assert!(!result.metadata.original.tags.is_empty());
}

#[test]
fn prefixed_xml_namespaces_work_but_foreign_material_is_rejected() {
    let text =
        format!("<s:MediaProfile xmlns:s=\"{INDEX_NAMESPACE}\"><s:Contents/></s:MediaProfile>");
    assert!(index(&text).unwrap().materials.is_empty());
    assert!(index(&INDEX.replace("<Material ", "<Material xmlns=\"urn:other\" ")).is_err());
    assert!(index(&INDEX.replace(INDEX_NAMESPACE, "urn:other")).is_err());
}

#[test]
fn rejects_duplicate_media_paths_and_original_identities() {
    assert!(index(&INDEX.replace("./Sub/TEST BS03.MP4", "./Sub/TEST AS03.MP4")).is_err());
    assert!(index(&INDEX.replace("ORIGINAL-B", "ORIGINAL-A")).is_err());
    assert!(index(&INDEX.replace("./Sub/TEST AS03.MP4", "./Clip/TEST A.MXF")).is_err());
}

#[test]
fn multiple_proxies_are_preserved_in_index_but_not_arbitrarily_selected() {
    let index = index(&INDEX.replace(
        "<RelevantInfo uri=\"./Clip/TEST AM01.XML\"",
        "<Proxy uri=\"./Sub/other.MP4\" type=\"MP4\"/><RelevantInfo uri=\"./Clip/TEST AM01.XML\"",
    ))
    .unwrap();
    assert_eq!(index.materials[0].proxies.len(), 2);
    assert!(read_metadata(&index, 0, &bound(), None).is_err());
}

#[test]
fn relative_references_preserve_case_spaces_and_decode_entities_once() {
    assert_eq!(
        relative_reference("./Clip/TEST%20A.MXF").unwrap(),
        "Clip/TEST A.MXF"
    );
    let index = index(&INDEX.replace("TEST A.MXF", "TEST &amp; A.MXF")).unwrap();
    assert_eq!(
        index.materials[0].original.relative_path,
        "Clip/TEST & A.MXF"
    );
}

#[test]
fn rejects_paths_escaping_recording_root_and_double_encoding() {
    for path in [
        "../clip.mxf",
        "Clip/../clip.mxf",
        "/clip.mxf",
        "C:/clip.mxf",
        r"\\server\clip.mxf",
        "https://host/clip",
        "Clip%2Fclip.mxf",
        "Clip/%2e%2e/clip.mxf",
        "Clip/%252e%252e/clip",
        "Clip/clip?query",
        "Clip/clip#part",
        "Clip/%00clip",
        "Clip/%zz",
        "Clip//clip",
    ] {
        assert!(relative_reference(path).is_err(), "{path}");
    }
}

#[test]
fn rejects_dtd_xxe_invalid_xml_and_resource_exhaustion() {
    for text in [
        "<!DOCTYPE MediaProfile SYSTEM 'file:///private'><MediaProfile/>",
        "<!DOCTYPE MediaProfile [<!ENTITY a 'repeat'>]><MediaProfile>&a;</MediaProfile>",
        "<MediaProfile>",
        "<root/><root/>",
    ] {
        assert!(index(text).is_err());
    }
    assert!(index(&" ".repeat(MAX_XML_BYTES + 1)).is_err());
    let deep = format!(
        "<MediaProfile xmlns=\"{INDEX_NAMESPACE}\">{}{}</MediaProfile>",
        "<n>".repeat(65),
        "</n>".repeat(65)
    );
    assert!(index(&deep).is_err());
}

#[test]
fn duplicate_sidecar_fields_are_rejected_instead_of_first_wins() {
    let text = SIDECAR.replace(
        "<Duration value=\"500\"/>",
        "<Duration value=\"500\"/><Duration value=\"600\"/>",
    );
    assert!(read_metadata(&index(INDEX).unwrap(), 0, &bound(), Some(&sidecar(&text))).is_err());
}

#[test]
fn same_parser_contract_works_for_local_lan_intranet_bindings() {
    for prefix in ["local", "lan/studio", "intranet/archive"] {
        let mut index = index(INDEX).unwrap();
        index.document_uri = format!("qnc://{prefix}/artifact/index");
        let mut binding = bound();
        binding.original.media_uri = format!("qnc://{prefix}/media/original");
        binding.proxy.as_mut().unwrap().media_uri = format!("qnc://{prefix}/media/proxy");
        let mut sidecar = sidecar(SIDECAR);
        sidecar.document.document_uri = format!("qnc://{prefix}/artifact/sidecar");
        let result = read_metadata(&index, 0, &binding, Some(&sidecar)).unwrap();
        let json = serde_json::to_string(&result).unwrap();
        let decoded: MetadataRead = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.metadata, result.metadata);
        assert!(inspect(&result.metadata)
            .issues
            .iter()
            .all(|i| !i.message.contains("URI") && !i.message.contains("evidence")));
    }
}

#[test]
fn module_manifest_is_public_and_has_no_io_or_probe_capability() {
    let text = include_str!("../../../contracts/modules/sony-metadata.module.json");
    assert!(qnc_contracts::validate_module_manifest_json("sony-metadata", text).is_ok());
    let manifest: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(manifest["database_write_policy"], "no_db_writes");
    assert_eq!(
        manifest["capabilities"],
        serde_json::json!(["camera.sony.index.read", "camera.sony.metadata.read"])
    );
}

#[test]
fn rejects_raw_os_paths_in_public_bindings_and_document_uris() {
    let camera = index(INDEX).unwrap();
    let mut binding = bound();
    binding.original.media_uri = r"G:\PRIVATE\XDROOT\Clip\TEST A.MXF".into();
    assert!(read_metadata(&camera, 0, &binding, None).is_err());
    assert!(read_index(&XmlDocument {
        document_uri: "/Volumes/card/MEDIAPRO.XML".into(),
        text: INDEX.into()
    })
    .is_err());
}

#[test]
fn unsupported_encoding_and_missing_identity_are_controlled_errors() {
    assert!(index(&INDEX.replace("encoding=\"UTF-8\"", "encoding=\"ISO-8859-1\"")).is_err());
    assert!(index(&INDEX.replace("umid=\"ORIGINAL-A\"", "umid=\"\"")).is_err());
    assert!(read_metadata(&index(INDEX).unwrap(), usize::MAX, &bound(), None).is_err());
}

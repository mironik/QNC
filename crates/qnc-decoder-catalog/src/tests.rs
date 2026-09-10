#![cfg(test)]
use super::*;

fn catalog() -> Catalog {
    serde_json::from_str(include_str!("../../../catalogs/decoders/catalog.json")).unwrap()
}
fn local_catalog() -> (tempfile::TempDir, Catalog) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("decoder"),
        b"test executable marker; never launched",
    )
    .unwrap();
    let mut c = catalog();
    c.adapters[0].executable = Executable::Path {
        path: "decoder".into(),
    };
    c.adapters[0].supported_os = vec![std::env::consts::OS.into()];
    c.adapters[0].supported_cpu = vec![std::env::consts::ARCH.into()];
    (dir, c)
}
#[test]
fn selection_is_explicit_and_unknown_or_duplicate_ids_fail() {
    let mut c = catalog();
    c.validate().unwrap();
    c.selected = "missing".into();
    assert!(c.validate().is_err());
    let mut c = catalog();
    c.adapters.push(c.adapters[0].clone());
    assert!(c.validate().is_err());
    let mut c = catalog();
    c.version = "2".into();
    assert!(c.validate().is_err());
}
#[test]
fn only_installed_compatible_modules_are_available_and_no_fallback() {
    let (dir, mut c) = local_catalog();
    let mut missing = c.adapters[0].clone();
    missing.id = "absent".into();
    missing.executable = Executable::Path {
        path: "absent".into(),
    };
    c.adapters.push(missing);
    assert_eq!(c.available(dir.path()).len(), 1);
    c.selected_config(dir.path()).unwrap();
    c.selected = "absent".into();
    assert!(c.selected_config(dir.path()).is_err());
    c.selected = "qnc.ffmpeg".into();
    c.adapters[0].supported_cpu = vec!["not-this-cpu".into()];
    assert!(c.available(dir.path()).is_empty());
    assert!(c.selected_config(dir.path()).is_err());
}
#[test]
fn new_provider_uses_generic_protocol_without_a_provider_name_switch() {
    let (dir, mut c) = local_catalog();
    c.selected = "customer.decoder".into();
    c.adapters[0].id = c.selected.clone();
    c.adapters[0].driver = Driver::QncPacketsV1 { args: vec![] };
    c.selected_config(dir.path()).unwrap();
}
#[test]
fn malformed_oversized_and_missing_catalogs_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.json");
    assert!(Catalog::read(&path).is_err());
    std::fs::write(&path, b"{broken").unwrap();
    assert!(Catalog::read(&path).is_err());
    std::fs::write(&path, vec![b' '; MAX_CATALOG_BYTES as usize + 1]).unwrap();
    assert!(Catalog::read(&path).is_err());
}
#[test]
fn command_is_not_a_shell_fragment_and_wildcard_capabilities_are_rejected() {
    let (dir, mut c) = local_catalog();
    c.adapters[0].executable = Executable::Command {
        name: "decoder; command".into(),
    };
    assert!(c.selected_config(dir.path()).is_err());
    c.adapters[0].codecs = vec!["*".into()];
    assert!(c.validate().is_err());
}

#[test]
fn saved_format_must_match_selected_capabilities_before_launch() {
    let (dir, c) = local_catalog();
    let config = c.selected_config(dir.path()).unwrap();
    let request: DecodeRequest = serde_json::from_value(serde_json::json!({
        "version": qnc_media_decode::VERSION, "stream_index": 0, "start": null,
        "media": {"media_uri": "qnc://local/source/test/file/clip", "container": null,
            "duration_seconds": null, "streams_complete": null, "tags": {}, "streams": []}
    }))
    .unwrap();
    let mut plan = DecodePlan {
        format: DecodedFormat::Video {
            width: 16,
            height: 16,
            pixel_format: "yuv420p".into(),
        },
        max_packet: 384,
        exact_packet: Some(384),
        container: "matroska".into(),
        codec: "ffv1".into(),
    };
    config.adapter.validate(&request, &plan).unwrap();
    plan.codec = "unregistered_codec".into();
    assert_eq!(
        config.adapter.validate(&request, &plan).unwrap_err().kind,
        ErrorKind::Unsupported
    );
    plan.codec = "ffv1".into();
    plan.container = "unregistered_container".into();
    assert!(config.adapter.validate(&request, &plan).is_err());
    plan.container = "matroska".into();
    plan.format = DecodedFormat::Video {
        width: 16,
        height: 16,
        pixel_format: "unregistered_pixels".into(),
    };
    assert!(config.adapter.validate(&request, &plan).is_err());
}

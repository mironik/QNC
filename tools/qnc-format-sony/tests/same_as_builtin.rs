//! The Sony format as a plug-in program answers exactly what the built-in format answers.

use qnc_camera_adapter::RecordFormat;
use qnc_plugin_process::PluginSpec;
use qnc_record_format_plugin::{PluginFormat, CONTRACT, CONTRACT_VERSION};
use qnc_source_contract::SourceReference;
use qnc_source_groups::IndexDocument;
use std::time::Duration;

const INDEX: &str = include_str!("../../../crates/qnc-sony-metadata/tests/fixtures/MEDIAPRO.XML");
const SIDECAR: &str = include_str!("../../../crates/qnc-sony-metadata/tests/fixtures/TEST-AM01.XML");

fn spec(version: u32) -> PluginSpec {
    PluginSpec { executable: env!("CARGO_BIN_EXE_qnc-format-sony").into(), contract: CONTRACT.into(), version, timeout: Duration::from_secs(10) }
}

fn json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

#[test]
fn the_plugin_answers_what_the_builtin_format_answers() {
    let builtin = qnc_sony_metadata::SonyMediaProfile;
    let plugin = PluginFormat::start(spec(CONTRACT_VERSION)).unwrap();
    assert_eq!(plugin.index().reader_id(), builtin.index().reader_id());
    assert_eq!(plugin.index().namespace(), builtin.index().namespace());
    let root = SourceReference::new("qnc://local/source/card", "XDROOT").unwrap();
    let index = IndexDocument { reference: SourceReference::new("qnc://local/source/card", "XDROOT/MEDIAPRO.XML").unwrap(), text: INDEX.into() };
    let groups = builtin.index().read(&root, &index).unwrap();
    assert!(!groups.is_empty());
    assert_eq!(json(&plugin.index().read(&root, &index).unwrap()), json(&groups));
    for group in &groups {
        assert_eq!(plugin.documents(group), builtin.documents(group));
        assert_eq!(plugin.thumbnail(group), builtin.thumbnail(group));
        let mut documents = vec![index.clone()];
        documents.extend(builtin.documents(group).into_iter().map(|reference| IndexDocument { reference, text: SIDECAR.into() }));
        let expected = builtin.metadata("clip-1", group, &documents).map(|m| json(&m));
        assert_eq!(plugin.metadata("clip-1", group, &documents).map(|m| json(&m)), expected);
    }
}

#[test]
fn a_plugin_of_another_contract_version_is_refused() {
    let refused = PluginFormat::start(spec(CONTRACT_VERSION + 1)).err().unwrap();
    assert!(refused.contains("speaks"), "{refused}");
}

#[test]
fn a_missing_plugin_program_is_a_controlled_error() {
    let mut missing = spec(CONTRACT_VERSION);
    missing.executable = "no-such-qnc-plugin.exe".into();
    assert!(PluginFormat::start(missing).is_err());
}

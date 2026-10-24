//! A camera record format as a plug-in (user 2026-10-23: a new format is a plug-in and one line
//! in the catalog of formats; the application is not rebuilt). The contract `qnc.record-format`
//! 1 asks the four things a record format answers ([`RecordFormat`]): the groups of a card's
//! index, the documents of a clip, its card thumbnail, and its record. A plug-in is given texts
//! and references only, never a path or a file, so it can read nothing else and write nothing.
//!
//! [`PluginFormat`] is that plug-in as a record format of the application; [`serve`] is the side
//! a plug-in program runs.

use qnc_camera_adapter::RecordFormat;
use qnc_media_metadata::ClipMetadata;
use qnc_plugin_process::{Plugin, PluginSpec, Ready};
use qnc_source_contract::SourceReference;
use qnc_source_groups::{GroupProposal, IndexDocument, IndexReader};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

mod catalog;
pub use catalog::{FormatCatalog, FormatEntry, FormatKind, FORMAT_CATALOG};

pub const MODULE_ID: &str = "qnc.module.record-format-plugin";
pub const CONTRACT: &str = "qnc.record-format";
pub const CONTRACT_VERSION: u32 = 1;

/// What a record-format plug-in says about itself when it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct About {
    pub reader_id: String,
    pub namespace: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadIndex {
    root: SourceReference,
    document: IndexDocument,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfGroup {
    group: GroupProposal,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReadMetadata {
    clip_id: String,
    group: GroupProposal,
    documents: Vec<IndexDocument>,
}

/// A record-format plug-in as a record format of the application. Its reader and namespace are
/// asked when it is made, so a plug-in that does not start or speaks another contract is refused
/// before any card is read.
pub struct PluginFormat {
    plugin: Plugin,
    about: About,
}

impl PluginFormat {
    pub fn start(spec: PluginSpec) -> Result<Self, String> {
        let plugin = Plugin::new(spec);
        let about = serde_json::from_value(plugin.ready()?.about).map_err(|e| format!("record-format plug-in: {e}"))?;
        Ok(Self { plugin, about })
    }

    fn ask<T: DeserializeOwned>(&self, op: &str, body: impl Serialize) -> Result<T, String> {
        let answer = self.plugin.call(op, serde_json::to_value(body).map_err(|e| e.to_string())?)?;
        serde_json::from_value(answer).map_err(|e| format!("record-format plug-in answered {op} outside the contract: {e}"))
    }
}

impl IndexReader for PluginFormat {
    fn reader_id(&self) -> &str {
        &self.about.reader_id
    }
    fn namespace(&self) -> &str {
        &self.about.namespace
    }
    fn read(&self, root: &SourceReference, document: &IndexDocument) -> Result<Vec<GroupProposal>, String> {
        self.ask("read_index", ReadIndex { root: root.clone(), document: document.clone() })
    }
}

impl RecordFormat for PluginFormat {
    fn index(&self) -> &dyn IndexReader {
        self
    }
    /// A plug-in that fails here gives no documents: the clip is then read without them.
    fn documents(&self, group: &GroupProposal) -> Vec<SourceReference> {
        self.ask("documents", OfGroup { group: group.clone() }).unwrap_or_default()
    }
    fn thumbnail(&self, group: &GroupProposal) -> Option<SourceReference> {
        self.ask("thumbnail", OfGroup { group: group.clone() }).ok().flatten()
    }
    fn metadata(&self, clip_id: &str, group: &GroupProposal, documents: &[IndexDocument]) -> Result<ClipMetadata, String> {
        self.ask("metadata", ReadMetadata { clip_id: clip_id.into(), group: group.clone(), documents: documents.to_vec() })
    }
}

/// The plug-in program's side: serves `format` under `plugin_id` and `plugin_version` until the
/// application closes its stdin.
pub fn serve(format: &dyn RecordFormat, plugin_id: &str, plugin_version: &str) -> Result<(), String> {
    let index = format.index();
    let about = About { reader_id: index.reader_id().into(), namespace: index.namespace().into() };
    let ready = Ready {
        contract: CONTRACT.into(),
        version: CONTRACT_VERSION,
        plugin_id: plugin_id.into(),
        plugin_version: plugin_version.into(),
        about: serde_json::to_value(about).map_err(|e| e.to_string())?,
    };
    qnc_plugin_process::serve(ready, |op, body| answer(format, op, body))
}

fn answer(format: &dyn RecordFormat, op: &str, body: Value) -> Result<Value, String> {
    match op {
        "read_index" => {
            let asked: ReadIndex = parse(body)?;
            value(&format.index().read(&asked.root, &asked.document)?)
        }
        "documents" => value(&format.documents(&parse::<OfGroup>(body)?.group)),
        "thumbnail" => value(&format.thumbnail(&parse::<OfGroup>(body)?.group)),
        "metadata" => {
            let asked: ReadMetadata = parse(body)?;
            value(&format.metadata(&asked.clip_id, &asked.group, &asked.documents)?)
        }
        other => Err(format!("the record-format contract has no '{other}'")),
    }
}

fn parse<T: DeserializeOwned>(body: Value) -> Result<T, String> {
    serde_json::from_value(body).map_err(|e| format!("a request outside the contract: {e}"))
}

fn value(answer: &impl Serialize) -> Result<Value, String> {
    serde_json::to_value(answer).map_err(|e| e.to_string())
}

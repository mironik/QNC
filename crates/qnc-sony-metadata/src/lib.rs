//! Pure Sony XML adapter. Transport supplies text and resolved identities.

mod groups;
mod mapping;
mod xml;

pub use groups::{metadata_references, read_group_metadata, thumbnail_reference, SonyIndexReader};
pub use mapping::read_metadata;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub use xml::MAX_XML_BYTES;

pub const INDEX_NAMESPACE: &str = "http://xmlns.sony.net/pro/metadata/mediaprofile";
pub const SIDECAR_NAMESPACE: &str = "urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.20";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct XmlDocument {
    pub document_uri: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedMedia {
    pub relative_path: String,
    pub attributes: BTreeMap<String, String>,
    pub locator: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelatedFile {
    pub relative_path: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedMaterial {
    pub original: IndexedMedia,
    pub proxies: Vec<IndexedMedia>,
    pub related: Vec<RelatedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraIndex {
    pub document_uri: String,
    pub properties: BTreeMap<String, String>,
    pub materials: Vec<IndexedMaterial>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundMedia {
    pub relative_path: String,
    pub media_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipBinding {
    pub clip_id: String,
    pub original: BoundMedia,
    pub proxy: Option<BoundMedia>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidecarDocument {
    pub relative_path: String,
    pub document: XmlDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadNotice {
    pub code: String,
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetadataRead {
    pub metadata: qnc_media_metadata::ClipMetadata,
    pub notices: Vec<ReadNotice>,
}

pub fn read_index(document: &XmlDocument) -> Result<CameraIndex, String> {
    validate_uri(&document.document_uri)?;
    let root = xml::parse(&document.text, "MediaProfile", INDEX_NAMESPACE)?;
    let contents = root.one("Contents")?.ok_or("missing Contents")?;
    if contents
        .children
        .iter()
        .any(|n| n.namespace != INDEX_NAMESPACE || n.name != "Material")
    {
        return Err("unsupported Contents entry; entries cannot be silently discarded".into());
    }
    let mut materials = Vec::new();
    let mut media_paths = BTreeSet::new();
    let mut identities = BTreeSet::new();
    for node in contents.children_named("Material") {
        if node.children.iter().any(|n| {
            n.namespace != INDEX_NAMESPACE || !matches!(n.name.as_str(), "Proxy" | "RelevantInfo")
        }) {
            return Err("unsupported Material relationship".into());
        }
        let original = indexed_media(node)?;
        let umid = node.required("umid")?;
        if !identities.insert(umid.to_ascii_uppercase()) {
            return Err("duplicate Material UMID: unresolved/spanned identity".into());
        }
        let proxies = node
            .children_named("Proxy")
            .map(indexed_media)
            .collect::<Result<Vec<_>, _>>()?;
        for media in std::iter::once(&original).chain(&proxies) {
            if !media_paths.insert(media.relative_path.clone()) {
                return Err("duplicate or shared original/proxy reference".into());
            }
        }
        let related = node
            .children_named("RelevantInfo")
            .map(|n| {
                Ok(RelatedFile {
                    relative_path: relative_reference(n.required("uri")?)?,
                    kind: n.required("type")?.into(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        materials.push(IndexedMaterial {
            original,
            proxies,
            related,
        });
    }
    let mut properties = BTreeMap::new();
    if let Some(node) = root.one("Properties")? {
        raw_attributes(node, &mut properties);
    }
    Ok(CameraIndex {
        document_uri: document.document_uri.clone(),
        properties,
        materials,
    })
}

fn indexed_media(node: &xml::Node) -> Result<IndexedMedia, String> {
    Ok(IndexedMedia {
        relative_path: relative_reference(node.required("uri")?)?,
        attributes: node.attributes.clone(),
        locator: node.locator.clone(),
    })
}

pub(crate) fn raw_attributes(node: &xml::Node, output: &mut BTreeMap<String, String>) {
    for (key, value) in &node.attributes {
        output.insert(format!("{}/@{key}", node.locator), value.clone());
    }
    for child in &node.children {
        raw_attributes(child, output);
    }
}

pub(crate) fn validate_uri(uri: &str) -> Result<(), String> {
    let parsed = qnc_contracts::parse_qnc_uri(uri)?;
    if uri != uri.trim()
        || uri.contains(['\\', '?', '#'])
        || uri.chars().any(char::is_control)
        || parsed.resource_kind.contains(':')
        || parsed.resource_id.contains(':')
        || uri.split('/').any(|s| matches!(s, "." | ".."))
    {
        return Err("invalid public QNC URI".into());
    }
    Ok(())
}

/// Camera-local reference only. The transport must bind it inside its recording root.
pub fn relative_reference(value: &str) -> Result<String, String> {
    let value = value.strip_prefix("./").unwrap_or(value);
    let mut segments = Vec::new();
    for encoded in value.split('/') {
        let bytes = encoded.as_bytes();
        for (i, byte) in bytes.iter().enumerate() {
            if *byte == b'%'
                && (i + 2 >= bytes.len()
                    || !bytes[i + 1].is_ascii_hexdigit()
                    || !bytes[i + 2].is_ascii_hexdigit())
            {
                return Err("invalid URI escape".into());
            }
        }
        let segment = percent_encoding::percent_decode_str(encoded)
            .decode_utf8()
            .map_err(|_| "invalid UTF-8 URI")?;
        if matches!(segment.as_ref(), "" | "." | "..")
            || segment.contains(['/', '\\', ':', '?', '#', '%'])
            || segment.chars().any(char::is_control)
            || segment.trim() != segment
        {
            return Err("camera reference must remain inside its recording root".into());
        }
        segments.push(segment.into_owned());
    }
    Ok(segments.join("/"))
}

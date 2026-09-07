use crate::{
    read_index, read_metadata, BoundMedia, ClipBinding, SidecarDocument, XmlDocument,
    INDEX_NAMESPACE,
};
use qnc_source_contract::SourceReference;
use qnc_source_groups::{
    GroupEvidence, GroupProposal, IndexDocument, IndexReader, RelatedReference,
};

pub struct SonyIndexReader;

pub fn thumbnail_reference(group: &GroupProposal) -> Option<SourceReference> {
    let mut references = group.related.iter().filter(|r| r.kind == "JPG");
    let first = references.next()?;
    if references.next().is_some() {
        return None;
    }
    Some(first.reference.clone())
}

pub fn metadata_references(group: &GroupProposal) -> Vec<SourceReference> {
    std::iter::once(group.evidence.document.clone())
        .chain(
            group
                .related
                .iter()
                .filter(|r| r.kind == "XML")
                .map(|r| r.reference.clone()),
        )
        .collect()
}

/// Interpret only documents explicitly linked by the validated recording index.
/// The caller owns transport and persistence; this reader performs no I/O.
pub fn read_group_metadata(
    clip_id: &str,
    group: &GroupProposal,
    documents: &[IndexDocument],
) -> Result<qnc_media_metadata::ClipMetadata, String> {
    let document = documents
        .iter()
        .find(|d| d.reference == group.evidence.document)
        .ok_or("missing camera index evidence")?;
    let proposals = SonyIndexReader.read(&group.root, document)?;
    let i = proposals
        .iter()
        .position(|p| p == group)
        .ok_or("camera index no longer matches persisted recording")?;
    let index = read_index(&XmlDocument {
        document_uri: document.reference.uri(),
        text: document.text.clone(),
    })?;
    let material = &index.materials[i];
    if material.proxies.len() > 1 {
        return Err("multiple proxy representations are not supported".into());
    }
    let binding = ClipBinding {
        clip_id: clip_id.into(),
        original: BoundMedia {
            relative_path: material.original.relative_path.clone(),
            media_uri: group.original.uri(),
        },
        proxy: material
            .proxies
            .first()
            .zip(group.proxies.first())
            .map(|(p, r)| BoundMedia {
                relative_path: p.relative_path.clone(),
                media_uri: r.uri(),
            }),
    };
    let linked: Vec<_> = material
        .related
        .iter()
        .filter(|r| r.kind == "XML")
        .collect();
    if linked.len() > 1 {
        return Err("ambiguous camera sidecar".into());
    }
    let sidecar = linked
        .first()
        .map(|r| -> Result<Option<SidecarDocument>, String> {
            let reference = group
                .root
                .descendant(&r.relative_path)
                .map_err(|e| e.to_string())?;
            Ok(documents
                .iter()
                .find(|d| d.reference == reference)
                .map(|d| SidecarDocument {
                    relative_path: r.relative_path.clone(),
                    document: XmlDocument {
                        document_uri: d.reference.uri(),
                        text: d.text.clone(),
                    },
                }))
        })
        .transpose()?
        .flatten();
    let result = read_metadata(&index, i, &binding, sidecar.as_ref())?;
    if result.notices.iter().any(|n| n.code == "conflict") {
        return Err("conflicting camera metadata".into());
    }
    Ok(result.metadata)
}

impl IndexReader for SonyIndexReader {
    fn reader_id(&self) -> &str {
        "camera.sony.index.read"
    }
    fn namespace(&self) -> &str {
        INDEX_NAMESPACE
    }

    fn read(
        &self,
        root: &SourceReference,
        document: &IndexDocument,
    ) -> Result<Vec<GroupProposal>, String> {
        if !document.reference.is_within(root) || &document.reference == root {
            return Err("index must be inside recording root".into());
        }
        let index = read_index(&XmlDocument {
            document_uri: document.reference.uri(),
            text: document.text.clone(),
        })?;
        index
            .materials
            .into_iter()
            .map(|material| {
                let proposal = GroupProposal {
                    root: root.clone(),
                    recording_identity: material
                        .original
                        .attributes
                        .get("umid")
                        .ok_or("missing recording identity")?
                        .to_ascii_uppercase(),
                    evidence: GroupEvidence {
                        reader_id: self.reader_id().into(),
                        document: document.reference.clone(),
                        locator: material.original.locator,
                    },
                    original: root
                        .descendant(&material.original.relative_path)
                        .map_err(|e| e.to_string())?,
                    proxies: material
                        .proxies
                        .iter()
                        .map(|p| root.descendant(&p.relative_path).map_err(|e| e.to_string()))
                        .collect::<Result<_, _>>()?,
                    related: material
                        .related
                        .into_iter()
                        .map(|r| {
                            Ok(RelatedReference {
                                reference: root
                                    .descendant(&r.relative_path)
                                    .map_err(|e| e.to_string())?,
                                kind: r.kind,
                            })
                        })
                        .collect::<Result<_, String>>()?,
                };
                proposal.validate(root.source_uri())?;
                Ok(proposal)
            })
            .collect()
    }
}

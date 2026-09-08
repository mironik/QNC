use super::*;
use sha2::{Digest, Sha256};

/// Immutable DB evidence identity. A mutable camera index is not an immutable blob.
pub fn camera_document_uri(source_uri: &str, text: &str) -> Result<String> {
    validate_resource_uri(source_uri)?;
    let parsed = parse_qnc_uri(source_uri).map_err(|_| Error::InvalidRequest)?;
    if parsed.resource_kind != "source" {
        return Err(Error::InvalidRequest);
    }
    let prefix = match parsed.authority {
        Some(authority) => format!("qnc://{}/{authority}", parsed.environment),
        None => format!("qnc://{}", parsed.environment),
    };
    let digest = Sha256::digest(text.as_bytes());
    Ok(format!(
        "{prefix}/artifact/camera-document/{digest:x}/{}",
        parsed.resource_id
    ))
}

/// Freeze camera evidence before first publication. Raw bytes and origin path are retained.
pub fn freeze_camera_documents(
    metadata: &mut ClipMetadata,
    documents: &mut [Document],
) -> Result<()> {
    let mut replacements = BTreeMap::new();
    for doc in documents.iter_mut() {
        if doc.media_type != DocumentType::Xml {
            continue;
        }
        let frozen = camera_document_uri(&doc.document_uri, &doc.text)?;
        replacements.insert(doc.document_uri.clone(), frozen.clone());
        doc.document_uri = frozen;
    }
    for evidence in &mut metadata.evidence {
        if evidence.kind == EvidenceKind::CameraMetadata {
            evidence.document_uri = replacements
                .get(&evidence.document_uri)
                .ok_or(Error::InvalidMetadata)?
                .clone();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_document_identity_covers_body_source_and_authority() {
        for prefix in ["qnc://local", "qnc://lan/server", "qnc://intranet/server"] {
            let source = format!("{prefix}/source/card/MEDIAPRO.XML");
            let before = camera_document_uri(&source, "<old/>").unwrap();
            assert_eq!(before, camera_document_uri(&source, "<old/>").unwrap());
            assert_ne!(before, camera_document_uri(&source, "<new/>").unwrap());
            assert_ne!(
                before,
                camera_document_uri(&source.replace("/card/", "/other/"), "<old/>").unwrap()
            );
            assert!(before.starts_with(&format!("{prefix}/artifact/camera-document/")));
            validate_resource_uri(&before).unwrap();
        }
    }
}

//! Reparse archived evidence into a NEW disposable verification DB. Never invokes ffprobe.
use qnc_media_probe::Report;
use qnc_media_record_db::{contract::*, Access, Client};
use qnc_sony_metadata::{
    read_index, read_metadata, BoundMedia, ClipBinding, SidecarDocument, XmlDocument,
};
use qnc_transport_resolver::ResolverConfig;
use std::{collections::BTreeMap, path::PathBuf};

// Read only raw evidence and its bindings, never decode an older derived metadata schema.
#[derive(serde::Deserialize)]
struct ArchivedEvidence {
    request_id: String,
    source_index_uri: String,
    source_record: SourceRecord,
    metadata: ArchivedIdentity,
    documents: Vec<Document>,
}
#[derive(serde::Deserialize)]
struct ArchivedIdentity {
    clip_id: String,
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(std::env::args_os().nth(1).ok_or("archive argument")?);
    let temp = tempfile::tempdir()?;
    let uri = "qnc://local/db/media_records";
    let resolver =
        ResolverConfig::new(temp.path()).with_local_binding(uri, temp.path().join("replay.sqlite"));
    let mut db = Client::create_local(&resolver, uri)?;
    let mut reports = BTreeMap::new();
    let mut writes = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        if name.starts_with("probe-") && name.ends_with(".json") {
            let report: std::result::Result<Report, qnc_media_probe::Error> =
                serde_json::from_slice(&std::fs::read(&path)?)?;
            let r = report?;
            reports.insert(r.media_uri.clone(), r);
        } else if name.starts_with("camera-") && name.ends_with(".json") {
            writes.push(serde_json::from_slice::<ArchivedEvidence>(&std::fs::read(
                &path,
            )?)?);
        }
    }
    let mut count = 0;
    let mut missing = BTreeMap::<String, usize>::new();
    let mut conflicts = Vec::new();
    for write in writes {
        let p = &write.source_record.group.proposal;
        let index_doc = write
            .documents
            .iter()
            .find(|d| d.document_uri == p.evidence.document.uri())
            .ok_or("index proof")?;
        let index = read_index(&XmlDocument {
            document_uri: index_doc.document_uri.clone(),
            text: index_doc.text.clone(),
        })?;
        let i = index
            .materials
            .iter()
            .position(|m| m.original.relative_path == p.original.relative_path())
            .ok_or("indexed material")?;
        let mat = &index.materials[i];
        let xml = mat
            .related
            .iter()
            .find(|r| r.kind == "XML")
            .ok_or("sidecar link")?;
        let side_doc = write
            .documents
            .iter()
            .find(|d| d.document_uri != index_doc.document_uri)
            .ok_or("side proof")?;
        let binding = ClipBinding {
            clip_id: write.metadata.clip_id.clone(),
            original: BoundMedia {
                relative_path: mat.original.relative_path.clone(),
                media_uri: p.original.uri(),
            },
            proxy: p.proxies.first().map(|p| BoundMedia {
                relative_path: mat.proxies[0].relative_path.clone(),
                media_uri: p.uri(),
            }),
        };
        let metadata = read_metadata(
            &index,
            i,
            &binding,
            Some(&SidecarDocument {
                relative_path: xml.relative_path.clone(),
                document: XmlDocument {
                    document_uri: side_doc.document_uri.clone(),
                    text: side_doc.text.clone(),
                },
            }),
        )?
        .metadata;
        let mut write = Write {
            request_id: write.request_id,
            source_index_uri: write.source_index_uri,
            source_record: write.source_record,
            documents: write.documents,
            expected_revision: 0,
            phase: Phase::Camera,
            metadata,
        };
        db.write(write.clone())?;
        let mut parsed = Vec::new();
        for (j, media) in std::iter::once(&write.metadata.original)
            .chain(&write.metadata.proxy)
            .enumerate()
        {
            let r = reports
                .get(&media.media_uri)
                .ok_or("missing saved report")?;
            parsed.push(qnc_ffprobe_metadata::read(
                &r.json,
                &r.media_uri,
                &r.document_uri,
                &format!("ffprobe-{j}"),
            )?);
            write.documents.push(Document {
                document_uri: r.document_uri.clone(),
                media_type: DocumentType::Json,
                text: r.json.clone(),
            });
        }
        let result = qnc_media_metadata_compose::compose(&write.metadata, &parsed);
        match result {
            Ok(metadata) => {
                write.metadata = metadata;
                write.phase = Phase::Final;
                write.expected_revision = 1;
                write.request_id = format!("final-{}", write.request_id);
                db.write(write.clone())?;
                let snapshot = db
                    .read(&write.metadata.clip_id, None)?
                    .ok_or("final snapshot")?;
                for issue in snapshot.report.issues {
                    *missing.entry(issue.path).or_default() += 1;
                }
                count += 1;
            }
            Err(error) => conflicts.push(error.to_string()),
        }
    }
    drop(db);
    let mut db = Client::open(&resolver, uri, Access::ReadOnly, None)?;
    let mut archived = 0;
    for report in reports.values() {
        if let Some(doc) = db.document(&report.document_uri)? {
            assert_eq!(doc.text, report.json);
            archived += 1;
        }
    }
    println!(
        "{}",
        serde_json::json!({"finalized":count,"archived_probe_json":archived,"missing":missing,"conflicts":conflicts,"ffprobe_calls":0})
    );
    Ok(())
}

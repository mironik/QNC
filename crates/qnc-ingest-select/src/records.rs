//! Stage 2: register what the scan found in the source index DB.
//!
//! Turns the scanned groups into source records, in bounded batches. Records the
//! content DB already holds as final are not returned again.

use crate::*;
use qnc_ingest_store::content::InventoryClip;
use qnc_source_groups::{FileFact, GroupProposal};

pub(crate) fn register_groups(
    config: &SelectionConfig,
    source: &SourceReader,
    groups: &[GroupProposal],
    file_facts: &[FileFact],
    existing: &BTreeMap<String, InventoryClip>,
    cancel: &AtomicBool,
) -> Result<Vec<SourceRecord>> {
    let mut source_db = config.source_index.source_db()?;
    // Initialize once before opening the bounded worker connections.
    drop(config.media_records.media_db()?);
    let mut records = Vec::new();
    for chunk in groups.chunks(32) {
        if cancel.load(Ordering::Relaxed) {
            return Err("Select je prekinut.".into());
        }
        let references: BTreeSet<_> = chunk
            .iter()
            .flat_map(|g| g.references())
            .map(|r| r.uri())
            .collect();
        let receipt = source_db.write(qnc_source_index_db::contract::Batch {
            batch_id: uuid::Uuid::new_v4().to_string(),
            source_uri: source.source_uri().into(),
            proposals: chunk.to_vec(),
            file_facts: file_facts
                .iter()
                .filter(|f| references.contains(&f.reference.uri()))
                .cloned()
                .collect(),
        })?;
        for id in receipt.record_ids {
            if !existing
                .get(&format!("clip-{id}"))
                .is_some_and(|c| c.final_record)
            {
                records.push(source_db.read(&id)?.ok_or("source DB receipt missing")?);
            }
        }
    }
    Ok(records)
}

/// The media of all records, each reference once: what the probe backend may open.
pub(crate) fn media_references(records: &[SourceRecord]) -> Vec<SourceReference> {
    let mut media = Vec::new();
    let mut ids = BTreeSet::new();
    for record in records {
        let p = &record.group.proposal;
        for r in std::iter::once(&p.original).chain(&p.proxies) {
            if ids.insert(r.uri()) {
                media.push(r.clone());
            }
        }
    }
    media
}

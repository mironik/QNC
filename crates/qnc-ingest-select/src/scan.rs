//! Stage 1: recognise the source.
//!
//! What the content DB already knows about the source, what the camera catalog and the
//! scanner find on it, and which known clips have disappeared. It reads only: no
//! metadata, no probe and no database write.

use crate::*;
use qnc_ingest_store::content::InventoryClip;
use qnc_source_groups::{FileFact, GroupProposal};

pub(crate) struct Scanned {
    pub existing: BTreeMap<String, InventoryClip>,
    pub groups: Vec<GroupProposal>,
    pub file_facts: Vec<FileFact>,
    /// Known clips whose absence was confirmed against the source.
    pub missing: Vec<InventoryClip>,
}

pub(crate) fn scan_source(
    config: &SelectionConfig,
    selected: &SourceReference,
    source_config: &SourceConfig,
    source: &SourceReader,
    content_target: &ContentTarget,
    registry: &CameraRegistry,
    send: &SyncSender<Event>,
) -> Result<Scanned> {
    let existing = inventory(content_target, source)?;
    send.send(Event::Existing(existing.keys().cloned().collect()))?;
    let catalog = qnc_camera_patterns::read_uri(
        &config.catalog.resolver()?,
        &config.catalog.uri,
        config.catalog.token()?.as_deref(),
    )?;
    let indexes = registry.indexes();
    send.send(Event::Status("Prepoznavanje izvora...".into()))?;
    let scan = qnc_scanner::scan_roles(
        &catalog,
        source,
        source_config.scope,
        &indexes,
        Default::default(),
    )?;
    if !scan.relationships_resolved() {
        send.send(Event::Warning(format!(
            "Nerazrijeseni zapisi: {}; greske skeniranja: {}; blokirane grupe: {}.",
            scan.unresolved_files.len(),
            scan.issues.len(),
            scan.grouping.blocked.len()
        )))?;
    }
    let mut scan_complete = scan.detection.traversal_complete && scan.issues.is_empty();
    let mut file_facts = scan.file_facts;
    let mut groups: Vec<_> = scan
        .grouping
        .groups
        .into_iter()
        .filter(|g| g.proposal.original.is_within(selected) || g.proposal.root.is_within(selected))
        .map(|g| g.proposal)
        .collect();
    // A folder that is no card of a known camera (incoming / FTP, an export): each media
    // file in it is one clip (v5 `scan_media_files`).
    let card = scan
        .detection
        .roots
        .iter()
        .filter(|r| r.has_original_candidates())
        .any(|r| {
            r.root.is_within(selected)
                || r.files
                    .iter()
                    .any(|f| f.role == "original_candidate" && f.reference.is_within(selected))
        });
    if !card {
        let readers = registry.file_readers();
        if !readers.is_empty() {
            let files = qnc_scanner::scan_files(source, selected, &readers, Default::default())?;
            if !files.complete() {
                send.send(Event::Warning(format!(
                    "Pojedinacne datoteke: greske citanja {}, blokirane {}.",
                    files.issues.len(),
                    files.grouping.blocked.len()
                )))?;
            }
            scan_complete = files.issues.is_empty();
            groups = files.grouping.groups.into_iter().map(|g| g.proposal).collect();
            file_facts = files.file_facts;
        }
    }
    let missing = if scan_complete {
        confirm_missing(&existing, &groups, selected, source, send)?
    } else {
        Vec::new()
    };
    Ok(Scanned {
        existing,
        groups,
        file_facts,
        missing,
    })
}

fn inventory(
    content_target: &ContentTarget,
    source: &SourceReader,
) -> Result<BTreeMap<String, InventoryClip>> {
    let mut content = content_target.open(Access::ReadOnly)?;
    let mut existing = BTreeMap::new();
    let mut after = None;
    loop {
        let page = content.inventory(source.source_uri(), after.clone())?;
        if page.is_empty() {
            break;
        }
        let next = page.last().unwrap().clip_id.clone();
        if after.as_ref().is_some_and(|last| last >= &next) {
            return Err("Neispravan DB inventory.".into());
        }
        after = Some(next);
        for clip in page {
            existing.insert(clip.clip_id.clone(), clip);
        }
    }
    Ok(existing)
}

/// Absence must be confirmed against the source, not inferred from camera XML.
fn confirm_missing(
    existing: &BTreeMap<String, InventoryClip>,
    groups: &[GroupProposal],
    selected: &SourceReference,
    source: &SourceReader,
    send: &SyncSender<Event>,
) -> Result<Vec<InventoryClip>> {
    let present: BTreeSet<_> = groups.iter().map(|g| g.original.uri()).collect();
    let mut missing = Vec::new();
    for clip in existing.values() {
        let reference = SourceReference::from_uri(&clip.original_uri)?;
        if reference.is_within(selected) && !present.contains(&clip.original_uri) {
            match source.stat(&reference) {
                Err(qnc_source_reader::ReadError::NotFound) => missing.push(clip.clone()),
                Err(error) => {
                    send.send(Event::Warning(format!(
                        "Nije potvrden nedostatak klipa: {error}"
                    )))?;
                }
                Ok(_) => {}
            }
        }
    }
    Ok(missing)
}

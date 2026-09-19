//! Stage 4: publish to the content DB.
//!
//! The only stage that writes, and it writes only through the public content write
//! transport: clips in bounded batches, and the removal of clips whose absence the scan
//! confirmed.

use crate::*;
use qnc_ingest_store::content::InventoryClip;

/// Writes the published clips until the channel closes or Select is cancelled.
pub(crate) fn publish_batches(
    content_target: ContentTarget,
    publications: Receiver<CatalogClip>,
    send: &SyncSender<Event>,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut writer = ContentWriteTransport::start(content_target)?;
    let mut failed = false;
    let mut sequence = 0_u64;
    while let Ok(first) = publications.recv() {
        // A cancelled Select starts no new database write.
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut batch = vec![first];
        while batch.len() < 16 {
            match publications.try_recv() {
                Ok(clip) => batch.push(clip),
                Err(_) => break,
            }
        }
        let revisions = batch
            .iter()
            .map(|c| (c.id().to_string(), c.snapshot.revision))
            .collect();
        sequence += 1;
        let key = format!("select-batch-{sequence}");
        let error: Option<String> = match writer.publish_batch(key.clone(), batch) {
            Ok(()) => wait_write_completion(&mut writer, &key)
                .and_then(expect_changed)
                .map_err(|err| err.to_string())
                .err(),
            Err(err) => Some(err),
        };
        failed |= error.is_some();
        send.send(Event::Saved { revisions, error })?;
    }
    if failed {
        Err("Neki klipovi nisu spremljeni u bazu.".into())
    } else {
        Ok(())
    }
}

/// Removes the clips the scan confirmed missing and reports how many went.
pub(crate) fn remove_missing(
    content_target: ContentTarget,
    missing: &[InventoryClip],
    send: &SyncSender<Event>,
) -> Result<usize> {
    let mut removed_count = 0;
    let mut writer = ContentWriteTransport::start(content_target)?;
    for chunk in missing.chunks(4096) {
        let key = format!("select-remove-missing-{removed_count}");
        writer.remove_missing(key.clone(), chunk.to_vec())?;
        let removed = match wait_write_completion(&mut writer, &key)?.data {
            ContentWriteData::Removed(ids) => ids,
            ContentWriteData::Changed | ContentWriteData::Claimed(_) => {
                return Err("Neispravan remove-missing odgovor.".into())
            }
        };
        removed_count += removed.len();
        send.send(Event::Removed(removed))?;
    }
    Ok(removed_count)
}

fn wait_write_completion(
    transport: &mut ContentWriteTransport,
    key: &str,
) -> Result<ContentWriteResult> {
    loop {
        for completion in transport.poll() {
            if completion.key == key {
                return completion.result.map_err(Into::into);
            }
        }
        if !transport.has_pending() {
            return Err("Content write transport nije vratio rezultat.".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

fn expect_changed(result: ContentWriteResult) -> Result<()> {
    match result.data {
        ContentWriteData::Changed => Ok(()),
        ContentWriteData::Removed(_) | ContentWriteData::Claimed(_) => {
            Err("Neispravan content write odgovor.".into())
        }
    }
}

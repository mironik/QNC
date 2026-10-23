use crate::*;

/// The queue of the content database seen by the executor.
pub trait ImportQueue {
    fn claim_next(&mut self) -> Result<Option<StoredClip>, String>;
    /// Tells the queue this clip is still being imported (renews its lease).
    fn heartbeat(&mut self, _clip_id: &str) -> Result<(), String> {
        Ok(())
    }
    fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        copy_of: Option<ImportedCopy>,
        optimized: Option<MediaRepresentation>,
        error: Option<String>,
    ) -> Result<(), String>;
}

impl ImportQueue for ContentClient {
    fn heartbeat(&mut self, clip_id: &str) -> Result<(), String> {
        ContentClient::heartbeat(self, clip_id.to_string()).map_err(|e| e.to_string())
    }

    fn claim_next(&mut self) -> Result<Option<StoredClip>, String> {
        ContentClient::claim_next(self).map_err(|e| e.to_string())
    }

    fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        copy_of: Option<ImportedCopy>,
        optimized: Option<MediaRepresentation>,
        error: Option<String>,
    ) -> Result<(), String> {
        ContentClient::finish_import(self, clip_id, media_uri, thumbnail_uri, copy_of, optimized, error)
            .map_err(|e| e.to_string())
    }
}

/// The import queue reached through the serialized content write transport: the
/// only way this module writes to the content database.
pub struct TransportQueue {
    transport: qnc_ingest_store::content::ContentWriteTransport,
    sequence: u64,
}

impl TransportQueue {
    pub fn start(target: qnc_ingest_store::content::ContentTarget) -> Result<Self, String> {
        Self::start_leased(target, None)
    }

    /// A queue whose writes are made under `lease` (external audit 2026-10-17): once the
    /// worker lost it, the database owner refuses its claims and results.
    pub fn start_leased(
        target: qnc_ingest_store::content::ContentTarget,
        lease: Option<qnc_ingest_store::content::WriteLease>,
    ) -> Result<Self, String> {
        Ok(Self {
            transport: qnc_ingest_store::content::ContentWriteTransport::start_leased(target, lease)
                .map_err(|e| e.to_string())?,
            sequence: 0,
        })
    }

    fn next_key(&mut self, what: &str) -> String {
        self.sequence += 1;
        format!("import-{what}-{}", self.sequence)
    }

    fn wait(&mut self, key: &str) -> Result<qnc_ingest_store::content::ContentWriteData, String> {
        loop {
            for completion in self.transport.poll() {
                if completion.key == key {
                    return completion.result.map(|r| r.data).map_err(|e| e.to_string());
                }
            }
            if !self.transport.has_pending() {
                return Err(qnc_ui_text::text("db.content_writer_returned_result").into());
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
    }

    /// Records the poster made in the project for a clip that has none.
    pub fn set_poster(&mut self, clip_id: String, thumbnail_uri: String) -> Result<(), String> {
        let key = self.next_key("poster");
        self.transport
            .set_poster(key.clone(), clip_id, thumbnail_uri)
            .map_err(|e| e.to_string())?;
        self.wait(&key).map(|_| ())
    }

    /// Puts the selected, ready clips into the queue.
    pub fn queue_selected(&mut self, needs: qnc_import_fit::Needs) -> Result<(), String> {
        let key = self.next_key("queue");
        self.transport
            .queue_selected(key.clone(), needs)
            .map_err(|e| e.to_string())?;
        self.wait(&key).map(|_| ())
    }
}

impl ImportQueue for TransportQueue {
    fn heartbeat(&mut self, clip_id: &str) -> Result<(), String> {
        let key = self.next_key("beat");
        self.transport
            .heartbeat(key.clone(), clip_id.to_string())
            .map_err(|e| e.to_string())?;
        self.wait(&key).map(|_| ())
    }

    fn claim_next(&mut self) -> Result<Option<StoredClip>, String> {
        use qnc_ingest_store::content::ContentWriteData;
        let key = self.next_key("claim");
        self.transport
            .claim_next(key.clone())
            .map_err(|e| e.to_string())?;
        match self.wait(&key)? {
            ContentWriteData::Claimed(clip) => Ok(clip.map(|c| *c)),
            _ => Err("Neispravan claim odgovor.".into()),
        }
    }

    fn finish_import(
        &mut self,
        clip_id: String,
        media_uri: Option<String>,
        thumbnail_uri: Option<String>,
        copy_of: Option<ImportedCopy>,
        optimized: Option<MediaRepresentation>,
        error: Option<String>,
    ) -> Result<(), String> {
        let key = self.next_key("finish");
        self.transport
            .finish_import(key.clone(), clip_id, media_uri, thumbnail_uri, copy_of, optimized, error)
            .map_err(|e| e.to_string())?;
        self.wait(&key).map(|_| ())
    }
}

/// Queues the selected clips through the write transport of `target`.
pub fn queue_selected(target: qnc_ingest_store::content::ContentTarget, needs: qnc_import_fit::Needs) -> Result<(), String> {
    TransportQueue::start(target)?.queue_selected(needs)
}

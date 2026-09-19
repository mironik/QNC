//! Writes which clips are selected for import.
//!
//! One write at a time, on its own thread, through the public content write
//! transport. The caller starts it, polls for the outcome and never touches the
//! database. It knows no form and no application, and works over a local, LAN or
//! intranet content target.

use qnc_ingest_store::content::{ContentTarget, ContentWriteTransport};
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::JoinHandle,
    time::Duration,
};

pub const MODULE_ID: &str = "qnc.module.ingest-selection-write";
pub const VERSION: &str = "0.1.0";

/// What a finished write changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub clip_ids: Vec<String>,
    pub selected: bool,
}

#[derive(Debug, Default)]
pub struct SelectionWriter {
    result: Option<Receiver<Result<Applied, String>>>,
    thread: Option<JoinHandle<()>>,
}

impl SelectionWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_busy(&self) -> bool {
        self.result.is_some()
    }

    /// Starts the write. Refused while the previous one is still running.
    pub fn start(
        &mut self,
        target: ContentTarget,
        clip_ids: Vec<String>,
        selected: bool,
    ) -> Result<(), String> {
        if self.is_busy() {
            return Err("DB odabir je u tijeku.".into());
        }
        let (send, receive) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("ingest-selection-write".into())
            .spawn(move || {
                let _ = send.send(write(target, clip_ids, selected));
            })
            .map_err(|error| error.to_string())?;
        self.result = Some(receive);
        self.thread = Some(thread);
        Ok(())
    }

    /// The outcome once the write has finished, `None` while it still runs.
    pub fn poll(&mut self) -> Option<Result<Applied, String>> {
        let receiver = self.result.as_ref()?;
        let outcome = match receiver.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Disconnected) => Err("DB odabir je prekinut.".into()),
            Err(TryRecvError::Empty) => return None,
        };
        self.finish();
        Some(outcome)
    }

    /// Drops the outcome and waits for the write thread.
    pub fn cancel(&mut self) {
        self.finish();
    }

    fn finish(&mut self) {
        self.result = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for SelectionWriter {
    fn drop(&mut self) {
        self.finish();
    }
}

fn write(target: ContentTarget, clip_ids: Vec<String>, selected: bool) -> Result<Applied, String> {
    let mut transport = ContentWriteTransport::start(target)?;
    let key = "select-clips".to_string();
    transport.select(key.clone(), clip_ids.clone(), selected)?;
    loop {
        for completion in transport.poll() {
            if completion.key == key {
                completion.result?;
                return Ok(Applied { clip_ids, selected });
            }
        }
        if !transport.has_pending() {
            return Err("Content write transport nije vratio rezultat.".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_idle_writer_has_no_outcome_and_can_be_cancelled_repeatedly() {
        let mut writer = SelectionWriter::new();
        assert!(!writer.is_busy());
        assert!(writer.poll().is_none());
        writer.cancel();
        writer.cancel();
        assert!(!writer.is_busy());
    }
}

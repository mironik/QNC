//! Writes which clips are selected for import.
//!
//! Selecting clips is only a mark in the form's cache. The database sees the selection
//! once, when the user starts the import: this component then writes the whole set
//! (selected on, the others off) on its own thread through the public content write
//! transport. It knows no form and no application, and works over a local, LAN or
//! intranet content target.

use qnc_ingest_store::content::{ContentTarget, ContentWriteTransport};
use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread::JoinHandle,
    time::Duration,
};

pub const MODULE_ID: &str = "qnc.module.ingest-selection-write";
pub const VERSION: &str = "0.2.0";

/// The selection as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub selected: Vec<String>,
    pub unselected: Vec<String>,
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

    /// Starts writing the whole selection. Refused while a write is still running.
    pub fn start(
        &mut self,
        target: ContentTarget,
        selected: Vec<String>,
        unselected: Vec<String>,
    ) -> Result<(), String> {
        if self.is_busy() {
            return Err("Upis odabira je u tijeku.".into());
        }
        let (send, receive) = mpsc::sync_channel(1);
        let thread = std::thread::Builder::new()
            .name("ingest-selection-write".into())
            .spawn(move || {
                let _ = send.send(write(target, selected, unselected));
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
            Err(TryRecvError::Disconnected) => Err("Upis odabira je prekinut.".into()),
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

fn write(
    target: ContentTarget,
    selected: Vec<String>,
    unselected: Vec<String>,
) -> Result<Applied, String> {
    let mut transport = ContentWriteTransport::start(target)?;
    if !unselected.is_empty() {
        run(&mut transport, "unselect", |t, key| {
            t.select(key, unselected.clone(), false)
        })?;
    }
    if !selected.is_empty() {
        run(&mut transport, "select", |t, key| {
            t.select(key, selected.clone(), true)
        })?;
    }
    Ok(Applied {
        selected,
        unselected,
    })
}

fn run<E: ToString>(
    transport: &mut ContentWriteTransport,
    name: &str,
    send: impl FnOnce(&mut ContentWriteTransport, String) -> Result<(), E>,
) -> Result<(), String> {
    let key = format!("selection-{name}");
    send(transport, key.clone()).map_err(|e| e.to_string())?;
    loop {
        for completion in transport.poll() {
            if completion.key == key {
                completion.result.map_err(|e| e.to_string())?;
                return Ok(());
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

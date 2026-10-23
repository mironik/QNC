//! The read of the catalog off the caller's thread.

use crate::{load, LoadedCatalog};
use qnc_active_project_read::ActiveProjectReader;
use qnc_ingest_store::content::CatalogStats;

/// Loads the catalog off the caller's thread: start it, poll for the outcome.
/// Only one load runs at a time and a cancelled load never delivers.
#[derive(Debug, Default)]
pub struct CatalogLoader {
    result: Option<std::sync::mpsc::Receiver<Result<LoadedCatalog, String>>>,
}

impl CatalogLoader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_busy(&self) -> bool {
        self.result.is_some()
    }

    pub fn start(
        &mut self,
        active_project: ActiveProjectReader,
        retained_workspace: Option<String>,
        retained_stats: Option<CatalogStats>,
    ) -> Result<(), String> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("ingest-catalog-load".into())
            .spawn(move || {
                let _ = send.send(load(
                    &active_project,
                    retained_workspace.as_deref(),
                    retained_stats.as_ref(),
                ));
            })
            .map_err(|_| qnc_ui_text::text("ingest.could_start_reading_work_settings").to_string())?;
        self.result = Some(receive);
        Ok(())
    }

    /// The finished load, `None` while it still runs.
    pub fn poll(&mut self) -> Option<Result<LoadedCatalog, String>> {
        use std::sync::mpsc::TryRecvError;
        let outcome = match self.result.as_ref()?.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(qnc_ui_text::text("ingest.reading_work_settings_interrupted").into()),
        };
        self.result = None;
        Some(outcome)
    }

    pub fn cancel(&mut self) {
        self.result = None;
    }
}

//! How the application starts the background application. The real launcher starts the
//! `qnc-ingest-worker` executable; tests and other compositions can supply their own.

use qnc_ingest_store::content::ContentTarget;
use std::{path::Path, sync::Arc};

type Start = dyn Fn(&Path, &ContentTarget) -> Result<(), String> + Send + Sync;

#[derive(Clone)]
pub(super) struct WorkerLauncher(Arc<Start>);

impl WorkerLauncher {
    pub(super) fn start(&self, root: &Path, target: &ContentTarget) -> Result<(), String> {
        (self.0)(root, target)
    }

    #[cfg(test)]
    pub(super) fn with(
        start: impl Fn(&Path, &ContentTarget) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(start))
    }
}

impl Default for WorkerLauncher {
    fn default() -> Self {
        Self(Arc::new(qnc_ingest_import_worker::launch_worker))
    }
}

impl std::fmt::Debug for WorkerLauncher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WorkerLauncher")
    }
}

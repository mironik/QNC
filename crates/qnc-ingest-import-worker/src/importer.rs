//! The importer: everything a form's application needs to start an import and
//! follow it, in one public component. The application passes what it already
//! holds (settings reader, plan, sources, database target) and shows the notices.

use crate::{queue_selected, ConfigMediaOpener, ImportEvent, ImportSession};
use qnc_ingest_select::selection_config::SourceConfig;
use qnc_ingest_store::content::ContentTarget;
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::SettingsReader;
use std::sync::Arc;

/// One line for the user and whether the import is over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportNotice {
    pub message: String,
    pub finished: bool,
}

#[derive(Debug, Default)]
pub struct Importer {
    session: ImportSession,
}

impl Importer {
    pub fn has_pending_work(&self) -> bool {
        self.session.has_pending_work()
    }

    pub fn cancel(&mut self) {
        self.session.cancel();
    }

    /// Queues the selected clips and starts importing them in the background. The
    /// copy goes to the project folder of this machine; without local access to it
    /// the import is refused.
    pub fn start(
        &mut self,
        reader: &SettingsReader,
        plan: IngestWorkPlan,
        sources: Vec<SourceConfig>,
        target: ContentTarget,
    ) -> Result<(), String> {
        if self.session.has_pending_work() {
            return Err("Uvoz je vec u tijeku.".into());
        }
        let project_dir = reader
            .local_workspace_dir(&plan.settings)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| {
                "Uvoz trazi lokalni pristup direktoriju projekta na ovom stroju.".to_string()
            })?;
        queue_selected(target.clone())?;
        let opener = Arc::new(ConfigMediaOpener::new(sources));
        self.session.start(plan, project_dir, opener, target)
    }

    pub fn poll(&mut self, limit: usize) -> Vec<ImportNotice> {
        self.session
            .poll(limit)
            .into_iter()
            .map(|event| match event {
                ImportEvent::Clip(outcome) => ImportNotice {
                    message: match outcome.result {
                        Ok(_) => format!("Uvezeno: {}", outcome.clip_id),
                        Err(error) => format!("Uvoz nije uspio ({}): {error}", outcome.clip_id),
                    },
                    finished: false,
                },
                ImportEvent::Finished(result) => ImportNotice {
                    message: match result {
                        Ok(summary) => format!(
                            "Uvoz: {} uvezeno; {} neuspjelo.",
                            summary.imported, summary.failed
                        ),
                        Err(error) => error,
                    },
                    finished: true,
                },
            })
            .collect()
    }
}

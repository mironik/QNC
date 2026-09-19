use qnc_ingest_store::content::{
    Access, CatalogStats, ContentTarget, ImportStatus, StoredClipSummary,
};
use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::SettingsReader;

pub const MODULE_ID: &str = "qnc.module.ingest-catalog";
pub const VERSION: &str = "0.1.0";

#[derive(Debug)]
pub struct LoadedCatalog {
    pub plan: IngestWorkPlan,
    pub target: ContentTarget,
    pub stats: CatalogStats,
    pub clips: Option<Vec<CatalogClipRow>>,
    pub source: Option<CatalogSource>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSource {
    pub uri: String,
    pub name: String,
    pub serial_number: String,
    pub volume_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CatalogThumbStatus {
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogClipRow {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub selected: bool,
    pub imported: bool,
    pub previously_seen: bool,
    pub metadata_revision: u32,
    pub thumb_uri: Option<String>,
    pub thumb_status: CatalogThumbStatus,
}

pub fn load(
    reader: &SettingsReader,
    retained_workspace: Option<&str>,
    retained_stats: Option<&CatalogStats>,
) -> Result<LoadedCatalog, String> {
    let plan = reader
        .read()
        .map_err(|e| e.to_string())
        .and_then(IngestWorkPlan::from_settings)?;
    let target = ContentTarget::for_project(reader, &plan.settings)?;
    let mut db = target.open(Access::ReadOnly)?;
    let stats = db.stats()?;
    if retained_workspace == Some(plan.settings.workspace_db_uri.as_str())
        && retained_stats == Some(&stats)
    {
        return Ok(LoadedCatalog {
            plan,
            target,
            stats,
            clips: None,
            source: None,
        });
    }
    let mut clips = Vec::new();
    let mut after = None;
    let mut sources = std::collections::BTreeMap::new();
    loop {
        let page = db.list_summary(after.clone())?;
        if page.is_empty() {
            break;
        }
        let next = page.last().unwrap().clip_id.clone();
        if after.as_ref().is_some_and(|last| last >= &next) {
            return Err("Neispravno stranicenje Ingest baze.".into());
        }
        after = Some(next);
        for stored in page {
            sources.insert(
                stored.source_uri.clone(),
                CatalogSource {
                    uri: stored.source_uri.clone(),
                    name: stored.source_name.clone(),
                    serial_number: stored.serial_number.clone(),
                    volume_name: stored.volume_name.clone(),
                },
            );
            clips.push(row(&stored));
        }
    }
    clips.sort_by(|a, b| a.name.cmp(&b.name));
    let source = if sources.len() == 1 {
        sources.into_values().next()
    } else {
        None
    };
    Ok(LoadedCatalog {
        plan,
        target,
        stats,
        clips: Some(clips),
        source,
    })
}

fn row(stored: &StoredClipSummary) -> CatalogClipRow {
    CatalogClipRow {
        clip_id: stored.clip_id.clone(),
        name: stored.name.clone(),
        duration_seconds: stored.duration_seconds,
        selected: stored.selected,
        imported: stored.import_status == ImportStatus::Imported,
        previously_seen: true,
        metadata_revision: stored.revision,
        thumb_uri: stored.thumbnail_uri.clone(),
        thumb_status: if stored.thumbnail_uri.is_some() {
            CatalogThumbStatus::Pending
        } else {
            CatalogThumbStatus::Missing
        },
    }
}

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
        reader: SettingsReader,
        retained_workspace: Option<String>,
        retained_stats: Option<CatalogStats>,
    ) -> Result<(), String> {
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("ingest-catalog-load".into())
            .spawn(move || {
                let _ = send.send(load(
                    &reader,
                    retained_workspace.as_deref(),
                    retained_stats.as_ref(),
                ));
            })
            .map_err(|_| "Nije moguce pokrenuti citanje radnih postavki.".to_string())?;
        self.result = Some(receive);
        Ok(())
    }

    /// The finished load, `None` while it still runs.
    pub fn poll(&mut self) -> Option<Result<LoadedCatalog, String>> {
        use std::sync::mpsc::TryRecvError;
        let outcome = match self.result.as_ref()?.try_recv() {
            Ok(outcome) => outcome,
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err("Citanje radnih postavki je prekinuto.".into()),
        };
        self.result = None;
        Some(outcome)
    }

    pub fn cancel(&mut self) {
        self.result = None;
    }
}

#[cfg(test)]
mod loader_tests {
    use super::*;

    #[test]
    fn an_idle_loader_delivers_nothing_and_cancels_safely() {
        let mut loader = CatalogLoader::new();
        assert!(!loader.is_busy());
        assert!(loader.poll().is_none());
        loader.cancel();
        assert!(!loader.is_busy());
    }
}

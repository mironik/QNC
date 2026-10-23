//! One read of the catalog of the active project (the work plan, the catalog target, its
//! statistics and, when they changed, its clip rows).

use crate::{CatalogClipRow, CatalogThumbStatus};
use qnc_active_project_read::ActiveProjectReader;
use qnc_ingest_store::content::{Access, CatalogStats, ContentTarget, ImportStatus, StoredClipSummary};
use qnc_ingest_work_plan::IngestWorkPlan;

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

pub fn load(
    active_project: &ActiveProjectReader,
    retained_workspace: Option<&str>,
    retained_stats: Option<&CatalogStats>,
) -> Result<LoadedCatalog, String> {
    let snapshot = active_project.read().map_err(|e| e.to_string())?;
    let plan = IngestWorkPlan::from_settings(snapshot.settings)?;
    let target = ContentTarget::for_project(active_project.settings_reader(), &plan.settings)?;
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
            return Err(qnc_ui_text::text("ingest.invalid_paging_ingest_database").into());
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

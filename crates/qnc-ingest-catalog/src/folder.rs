//! The project folder of this machine, where posters copied by an import live.

use qnc_ingest_work_plan::IngestWorkPlan;
use qnc_work_settings::{SettingsReader, WorkSettings};

/// The project folder of this machine, where posters copied by an import live. `None`
/// when the project is not on this machine.
pub fn project_folder(
    reader: &SettingsReader,
    plan: &IngestWorkPlan,
) -> Option<qnc_media_thumbnail::ProjectFolder> {
    let dir = reader.local_workspace_dir(&plan.settings).ok().flatten()?;
    Some(qnc_media_thumbnail::ProjectFolder {
        root_uri: plan.settings.output_root_uri.clone(),
        dir,
    })
}

pub fn project_folder_for_settings(
    reader: &SettingsReader,
    settings: &WorkSettings,
) -> Option<qnc_media_thumbnail::ProjectFolder> {
    let dir = reader.local_workspace_dir(settings).ok().flatten()?;
    Some(qnc_media_thumbnail::ProjectFolder {
        root_uri: settings.output_root_uri.clone(),
        dir,
    })
}

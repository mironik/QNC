//! The script of a story written out (audit 2026-10-28: the writing lived inside the script's
//! piece). Into the export folder of the project on this computer go the three forms of
//! `qnc-script-export` (`<project>_script.txt`, `.html`, `.mos.xml`), with the project's name as
//! the title and its story sheet as the header (user 2026-10-28: the field crew sends it to
//! the editor and the proofreader). It writes files only, no database.

use qnc_db_broker::ProjectDbTarget;
use qnc_script_export::{MosIds, Story};
use qnc_still_locator::StillLocator;

pub const MODULE_ID: &str = "qnc.module.script-files";

/// Writes `story` out; the file of the A/V script. Reads the story sheet and writes files:
/// call it off the form thread.
pub fn write(locator: &StillLocator, target: &ProjectDbTarget, mut story: Story) -> Result<String, String> {
    let settings = locator.settings();
    let project_dir = locator.reader().local_workspace_dir(settings).map_err(|e| e.to_string())?.ok_or_else(|| qnc_ui_text::text("script.voice_local_only").to_string())?;
    let folder = settings.export_folder(&project_dir).map_err(|e| e.message)?;
    story.title = settings.project_name.clone();
    story.header = qnc_newsroom_sheet_db::read(target).map(|sheet| sheet.shown()).unwrap_or_default();
    let base = format!("{}_script", qnc_work_settings::export_file_name(&settings.project_name));
    let ids = MosIds {
        mos_id: "qnc.mos".into(),
        ncs_id: "qnc.newsroom".into(),
        ro_id: settings.project_id.clone(),
        story_id: format!("{}-script", settings.project_id),
        message_id: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs()),
    };
    let av = folder.join(format!("{base}.html"));
    for (file, text) in [
        (folder.join(format!("{base}.txt")), qnc_script_export::text(&story)),
        (av.clone(), qnc_script_export::av_html(&story)),
        (folder.join(format!("{base}.mos.xml")), qnc_script_export::mos_xml(&story, &ids)),
    ] {
        std::fs::write(&file, text).map_err(|e| format!("{}: {e}", file.display()))?;
    }
    Ok(av.display().to_string())
}

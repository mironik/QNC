//! Removes SubClips (statements) of the active project of a QNC root through the module's own
//! writer (user 2026-11-01: the same statement was saved four times). Only shorts go.
//! Usage: forget_shorts QNC_ROOT SHOT_ID...

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let root = args.next().ok_or("usage: forget_shorts QNC_ROOT SHOT_ID...")?;
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(&root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    println!("project {}", snapshot.settings.project_id);
    for shot_id in args {
        qnc_virtual_shots::forget_short_now(&target, &shot_id)?;
        println!("forgotten {shot_id}");
    }
    Ok(())
}

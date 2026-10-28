//! Reads (only) the shorts and B-roll shots of the active project of a QNC root.
//! Usage: shorts QNC_ROOT

fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).ok_or("usage: shorts QNC_ROOT")?;
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(&root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    println!("project {}", snapshot.settings.project_id);
    for shot in qnc_virtual_shots::list_pool_shots(&target)? {
        println!("{} {} {}..{} b_roll={} still={} {}", shot.shot_id, shot.clip_id, shot.in_frame, shot.out_frame, shot.b_roll, shot.still_status, shot.name);
    }
    Ok(())
}

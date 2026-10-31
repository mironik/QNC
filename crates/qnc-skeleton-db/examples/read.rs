//! Reads (only) the S-Story skeleton of the active project of a QNC root.
//! Usage: read QNC_ROOT

fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).ok_or("usage: read QNC_ROOT")?;
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(&root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    let elements = qnc_skeleton_db::read(&target)?;
    println!("project {}: {} elements", snapshot.settings.project_id, elements.len());
    for element in elements {
        println!("{:?} {} {}..{} {}", element.kind, element.clip_id, element.in_frame, element.out_frame, element.origin);
    }
    Ok(())
}

//! Reads (only) the parts of every statement of the active project's script, with the clip of
//! each part and where the clip's original is. Usage: pieces QNC_ROOT
fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).ok_or("usage: pieces QNC_ROOT")?;
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(&root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    for block in qnc_script_db::read(&target)? {
        for (index, piece) in block.pieces.iter().enumerate() {
            println!("[{}:{index}] clip={} {}..{} gap={} cut={} a1_channel={:?} a1_db={} a1_off={} smooth={}", block.id, piece.clip_id, piece.from_frame, piece.to_frame, piece.gap_before, piece.cut, piece.a1_channel, piece.a1_level_db, piece.a1_off, piece.smooth_frames);
        }
    }
    Ok(())
}

//! Reads (only) the script of the active project of a QNC root. Usage: script QNC_ROOT
fn main() -> Result<(), String> {
    let root = std::env::args().nth(1).ok_or("usage: script QNC_ROOT")?;
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(&root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    for block in qnc_script_db::read(&target)? {
        println!("[{}] {:?} {} {}..{} {} {} {} {:.1}s\n{}\n", block.id, block.kind, block.clip_name, block.from_frame, block.to_frame, block.start_tc, block.end_tc, block.length, block.seconds, block.text);
    }
    if let Ok(sheet) = qnc_newsroom_sheet_db::read(&target) {
        println!("sheet: {}\nvalues: {:?}", sheet.sheet, sheet.values);
    }
    Ok(())
}

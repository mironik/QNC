//! Turns A1 on for every part of statement BLOCK of the active project's script, through the
//! script's own writer (one step of undo per part). Usage: a1_on QNC_ROOT BLOCK
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let (root, block_id) = (args.next().ok_or("usage: a1_on QNC_ROOT BLOCK")?, args.next().ok_or("usage: a1_on QNC_ROOT BLOCK")?.parse::<i64>().map_err(|e| e.to_string())?);
    let reader = qnc_active_project_read::ActiveProjectReader::from_root(std::path::Path::new(&root)).map_err(|e| e.to_string())?;
    let snapshot = reader.read().map_err(|e| e.to_string())?;
    let target = qnc_db_broker::ProjectDbTarget::for_project(reader.settings_reader(), &snapshot.settings)?;
    let block = qnc_script_db::read(&target)?.into_iter().find(|block| block.id == block_id).ok_or("no such block")?;
    for (index, piece) in block.pieces.into_iter().enumerate().filter(|(_, piece)| piece.a1_off) {
        qnc_script_db::set_piece(&target, block_id, index, qnc_script_db::Piece { a1_off: false, ..piece })?;
        println!("part {index}: A1 on");
    }
    Ok(())
}

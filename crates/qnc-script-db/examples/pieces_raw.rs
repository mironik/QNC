//! Read-only: the stored parts of every block of a copy of a project database, and why a
//! block's parts do not read (the serde error the reader keeps quiet).
fn main() -> Result<(), String> {
    let db = std::env::args().nth(1).ok_or("usage: pieces_raw PROJECT_DB_COPY")?;
    let conn = rusqlite::Connection::open_with_flags(&db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
    let mut statement = conn.prepare("SELECT block_id, pieces_json FROM script_blocks ORDER BY position").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())?;
    for row in rows {
        let (id, json) = row.map_err(|e| e.to_string())?;
        let parsed: Result<Vec<qnc_script_db::Piece>, _> = serde_json::from_str(&json);
        match parsed {
            Ok(pieces) => {
                for piece in pieces {
                    println!("[{id}] {} {}..{} cut={} gap={} text={:?}", piece.clip_id, piece.from_frame, piece.to_frame, piece.cut, piece.gap_before, piece.text);
                }
            }
            Err(e) => println!("[{id}] {e}"),
        }
    }
    Ok(())
}

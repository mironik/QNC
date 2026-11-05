//! The words with letters in a project database, clip by clip (only read): `cargo run -p
//! qnc-letter-db --example count -- <project.db>`.

fn main() -> Result<(), String> {
    let file = std::env::args().nth(1).ok_or("project.db")?;
    let conn = rusqlite::Connection::open_with_flags(&file, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(|e| e.to_string())?;
    let exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='word_letters')", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !exists {
        println!("no word_letters table");
        return Ok(());
    }
    let mut statement = conn.prepare("SELECT clip_id, count(*), max(made_at_utc) FROM word_letters GROUP BY clip_id").map_err(|e| e.to_string())?;
    let rows = statement.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?))).map_err(|e| e.to_string())?;
    for row in rows {
        println!("{:?}", row.map_err(|e| e.to_string())?);
    }
    let mut sample = conn.prepare("SELECT text, letters_json FROM word_letters ORDER BY made_at_utc DESC LIMIT 3").map_err(|e| e.to_string())?;
    for row in sample.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))).map_err(|e| e.to_string())? {
        println!("{:?}", row.map_err(|e| e.to_string())?);
    }
    Ok(())
}

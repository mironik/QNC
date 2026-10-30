//! Prints how a database looks: every table, view, index and trigger as SQLite stores its
//! `CREATE` statement, and the rows each table holds. Opened read only; nothing is written.
//! `cargo run -p qnc-content-read --example schema_dump -- <path to .db>`

use rusqlite::{Connection, OpenFlags};

fn main() -> Result<(), String> {
    let path = std::env::args().nth(1).ok_or("give the path of a .db file")?;
    let db = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|error| error.to_string())?;
    let mut statement = db
        .prepare("SELECT type, name, sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY CASE type WHEN 'table' THEN 0 WHEN 'index' THEN 1 WHEN 'view' THEN 2 ELSE 3 END, name")
        .map_err(|error| error.to_string())?;
    let objects: Vec<(String, String, String)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|error| error.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|error| error.to_string())?;
    println!("-- schema of {path}");
    for (kind, name, sql) in objects {
        if kind == "table" {
            let rows: i64 = db.query_row(&format!("SELECT COUNT(*) FROM \"{name}\""), [], |row| row.get(0)).unwrap_or(-1);
            println!("\n-- table {name}: {rows} rows");
        }
        println!("{sql};");
    }
    Ok(())
}

//! Undo and redo of the script (user 2026-11-06: "Backspace undo does not work"; words cut
//! out of a statement must come back). Every change of the script's blocks is one step: the
//! blocks as they were before it are kept in `script_undo`, in the same transaction as the
//! change. Undo puts them back and keeps what it took for Redo; a new change forgets Redo. The
//! voice of a block (its recording) is never undone: it stays as recorded.

use crate::store::err;
use crate::Result;
use rusqlite::{params, Connection, OptionalExtension};

/// The steps kept back.
const KEPT: i64 = 200;
/// The columns of a block that undo leaves as they are (the recording).
const VOICE: [&str; 3] = ["voice_uri", "voice_seconds", "voice_tc"];

pub(crate) const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS script_undo (
    step INTEGER PRIMARY KEY AUTOINCREMENT,
    stack TEXT NOT NULL CHECK (stack IN ('undo', 'redo')),
    rows_json TEXT NOT NULL);";

fn columns(conn: &Connection) -> Result<Vec<String>> {
    let statement = conn.prepare("SELECT * FROM script_blocks LIMIT 0").map_err(err)?;
    Ok(statement.column_names().into_iter().map(str::to_string).collect())
}

/// The blocks as they are, as one JSON array of their rows.
pub(crate) fn snapshot(conn: &Connection) -> Result<String> {
    let pairs: Vec<String> = columns(conn)?.iter().map(|column| format!("'{column}', {column}")).collect();
    conn.query_row(&format!("SELECT COALESCE(json_group_array(json_object({})), '[]') FROM (SELECT * FROM script_blocks ORDER BY position)", pairs.join(", ")), [], |r| r.get(0))
        .map_err(err)
}

/// The blocks back as `rows` holds them, each keeping the recording it has now.
fn restore(conn: &Connection, rows: &str) -> Result<()> {
    let all = columns(conn)?;
    // Only the columns the step holds (audit 2026-11-08 #17): a step kept before the table got a
    // column puts it back with that column's default, it never writes NULL into it.
    let held: Vec<String> = {
        let mut statement = conn.prepare("SELECT key FROM json_each(json_extract(?1, '$[0]'))").map_err(err)?;
        let keys = statement.query_map(params![rows], |r| r.get::<_, String>(0)).map_err(err)?;
        keys.collect::<std::result::Result<Vec<_>, _>>().map_err(err)?
    };
    let inserted: Vec<&String> = all.iter().filter(|column| held.is_empty() || held.contains(column)).collect();
    let names = inserted.iter().map(|column| column.as_str()).collect::<Vec<_>>().join(", ");
    let values: Vec<String> = inserted.iter().map(|column| format!("json_extract(value, '$.{column}')")).collect();
    let voice: Vec<&str> = VOICE.iter().copied().filter(|column| all.iter().any(|c| c == column)).collect();
    // The recording of every block as it is now.
    let kept: Vec<(i64, Vec<rusqlite::types::Value>)> = if voice.is_empty() {
        Vec::new()
    } else {
        let mut statement = conn.prepare(&format!("SELECT block_id, {} FROM script_blocks", voice.join(", "))).map_err(err)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, (1..=voice.len()).map(|at| r.get::<_, rusqlite::types::Value>(at)).collect::<rusqlite::Result<Vec<_>>>()?)))
            .map_err(err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(err)?
    };
    conn.execute("DELETE FROM script_blocks", []).map_err(err)?;
    conn.execute(&format!("INSERT INTO script_blocks ({names}) SELECT {} FROM json_each(?1)", values.join(", ")), params![rows]).map_err(err)?;
    let set: Vec<String> = voice.iter().enumerate().map(|(at, column)| format!("{column} = ?{}", at + 2)).collect();
    for (id, values) in kept {
        let mut bound: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Integer(id)];
        bound.extend(values);
        conn.execute(&format!("UPDATE script_blocks SET {} WHERE block_id = ?1", set.join(", ")), rusqlite::params_from_iter(bound)).map_err(err)?;
    }
    Ok(())
}

/// After a change: the blocks as they were before it are one step back, Redo is forgotten.
pub(crate) fn changed(conn: &Connection, before: &str) -> Result<()> {
    conn.execute("DELETE FROM script_undo WHERE stack = 'redo'", []).map_err(err)?;
    conn.execute("INSERT INTO script_undo (stack, rows_json) VALUES ('undo', ?1)", params![before]).map_err(err)?;
    conn.execute("DELETE FROM script_undo WHERE stack = 'undo' AND step <= (SELECT MAX(step) FROM script_undo) - ?1", params![KEPT]).map_err(err)?;
    Ok(())
}

/// One step back (`undo`) or forward; `false`: there is none.
pub(crate) fn step(conn: &Connection, undo: bool) -> Result<bool> {
    let (from, to) = if undo { ("undo", "redo") } else { ("redo", "undo") };
    let Some((id, rows)) = conn
        .query_row("SELECT step, rows_json FROM script_undo WHERE stack = ?1 ORDER BY step DESC LIMIT 1", params![from], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
        .optional()
        .map_err(err)?
    else {
        return Ok(false);
    };
    let now = snapshot(conn)?;
    conn.execute("DELETE FROM script_undo WHERE step = ?1", params![id]).map_err(err)?;
    conn.execute("INSERT INTO script_undo (stack, rows_json) VALUES (?1, ?2)", params![to, now]).map_err(err)?;
    restore(conn, &rows)?;
    Ok(true)
}

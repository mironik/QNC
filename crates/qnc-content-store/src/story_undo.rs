//! Story undo / redo (user request 2026-09-25: an UNDO button). Every story edit
//! (segments, markers, covers) keeps the story as it was before in the project
//! database; undo puts that story back and keeps the undone one for redo. The
//! history is database truth, like every other story record: any form or process
//! sees the same steps. A new edit clears redo. Selection changes are not steps.

use rusqlite::{params, types::Value, Connection, OptionalExtension};
use serde_json::{json, Map, Value as Json};

use crate::database::err;
use crate::Result;

/// The story tables a step restores, in an order that keeps no stale row behind.
const STORY_TABLES: [&str; 5] = [
    "story_parts",
    "story_markers",
    "story_marker_slots",
    "story_covers",
    "story_state",
];

/// How many steps back are kept.
const DEPTH: i64 = 100;

pub(crate) fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS story_undo (
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            stack TEXT NOT NULL CHECK (stack IN ('undo', 'redo')),
            snapshot_json TEXT NOT NULL,
            created_at TEXT NOT NULL
         );",
    )
    .map_err(err)
}

/// The whole story as it is now.
pub(crate) fn capture(conn: &Connection) -> Result<String> {
    let mut tables = Map::new();
    for table in STORY_TABLES {
        let mut statement = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .map_err(err)?;
        let columns: Vec<String> = statement
            .column_names()
            .into_iter()
            .map(String::from)
            .collect();
        let rows = statement
            .query_map([], |row| {
                (0..columns.len())
                    .map(|index| row.get::<_, Value>(index).map(to_json))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        tables.insert(table.into(), json!({ "columns": columns, "rows": rows }));
    }
    Ok(Json::Object(tables).to_string())
}

/// After an edit: the story before it becomes the last undo step, redo is gone.
/// An edit that changed nothing is no step.
pub(crate) fn record(conn: &Connection, before: &str) -> Result<()> {
    if capture(conn)? == before {
        return Ok(());
    }
    conn.execute("DELETE FROM story_undo WHERE stack = 'redo'", [])
        .map_err(err)?;
    push(conn, "undo", before)?;
    conn.execute(
        "DELETE FROM story_undo WHERE stack = 'undo' AND seq NOT IN
            (SELECT seq FROM story_undo WHERE stack = 'undo' ORDER BY seq DESC LIMIT ?1)",
        [DEPTH],
    )
    .map_err(err)?;
    Ok(())
}

/// Undo (or redo): the last kept story comes back, the current one goes to the
/// other stack. Nothing kept is a controlled error.
pub(crate) fn step(conn: &Connection, undo: bool) -> Result<()> {
    let (from, to) = if undo { ("undo", "redo") } else { ("redo", "undo") };
    let tx = conn.unchecked_transaction().map_err(err)?;
    let kept: Option<(i64, String)> = tx
        .query_row(
            "SELECT seq, snapshot_json FROM story_undo WHERE stack = ?1
             ORDER BY seq DESC LIMIT 1",
            [from],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((seq, snapshot)) = kept else {
        return Err(if undo {
            "Nema izmjene za ponistiti.".into()
        } else {
            "Nema izmjene za ponoviti.".into()
        });
    };
    let current = capture(&tx)?;
    restore(&tx, &snapshot)?;
    tx.execute("DELETE FROM story_undo WHERE seq = ?1", [seq])
        .map_err(err)?;
    push(&tx, to, &current)?;
    tx.commit().map_err(err)
}

/// How many undo and redo steps are kept.
pub(crate) fn depth(conn: &Connection) -> Result<(u64, u64)> {
    let count = |stack: &str| -> Result<u64> {
        conn.query_row(
            "SELECT COUNT(*) FROM story_undo WHERE stack = ?1",
            [stack],
            |row| row.get::<_, i64>(0),
        )
        .map(|n| n as u64)
        .map_err(err)
    };
    Ok((count("undo")?, count("redo")?))
}

fn push(conn: &Connection, stack: &str, snapshot: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO story_undo (stack, snapshot_json, created_at) VALUES (?1, ?2, ?3)",
        params![stack, snapshot, crate::database::story_now()],
    )
    .map_err(err)?;
    Ok(())
}

fn restore(conn: &Connection, snapshot: &str) -> Result<()> {
    let tables: Map<String, Json> = serde_json::from_str(snapshot).map_err(err)?;
    for table in STORY_TABLES {
        conn.execute(&format!("DELETE FROM {table}"), [])
            .map_err(err)?;
        let Some(stored) = tables.get(table) else {
            continue;
        };
        let columns: Vec<String> = serde_json::from_value(stored["columns"].clone()).map_err(err)?;
        let rows: Vec<Vec<Json>> = serde_json::from_value(stored["rows"].clone()).map_err(err)?;
        if columns.is_empty() {
            continue;
        }
        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({})",
            columns.join(", "),
            vec!["?"; columns.len()].join(", ")
        );
        let mut statement = conn.prepare(&sql).map_err(err)?;
        for row in rows {
            let values: Vec<Value> = row.into_iter().map(from_json).collect();
            statement
                .execute(rusqlite::params_from_iter(values))
                .map_err(err)?;
        }
    }
    Ok(())
}

fn to_json(value: Value) -> Json {
    match value {
        Value::Null => Json::Null,
        Value::Integer(n) => json!({ "i": n }),
        Value::Real(n) => json!({ "r": n }),
        Value::Text(text) => json!({ "t": text }),
        Value::Blob(bytes) => json!({ "b": bytes }),
    }
}

fn from_json(value: Json) -> Value {
    let Json::Object(map) = value else {
        return Value::Null;
    };
    if let Some(n) = map.get("i").and_then(Json::as_i64) {
        Value::Integer(n)
    } else if let Some(n) = map.get("r").and_then(Json::as_f64) {
        Value::Real(n)
    } else if let Some(text) = map.get("t").and_then(Json::as_str) {
        Value::Text(text.into())
    } else if let Some(bytes) = map.get("b") {
        Value::Blob(serde_json::from_value(bytes.clone()).unwrap_or_default())
    } else {
        Value::Null
    }
}

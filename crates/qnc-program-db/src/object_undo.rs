//! Undo of the selected (user 2026-10-29: "two kinds of undo: undo in time, back along the order
//! of the edits, and undo selected: what is selected is undone or redone, whenever it was").
//! v5 undid a selected cover only, one level (`story/object_history.rs`); here every segment, M
//! marker and cover. The steps of the story's undo (`undo.rs`) are the history: undo selected
//! finds the newest step that changed the object and puts back only that object's row as it was
//! before it; redo selected puts back what that undo took. An undo selected is a story edit
//! itself, so the undo in time takes it back too. Per object, in `story_object_history`: the
//! step it went back to and the rows it took (its redo), cleared when another edit changes it.

use rusqlite::{params, types::Value, Connection, OptionalExtension};
use serde_json::{json, Map, Value as Json};

use crate::store::{err, story_now};
use crate::Result;

/// An object that can be undone alone: its table and its key column.
fn table_of(object_type: &str) -> Result<(&'static str, &'static str)> {
    match object_type.trim() {
        "segment" => Ok(("story_parts", "part_id")),
        "marker" => Ok(("story_markers", "marker_id")),
        "cover" => Ok(("story_covers", "cover_id")),
        other => Err(format!("undo selected: unknown object type {other}")),
    }
}

/// The columns that say nothing about the object itself.
const STAMPS: [&str; 2] = ["created_at", "updated_at"];

/// An object's row in a captured story (`undo::capture`): its columns and values, `None` when
/// the object was not there.
fn row_in(snapshot: &Json, table: &str, key: &str, id: &str) -> Option<(Vec<String>, Vec<Json>)> {
    let stored = snapshot.get(table)?;
    let columns: Vec<String> = serde_json::from_value(stored["columns"].clone()).ok()?;
    let at = columns.iter().position(|column| column == key)?;
    let rows: Vec<Vec<Json>> = serde_json::from_value(stored["rows"].clone()).ok()?;
    let row = rows.into_iter().find(|row| row.get(at).and_then(|value| value.get("t")).and_then(Json::as_str) == Some(id))?;
    Some((columns, row))
}

/// The row without its stamps, to tell whether the object changed.
fn content(row: &Option<(Vec<String>, Vec<Json>)>) -> Option<Vec<Json>> {
    row.as_ref().map(|(columns, values)| {
        columns.iter().zip(values).filter(|(column, _)| !STAMPS.contains(&column.as_str())).map(|(_, value)| value.clone()).collect()
    })
}

/// The object's history: the undo step it went back to (`None` none yet) and what each undo
/// took, newest last.
#[derive(Default)]
struct History {
    from_seq: Option<i64>,
    taken: Vec<(Option<i64>, Json)>,
}

fn history(conn: &Connection, object_type: &str, id: &str) -> Result<History> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT snapshot_json FROM story_object_history WHERE object_type = ?1 AND object_id = ?2",
            params![object_type, id],
            |row| row.get(0),
        )
        .optional()
        .map_err(err)?;
    let Some(raw) = raw else {
        return Ok(History::default());
    };
    let json: Json = serde_json::from_str(&raw).map_err(err)?;
    let taken = json["taken"].as_array().cloned().unwrap_or_default();
    Ok(History {
        from_seq: json["from_seq"].as_i64(),
        taken: taken.into_iter().map(|entry| (entry["from_seq"].as_i64(), entry["row"].clone())).collect(),
    })
}

fn keep_history(conn: &Connection, object_type: &str, id: &str, history: &History) -> Result<()> {
    let taken: Vec<Json> = history.taken.iter().map(|(seq, row)| json!({ "from_seq": seq, "row": row })).collect();
    let snapshot = json!({ "from_seq": history.from_seq, "taken": taken }).to_string();
    conn.execute(
        "INSERT INTO story_object_history (object_type, object_id, state, snapshot_json, updated_at)
         VALUES (?1, ?2, 'undone', ?3, ?4)
         ON CONFLICT(object_type, object_id) DO UPDATE SET
            state = excluded.state, snapshot_json = excluded.snapshot_json, updated_at = excluded.updated_at",
        params![object_type, id, snapshot, story_now()],
    )
    .map_err(err)?;
    Ok(())
}

/// A row as kept in the history: `null` for an object that was not there.
fn row_json(row: &Option<(Vec<String>, Vec<Json>)>) -> Json {
    match row {
        Some((columns, values)) => json!({ "columns": columns, "values": values }),
        None => Json::Null,
    }
}

fn row_from_json(json: &Json) -> Option<(Vec<String>, Vec<Json>)> {
    if json.is_null() {
        return None;
    }
    Some((serde_json::from_value(json["columns"].clone()).ok()?, serde_json::from_value(json["values"].clone()).ok()?))
}

/// Puts the object's row back as `row` (`None`: the object goes).
fn put_row(conn: &Connection, table: &str, key: &str, id: &str, row: &Option<(Vec<String>, Vec<Json>)>) -> Result<()> {
    conn.execute(&format!("DELETE FROM {table} WHERE {key} = ?1"), [id]).map_err(err)?;
    if let Some((columns, values)) = row {
        let sql = format!("INSERT INTO {table} ({}) VALUES ({})", columns.join(", "), vec!["?"; columns.len()].join(", "));
        let values: Vec<Value> = values.iter().cloned().map(crate::undo::value_from_json).collect();
        conn.execute(&sql, rusqlite::params_from_iter(values)).map_err(err)?;
    }
    Ok(())
}

/// Undo selected: the newest change of the object before the step it last went back to is
/// undone, the object alone. Nothing to undo is a controlled error.
pub(crate) fn undo(conn: &Connection, object_type: &str, id: &str) -> Result<()> {
    let (table, key) = table_of(object_type)?;
    let id = id.trim();
    let mut history = history(conn, object_type, id)?;
    let current: Json = serde_json::from_str(&crate::undo::capture(conn)?).map_err(err)?;
    let now = row_in(&current, table, key, id);
    let mut steps = conn
        .prepare("SELECT seq, snapshot_json FROM story_undo WHERE stack = 'undo' ORDER BY seq DESC")
        .map_err(err)?;
    let steps = steps
        .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    // Each step is the story before an edit; the story after it is the step after (or now).
    let mut after = content(&now);
    let mut found = None;
    for (seq, snapshot) in steps {
        let before_json: Json = serde_json::from_str(&snapshot).map_err(err)?;
        let before = row_in(&before_json, table, key, id);
        let changed = content(&before) != after;
        after = content(&before);
        if history.from_seq.is_some_and(|from| seq >= from) {
            continue;
        }
        if changed {
            found = Some((seq, before));
            break;
        }
    }
    let Some((seq, before)) = found else {
        return Err(qnc_ui_text::text("story.nothing_undo_selected").into());
    };
    put_row(conn, table, key, id, &before)?;
    history.taken.push((history.from_seq, row_json(&now)));
    history.from_seq = Some(seq);
    keep_history(conn, object_type, id, &history)
}

/// Redo selected: what the last undo selected of the object took comes back.
pub(crate) fn redo(conn: &Connection, object_type: &str, id: &str) -> Result<()> {
    let (table, key) = table_of(object_type)?;
    let id = id.trim();
    let mut history = history(conn, object_type, id)?;
    let Some((from_seq, row)) = history.taken.pop() else {
        return Err(qnc_ui_text::text("story.nothing_redo_selected").into());
    };
    put_row(conn, table, key, id, &row_from_json(&row))?;
    history.from_seq = from_seq;
    keep_history(conn, object_type, id, &history)
}

/// After any other edit: the objects it changed lose their own history (a new change starts
/// it again), the others keep theirs.
pub(crate) fn forget_changed(conn: &Connection, before: &str) -> Result<()> {
    let before: Map<String, Json> = serde_json::from_str(before).map_err(err)?;
    let after: Map<String, Json> = serde_json::from_str(&crate::undo::capture(conn)?).map_err(err)?;
    let mut kept = conn.prepare("SELECT object_type, object_id FROM story_object_history").map_err(err)?;
    let kept = kept
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let (before, after) = (Json::Object(before), Json::Object(after));
    for (object_type, id) in kept {
        let Ok((table, key)) = table_of(&object_type) else { continue };
        if content(&row_in(&before, table, key, &id)) != content(&row_in(&after, table, key, &id)) {
            conn.execute("DELETE FROM story_object_history WHERE object_type = ?1 AND object_id = ?2", params![object_type, id])
                .map_err(err)?;
        }
    }
    Ok(())
}

/// After an undo or redo in time: every object's own history goes (its steps moved).
pub(crate) fn forget_all(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM story_object_history", []).map_err(err)?;
    Ok(())
}

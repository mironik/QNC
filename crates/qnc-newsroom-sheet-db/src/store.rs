//! The story sheet tables of a project database and their requests.

use crate::*;
use rusqlite::{params, Connection};

/// The tables this module owns; it writes no other.
const OWNED: [&str; 2] = ["project_story_sheet", "project_story_values"];

pub(crate) struct Store {
    conn: Connection,
    ready: bool,
}

impl Store {
    /// Creates the tables if missing (read-write) and serves them; a read-only connection of
    /// a project without them reads an empty sheet.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let ready: bool = conn
            .query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='view' AND name='public_project_story_values')", [], |r| r.get(0))
            .map_err(err)?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::Read if !self.ready => Ok(Data::Sheet(StorySheet::default())),
            Operation::Read => self.read(),
            Operation::Save(sheet) => self.save(sheet),
        }
    }

    fn save(&mut self, sheet: &StorySheet) -> Result<Data> {
        if !sheet.sheet.is_object() {
            return Err(qnc_ui_text::text("db.story_sheet_invalid").into());
        }
        if let Some(code) = sheet.values.keys().find(|code| !valid_code(code)) {
            return Err(qnc_ui_text::fill("db.story_sheet_code", &[("code", code)]));
        }
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        tx.execute(
            "INSERT INTO project_story_sheet (id, sheet_json, saved_at_utc) VALUES (1, ?1, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
             ON CONFLICT(id) DO UPDATE SET sheet_json = excluded.sheet_json, saved_at_utc = excluded.saved_at_utc",
            params![sheet.sheet.to_string()],
        )
        .map_err(err)?;
        tx.execute("DELETE FROM project_story_values", []).map_err(err)?;
        for (code, value) in &sheet.values {
            tx.execute("INSERT INTO project_story_values (code, value_json) VALUES (?1, ?2)", params![code, value.to_string()]).map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(Data::Changed)
    }

    fn read(&self) -> Result<Data> {
        let sheet = self
            .conn
            .query_row("SELECT sheet_json FROM public_project_story_sheet", [], |r| r.get::<_, String>(0))
            .map(|json| serde_json::from_str(&json).unwrap_or(Value::Null))
            .or_else(|error| if error == rusqlite::Error::QueryReturnedNoRows { Ok(Value::Null) } else { Err(err(error)) })?;
        let mut statement = self.conn.prepare("SELECT code, value_json FROM public_project_story_values ORDER BY code").map_err(err)?;
        let rows = statement
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(err)?;
        let values = rows.into_iter().map(|(code, json)| (code, serde_json::from_str(&json).unwrap_or(Value::Null))).collect();
        Ok(Data::Sheet(StorySheet { sheet, values }))
    }

    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

/// A field code: lower-case letters, digits and `_`, starting with a letter.
pub(crate) fn valid_code(code: &str) -> bool {
    code.chars().next().is_some_and(|first| first.is_ascii_lowercase())
        && code.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_story_sheet (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            sheet_json TEXT NOT NULL CHECK (json_valid(sheet_json)),
            saved_at_utc TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS project_story_values (
            code TEXT PRIMARY KEY,
            value_json TEXT NOT NULL CHECK (json_valid(value_json))
        );
        CREATE VIEW IF NOT EXISTS public_project_story_sheet AS
            SELECT sheet_json, saved_at_utc FROM project_story_sheet;
        CREATE VIEW IF NOT EXISTS public_project_story_values AS
            SELECT code, value_json FROM project_story_values;",
    )
    .map_err(err)
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

impl qnc_db_broker::OwnedTables for Store {
    const ID: &'static str = MODULE_ID;
    type Operation = Operation;
    type Data = Data;
    fn is_write(operation: &Operation) -> bool {
        matches!(operation, Operation::Save(_))
    }
    fn attach(connection: Connection, access: Access) -> Result<Self> {
        Store::attach(connection, access)
    }
    fn execute(&mut self, operation: &Operation) -> Result<Data> {
        Store::execute(self, operation)
    }
    fn connection(&self) -> &Connection {
        Store::connection(self)
    }
}

//! The versions table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["skeleton_versions"];

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection of a
    /// project without it reads no versions.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        let schema_ready = object_exists(&conn, "view", "public_skeleton_versions")?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        qnc_db_broker::write_only_own_tables(&conn, access, &OWNED);
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::ReadVersions if !self.schema_ready => Ok(Data::Versions(Vec::new())),
            Operation::ReadVersions => self.versions(),
            Operation::NewVersion => self.create(),
        }
    }

    fn create(&mut self) -> Result<Data> {
        let tx = qnc_db_broker::begin_write(&mut self.conn).map_err(err)?;
        // Numbered after the highest number a version ever had: a number is not given again.
        let number: u32 = tx.query_row("SELECT COALESCE(MAX(number), 0) + 1 FROM skeleton_versions", [], |row| row.get(0)).map_err(err)?;
        tx.execute(
            "INSERT INTO skeleton_versions (number, created_at_utc) VALUES (?1, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![number],
        )
        .map_err(err)?;
        let version_id = tx.last_insert_rowid();
        tx.commit().map_err(err)?;
        Ok(Data::Created(Version { version_id, number }))
    }

    fn versions(&mut self) -> Result<Data> {
        let mut statement = self.conn.prepare("SELECT version_id, number FROM public_skeleton_versions ORDER BY number").map_err(err)?;
        let versions = statement
            .query_map([], |row| Ok(Version { version_id: row.get(0)?, number: row.get(1)? }))
            .map_err(err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(err)?;
        Ok(Data::Versions(versions))
    }

    /// The connection the intermediary opens a leased write's transaction on.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    // The elements of the first day (kept here, not in a story of their own) are a development
    // record: removed, not converted (AGENTS 3); so are versions that had them.
    if object_exists(conn, "table", "skeleton_elements")? {
        conn.execute_batch(
            "DROP VIEW IF EXISTS public_skeleton_elements; DROP TABLE skeleton_elements;
             DROP VIEW IF EXISTS public_skeleton_versions; DROP TABLE IF EXISTS skeleton_versions;",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "table", "skeleton_versions")? {
        conn.execute_batch(
            "CREATE TABLE skeleton_versions (
                version_id INTEGER PRIMARY KEY AUTOINCREMENT,
                number INTEGER NOT NULL UNIQUE CHECK (number > 0),
                created_at_utc TEXT NOT NULL
            );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_skeleton_versions")? {
        conn.execute_batch("CREATE VIEW public_skeleton_versions AS SELECT version_id, number, created_at_utc FROM skeleton_versions;")
            .map_err(err)?;
    }
    Ok(())
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)", params![kind, name], |r| r.get(0))
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
        operation.is_write()
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

//! What every table module repeated (user 2026-10-26, step 1 of the architecture cleanup,
//! before AI tables come): joining the intermediary with typed requests, and the authorizer
//! that lets a module write only its own tables.

use std::marker::PhantomData;
use std::sync::Arc;

use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::{Access, Result, TableModule, TableModuleFactory};

/// A table module: its id, its typed requests and answers, how it joins a project database
/// (creating its tables when it may write) and how it serves a request.
pub trait OwnedTables: Send + Sized + 'static {
    const ID: &'static str;
    type Operation: DeserializeOwned;
    type Data: Serialize;
    fn is_write(operation: &Self::Operation) -> bool;
    fn attach(connection: Connection, access: Access) -> Result<Self>;
    fn execute(&mut self, operation: &Self::Operation) -> Result<Self::Data>;
    fn connection(&self) -> &Connection;
}

/// How the module `M` joins the intermediary.
pub fn owned_tables<M: OwnedTables>() -> Arc<dyn TableModuleFactory> {
    Arc::new(Factory::<M>(PhantomData))
}

struct Factory<M>(PhantomData<fn() -> M>);

impl<M: OwnedTables> TableModuleFactory for Factory<M> {
    fn id(&self) -> &'static str {
        M::ID
    }
    fn is_write(&self, payload: &Value) -> bool {
        serde_json::from_value::<M::Operation>(payload.clone()).is_ok_and(|operation| M::is_write(&operation))
    }
    fn attach(&self, connection: Connection, access: Access) -> Result<Box<dyn TableModule>> {
        Ok(Box::new(Attached(M::attach(connection, access)?)))
    }
}

struct Attached<M>(M);

impl<M: OwnedTables> TableModule for Attached<M> {
    fn execute(&mut self, payload: Value) -> Result<Value> {
        let operation: M::Operation = serde_json::from_value(payload).map_err(|e| e.to_string())?;
        let data = self.0.execute(&operation)?;
        serde_json::to_value(data).map_err(|e| e.to_string())
    }
    fn connection(&self) -> &Connection {
        self.0.connection()
    }
}

/// The connection reads anything and, when it may write, writes only the tables in `owned`.
/// Set it after the module has created its tables.
pub fn write_only_own_tables(conn: &Connection, access: Access, owned: &'static [&'static str]) {
    conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
        use rusqlite::hooks::{AuthAction as A, Authorization as R};
        match ctx.action {
            A::Insert { table_name } | A::Delete { table_name } | A::Update { table_name, .. }
                if access == Access::ReadWrite && owned.contains(&table_name) =>
            {
                R::Allow
            }
            A::Read { .. }
            | A::Select
            | A::Transaction { .. }
            | A::Savepoint { .. }
            | A::Function { .. }
            | A::Recursive => R::Allow,
            _ => R::Deny,
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_module_writes_only_its_own_tables_and_reads_any() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE mine (v INTEGER); CREATE TABLE theirs (v INTEGER); INSERT INTO theirs VALUES (1);")
            .unwrap();
        write_only_own_tables(&conn, Access::ReadWrite, &["mine"]);
        conn.execute("INSERT INTO mine VALUES (2)", []).unwrap();
        assert!(conn.execute("INSERT INTO theirs VALUES (3)", []).is_err());
        assert!(conn.execute("DELETE FROM theirs", []).is_err());
        let read: i64 = conn.query_row("SELECT v FROM theirs", [], |row| row.get(0)).unwrap();
        assert_eq!(read, 1);
    }

    #[test]
    fn a_read_only_module_writes_not_even_its_own() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE mine (v INTEGER);").unwrap();
        write_only_own_tables(&conn, Access::ReadOnly, &["mine"]);
        assert!(conn.execute("INSERT INTO mine VALUES (1)", []).is_err());
    }
}

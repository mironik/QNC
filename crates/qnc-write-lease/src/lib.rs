//! The lease a write is made under (external audit 2026-10-17: a worker whose lease
//! another took kept writing between its checks).
//!
//! A worker that holds a lease of a project (`qnc-playback-activity::Lease`) sends it
//! with its writes; the owner of the database checks it in the same transaction as
//! the write, so a write of a worker that lost its lease is refused, never half done.
//! This block only reads the lease entry; it never writes one.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// Who holds lease `key` (`owner`) and how long it stays fresh without renewal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WriteLease {
    pub key: String,
    pub owner: String,
    pub max_age_seconds: i64,
}

/// The value of a lease entry held by `owner`.
pub fn held_by(owner: &str) -> String {
    format!("on:{owner}")
}

impl WriteLease {
    /// Still this owner's and renewed in time, by the clock of the database. Run in the
    /// transaction of the write it guards.
    pub fn holds(&self, conn: &Connection) -> Result<bool, String> {
        conn.query_row(
            "SELECT count(*) FROM ingest_runtime WHERE key=?1 AND value=?2
               AND CAST(strftime('%s','now') AS INTEGER) - updated_at <= ?3",
            params![self.key, held_by(&self.owner), self.max_age_seconds.max(1)],
            |r| r.get::<_, i64>(0),
        )
        .map(|count| count == 1)
        .map_err(|e| e.to_string())
    }

    /// `Err` (the lease is lost) unless [`Self::holds`].
    pub fn check(&self, conn: &Connection) -> Result<(), String> {
        if self.holds(conn)? {
            Ok(())
        } else {
            Err(qnc_ui_text::fill("db.write_lease_lost", &[("lease", &self.key)]))
        }
    }
}

/// The write transaction of a database owner: its own immediate transaction, or a savepoint
/// inside the transaction the intermediary opened for a write made under a lease.
pub enum WriteTx<'a> {
    Own(rusqlite::Transaction<'a>),
    Nested(rusqlite::Savepoint<'a>),
}

/// Begins a write (in place of an immediate transaction).
pub fn begin_write(conn: &mut rusqlite::Connection) -> rusqlite::Result<WriteTx<'_>> {
    if conn.is_autocommit() {
        conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map(WriteTx::Own)
    } else {
        conn.savepoint().map(WriteTx::Nested)
    }
}

impl WriteTx<'_> {
    pub fn commit(self) -> rusqlite::Result<()> {
        match self {
            WriteTx::Own(tx) => tx.commit(),
            WriteTx::Nested(sp) => sp.commit(),
        }
    }
}

impl std::ops::Deref for WriteTx<'_> {
    type Target = rusqlite::Connection;
    fn deref(&self) -> &rusqlite::Connection {
        match self {
            WriteTx::Own(tx) => tx,
            WriteTx::Nested(sp) => sp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db(value: &str, age: i64) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE ingest_runtime(key TEXT PRIMARY KEY, value TEXT NOT NULL, updated_at INTEGER NOT NULL);")
            .unwrap();
        conn.execute(
            "INSERT INTO ingest_runtime VALUES('worker', ?1, CAST(strftime('%s','now') AS INTEGER) - ?2)",
            params![value, age],
        )
        .unwrap();
        conn
    }

    fn lease() -> WriteLease {
        WriteLease { key: "worker".into(), owner: "a".into(), max_age_seconds: 5 }
    }

    #[test]
    fn a_lease_holds_only_for_its_owner_while_it_is_fresh() {
        assert!(lease().holds(&db("on:a", 1)).unwrap());
        assert!(!lease().holds(&db("on:b", 1)).unwrap(), "another owner took it");
        assert!(!lease().holds(&db("off", 1)).unwrap(), "let go");
        assert!(!lease().holds(&db("on:a", 30)).unwrap(), "not renewed in time");
        assert!(lease().check(&db("on:b", 1)).is_err());
    }
}

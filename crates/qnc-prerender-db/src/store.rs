//! The prerender table of a project database and its requests.

use crate::*;
use rusqlite::{params, Connection};

/// The table this module owns; it writes no other.
const OWNED: [&str; 1] = ["prerender_units"];
/// Fingerprints read in one request at most (a story has some tens of units).
const MAX_READ: usize = 4096;

pub(crate) struct Store {
    conn: Connection,
    schema_ready: bool,
}

impl Store {
    /// Creates the table if missing (read-write) and serves it; a read-only connection
    /// of a project without it reads no unit.
    pub(crate) fn attach(mut conn: Connection, access: Access) -> Result<Self> {
        if access == Access::ReadWrite {
            let tx = conn
                .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
                .map_err(err)?;
            ensure_schema(&tx)?;
            tx.commit().map_err(err)?;
        }
        // A table of an earlier development form reads as empty until a write replaces it.
        let schema_ready = current_form(&conn)?;
        if access == Access::ReadOnly {
            conn.pragma_update(None, "query_only", true).map_err(err)?;
        }
        conn.authorizer(Some(move |ctx: rusqlite::hooks::AuthContext<'_>| {
            use rusqlite::hooks::{AuthAction as A, Authorization as R};
            match ctx.action {
                A::Insert { table_name } | A::Delete { table_name } | A::Update { table_name, .. }
                    if access == Access::ReadWrite && OWNED.contains(&table_name) =>
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
        Ok(Self { conn, schema_ready })
    }

    pub(crate) fn execute(&mut self, operation: &Operation) -> Result<Data> {
        match operation {
            Operation::Read { .. } | Operation::ReadAll if !self.schema_ready => Ok(Data::Units(Vec::new())),
            _ if !self.schema_ready => Err("Prerender projekta nije inicijaliziran.".into()),
            Operation::Publish(unit) => self.publish(unit),
            Operation::Read { fingerprints } => self.read(fingerprints),
            Operation::ReadAll => self.read_all(),
        }
    }

    fn publish(&mut self, unit: &RenderedUnit) -> Result<Data> {
        valid_fingerprint(&unit.fingerprint)?;
        if unit.frames == 0 || unit.file_uri.trim().is_empty() || unit.encoder.trim().is_empty() {
            return Err("Neispravna jedinica prerendera.".into());
        }
        self.conn
            .execute(
                "INSERT INTO prerender_units (fingerprint, file_uri, frames, samples, encoder, key_json, media_json, rendered_at_utc)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                 ON CONFLICT(fingerprint) DO UPDATE SET file_uri=excluded.file_uri, frames=excluded.frames,
                   samples=excluded.samples, encoder=excluded.encoder, key_json=excluded.key_json,
                   media_json=excluded.media_json, rendered_at_utc=excluded.rendered_at_utc",
                params![
                    unit.fingerprint,
                    unit.file_uri,
                    i64::try_from(unit.frames).map_err(err)?,
                    i64::try_from(unit.samples).map_err(err)?,
                    unit.encoder,
                    unit.key.to_string(),
                    unit.media.to_string()
                ],
            )
            .map_err(err)?;
        Ok(Data::Changed)
    }

    fn read(&mut self, fingerprints: &[String]) -> Result<Data> {
        if fingerprints.len() > MAX_READ {
            return Err("Previse jedinica u jednom citanju.".into());
        }
        let mut statement = self.conn.prepare_cached(&format!("{SELECT} WHERE fingerprint = ?1")).map_err(err)?;
        let mut units = Vec::new();
        for fingerprint in fingerprints {
            valid_fingerprint(fingerprint)?;
            let mut rows = statement.query(params![fingerprint]).map_err(err)?;
            if let Some(row) = rows.next().map_err(err)? {
                units.push(unit_of(row)?);
            }
        }
        Ok(Data::Units(units))
    }

    fn read_all(&mut self) -> Result<Data> {
        let mut statement = self.conn.prepare_cached(&format!("{SELECT} LIMIT {MAX_READ}")).map_err(err)?;
        let mut rows = statement.query([]).map_err(err)?;
        let mut units = Vec::new();
        while let Some(row) = rows.next().map_err(err)? {
            units.push(unit_of(row)?);
        }
        Ok(Data::Units(units))
    }
}

const SELECT: &str =
    "SELECT fingerprint, file_uri, frames, samples, encoder, key_json, media_json FROM public_prerender_units";

fn unit_of(row: &rusqlite::Row<'_>) -> Result<RenderedUnit> {
    let json = |index| -> Result<Value> { serde_json::from_str(&row.get::<_, String>(index).map_err(err)?).map_err(err) };
    Ok(RenderedUnit {
        fingerprint: row.get(0).map_err(err)?,
        file_uri: row.get(1).map_err(err)?,
        frames: u64::try_from(row.get::<_, i64>(2).map_err(err)?).map_err(err)?,
        samples: u64::try_from(row.get::<_, i64>(3).map_err(err)?).map_err(err)?,
        encoder: row.get(4).map_err(err)?,
        key: json(5)?,
        media: json(6)?,
    })
}

fn valid_fingerprint(value: &str) -> Result<()> {
    if value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err("Neispravan otisak jedinice prerendera.".into())
    }
}

fn ensure_schema(conn: &Connection) -> Result<()> {
    // A development table of an earlier form is removed, never converted (no migrations).
    if object_exists(conn, "table", "prerender_units")? && !current_form(conn)? {
        conn.execute_batch("DROP VIEW IF EXISTS public_prerender_units; DROP TABLE prerender_units;").map_err(err)?;
    }
    if !object_exists(conn, "table", "prerender_units")? {
        conn.execute_batch(
            "CREATE TABLE prerender_units (
                fingerprint TEXT PRIMARY KEY CHECK (length(fingerprint) = 16),
                file_uri TEXT NOT NULL,
                frames INTEGER NOT NULL CHECK (frames > 0),
                samples INTEGER NOT NULL CHECK (samples >= 0),
                encoder TEXT NOT NULL,
                key_json TEXT NOT NULL,
                media_json TEXT NOT NULL,
                rendered_at_utc TEXT NOT NULL
            );",
        )
        .map_err(err)?;
    }
    if !object_exists(conn, "view", "public_prerender_units")? {
        conn.execute_batch(
            "CREATE VIEW public_prerender_units AS
             SELECT fingerprint, file_uri, frames, samples, encoder, key_json, media_json, rendered_at_utc
             FROM prerender_units;",
        )
        .map_err(err)?;
    }
    Ok(())
}

/// The table exists in its current form (with the unit description).
fn current_form(conn: &Connection) -> Result<bool> {
    Ok(object_exists(conn, "table", "prerender_units")?
        && conn
            .query_row("SELECT COUNT(*) FROM pragma_table_info('prerender_units') WHERE name='media_json'", [], |r| r.get::<_, i64>(0))
            .map_err(err)?
            == 1)
}

fn object_exists(conn: &Connection, kind: &str, name: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type=?1 AND name=?2)",
        params![kind, name],
        |r| r.get(0),
    )
    .map_err(err)
}

pub(crate) fn err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

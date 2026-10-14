//! The local executor: one connection per table module, all on the same file.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde_json::Value;

use crate::{project_id, Access, Request, Result, TableModule, TableModuleFactory, VERSION};

pub struct ProjectDb {
    file: PathBuf,
    uri: String,
    access: Access,
    factories: Vec<Arc<dyn TableModuleFactory>>,
    modules: BTreeMap<&'static str, Box<dyn TableModule>>,
}

impl std::fmt::Debug for ProjectDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectDb").field("uri", &self.uri).finish_non_exhaustive()
    }
}

impl ProjectDb {
    /// The database of project `uri` at the private `file` binding the project gives.
    /// It must exist and belong to that project; nothing creates it here.
    pub fn open(
        file: &Path,
        uri: &str,
        access: Access,
        factories: Vec<Arc<dyn TableModuleFactory>>,
    ) -> Result<Self> {
        let id = project_id(uri)?;
        if !file.is_file() {
            return Err(qnc_ui_text::text("db.project_database_does_exist").into());
        }
        let check = connect(file, Access::ReadOnly)?;
        let owner: Option<String> = check
            .query_row(
                "SELECT min(project_id) FROM public_project_settings HAVING count(*) = 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| qnc_ui_text::text("db.file_project_database").to_string())?;
        if owner.as_deref() != Some(id.as_str()) {
            return Err(qnc_ui_text::text("db.database_belongs_another_project").into());
        }
        Ok(Self {
            file: file.to_path_buf(),
            uri: uri.into(),
            access,
            factories,
            modules: BTreeMap::new(),
        })
    }

    pub fn uri(&self) -> &str {
        &self.uri
    }

    pub fn execute(&mut self, request: &Request) -> Result<Value> {
        if request.version != VERSION || request.db_uri != self.uri {
            return Err(qnc_ui_text::text("db.invalid_request_project_database").into());
        }
        let factory = self
            .factories
            .iter()
            .find(|factory| factory.id() == request.module)
            .cloned()
            .ok_or_else(|| qnc_ui_text::fill("db.unknown_module", &[("module", &request.module)]))?;
        if factory.is_write(&request.payload) && self.access == Access::ReadOnly {
            return Err(qnc_ui_text::text("db.access_read_only").into());
        }
        if !self.modules.contains_key(factory.id()) {
            let module = factory.attach(connect(&self.file, self.access)?, self.access)?;
            self.modules.insert(factory.id(), module);
        }
        let module = self
            .modules
            .get_mut(factory.id())
            .ok_or(qnc_ui_text::text("db.project_database_module_open"))?;
        module.execute(request.payload.clone())
    }
}

/// A connection with the policy of the project database: waits for other writers,
/// keeps the rollback journal file (the project directory denies deletion) unless
/// the project is in WAL, never changes the identity of the file.
fn connect(file: &Path, access: Access) -> Result<Connection> {
    let flags = if access == Access::ReadOnly {
        OpenFlags::SQLITE_OPEN_READ_ONLY
    } else {
        OpenFlags::SQLITE_OPEN_READ_WRITE
    };
    let conn = Connection::open_with_flags(file, flags).map_err(|e| e.to_string())?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "trusted_schema", "OFF")
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| e.to_string())?;
    if access == Access::ReadWrite {
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |r| r.get(0))
            .map_err(|e| e.to_string())?;
        if !mode.eq_ignore_ascii_case("wal") {
            conn.pragma_update(None, "journal_mode", "PERSIST")
                .map_err(|e| e.to_string())?;
        }
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(|e| e.to_string())?;
        keep_wal_file(&conn);
    }
    Ok(conn)
}

/// The WAL file stays when this connection closes. The project directory denies deletion
/// (the delete lock of the project), so SQLite's delete of the WAL file at the close of
/// the last writing connection was refused and retried for ~3 s while it held the
/// database; every reader and writer waited (live 2026-10-12: a Sync play and a clip
/// waited 1.5-3 s). Closing now takes ~1 ms; the file is reused, never removed.
fn keep_wal_file(conn: &Connection) {
    let mut on: std::os::raw::c_int = 1;
    // SAFETY: a file control of this open connection's main database; it only sets a flag.
    unsafe {
        rusqlite::ffi::sqlite3_file_control(
            conn.handle(),
            c"main".as_ptr(),
            rusqlite::ffi::SQLITE_FCNTL_PERSIST_WAL,
            (&mut on as *mut std::os::raw::c_int).cast(),
        );
    }
}

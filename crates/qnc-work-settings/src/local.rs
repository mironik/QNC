use crate::{ReadError, WorkSettings};
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub const LOCAL_REGISTRY_URI: &str = "qnc://local/db/project_registry";

fn open_read_only(path: &Path) -> Result<Connection, ReadError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|_| {
        ReadError::new(
            "database_unavailable",
            "Baza radnih postavki nije dostupna.",
        )
    })?;
    conn.busy_timeout(Duration::from_secs(2))
        .map_err(|_| db_error())?;
    // Connection-local guard only; never create, repair or configure journal files.
    conn.pragma_update(None, "query_only", true)
        .map_err(|_| db_error())?;
    Ok(conn)
}

fn db_error() -> ReadError {
    ReadError::new(
        "database_contract",
        "Baza nema trazeni javni DB prikaz ili nije citljiva.",
    )
}

fn bound_path(resolver: &ResolverConfig, uri: &str) -> Result<PathBuf, ReadError> {
    match resolver.resolve(uri).map_err(|_| db_error())?.endpoint {
        ResolvedEndpoint::LocalPath(path) => Ok(path),
        _ => Err(db_error()),
    }
}

pub(crate) fn read(registry_file: &Path, context_uri: &str) -> Result<WorkSettings, ReadError> {
    let context = crate::registry_context(context_uri)?;
    let resolver =
        ResolverConfig::new(PathBuf::new()).with_local_binding(LOCAL_REGISTRY_URI, registry_file);
    let mut registry = open_read_only(&bound_path(&resolver, LOCAL_REGISTRY_URI)?)?;
    let tx = registry.transaction().map_err(|_| db_error())?;
    let active: Option<String> = tx
        .query_row(
            "SELECT value FROM public_app_settings WHERE key = 'active_project_id'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| db_error())?;
    let active = active.filter(|id| !id.is_empty()).ok_or_else(|| {
        ReadError::new("no_active_project", "U bazi nije odabran aktivni projekt.")
    })?;
    if active.contains(['/', '\\', ':']) || active == "." || active == ".." {
        return Err(db_error());
    }
    let (name, project_uri): (String, String) = tx
        .query_row(
            "SELECT name, project_uri FROM public_projects WHERE project_id = ?1",
            [&active],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| db_error())?;
    let public_uri = qnc_contracts::parse_qnc_uri(&project_uri).map_err(|_| db_error())?;
    if public_uri.resource_kind != "project" || public_uri.resource_id != active {
        return Err(db_error());
    }

    // Private owner-provided filesystem binding, used only inside the local transport adapter.
    // It is not returned to a consumer and is never used to choose/activate a project.
    let directory: String = tx
        .query_row(
            "SELECT local_path FROM project_storage_locations WHERE project_id = ?1",
            [&active],
            |r| r.get(0),
        )
        .map_err(|_| {
            ReadError::new(
                "missing_binding",
                "Nedostaje transport lokacija odabrane baze.",
            )
        })?;
    let directory = PathBuf::from(directory);
    if !directory.is_absolute() {
        return Err(ReadError::new(
            "missing_binding",
            "Transport lokacija baze nije potpuna.",
        ));
    }
    let local_db_uri = format!("qnc://local/db/project_workspace/{active}");
    let resolver = resolver.with_local_binding(&local_db_uri, directory.join("qnc_project.db"));
    let db = open_read_only(&bound_path(&resolver, &local_db_uri)?)?;
    let settings: String = db
        .query_row(
            "SELECT settings_json FROM public_project_settings WHERE project_id = ?1",
            [&active],
            |r| r.get(0),
        )
        .map_err(|_| {
            ReadError::new(
                "settings_not_public",
                "Javni prikaz baze ne sadrzi radne postavke.",
            )
        })?;
    let settings = serde_json::from_str(&settings).map_err(|_| db_error())?;
    let snapshot = WorkSettings::from_saved(
        active.clone(),
        name,
        format!("{context}/db/project_workspace/{active}"),
        format!("{context}/project/{active}"),
        settings,
    )?;
    tx.commit().map_err(|_| db_error())?;
    Ok(snapshot)
}

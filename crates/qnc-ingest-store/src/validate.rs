use crate::*;

pub(crate) fn open_connection(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create DB parent failed for {}: {error}", path.display()))?;
    }
    let conn = Connection::open(path)
        .map_err(|error| format!("open SQLite DB failed for {}: {error}", path.display()))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|error| format!("set SQLite busy_timeout failed: {error}"))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|error| format!("set SQLite WAL failed: {error}"))?;
    Ok(conn)
}

pub(crate) fn sqlite_db_path(resolver: &ResolverConfig, uri: &str) -> Result<PathBuf, String> {
    let resolved = resolver
        .resolve(uri)
        .map_err(|error| format!("resolve {uri} failed: {error}"))?;
    match resolved.endpoint {
        ResolvedEndpoint::LocalPath(path) => Ok(path),
        ResolvedEndpoint::NetworkEndpoint { .. } => Err(format!(
            "{uri} requires a DB transport gateway; direct SQLite file open is local binding only"
        )),
    }
}

pub(crate) fn validate_ingest_db_transport(transport: &IngestDbTransport) -> Result<(), String> {
    validate_db_uri(&transport.registry_uri, INGEST_REGISTRY_DB_ID)?;
    Ok(())
}

pub(crate) fn validate_db_contracts() -> Result<(), String> {
    // The content tables belong to the project database; its one public writer is
    // the content store, never an application (user rule 2026-09-25).
    for (name, contents, expected_id, writer) in [
        (
            "contracts/databases/ingest-registry.database.json",
            INGEST_REGISTRY_DB_CONTRACT,
            INGEST_REGISTRY_DB_ID,
            APPLICATION_ID,
        ),
        (
            "contracts/databases/ingest-content.database.json",
            INGEST_CONTENT_DB_CONTRACT,
            INGEST_CONTENT_DB_ID,
            PROJECT_DB_WRITER,
        ),
    ] {
        let contract = DatabaseContract::from_json_str(name, contents)
            .map_err(|report| report.errors.join("; "))?;
        if contract.database_id != expected_id {
            return Err(format!(
                "{name}: database_id '{}' does not match {expected_id}",
                contract.database_id
            ));
        }
        contract
            .validate_write(writer)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

pub(crate) fn validate_db_uri(uri: &str, expected_db_id: &str) -> Result<(), String> {
    let parsed = qnc_contracts::parse_qnc_uri(uri)
        .map_err(|error| format!("invalid {expected_db_id} transport URI: {error}"))?;
    if parsed.resource_kind != "db" {
        return Err(format!("{expected_db_id} transport URI must use db kind"));
    }
    let expected_resource = expected_db_id
        .strip_prefix("qnc.db.")
        .unwrap_or(expected_db_id)
        .replace('.', "_");
    if parsed.resource_id != expected_resource {
        return Err(format!(
            "{expected_db_id} transport URI resource must be {expected_resource}"
        ));
    }
    Ok(())
}

pub(crate) fn validate_source_record(record: &SourceSelectionRecord) -> Result<(), String> {
    qnc_contracts::parse_qnc_uri(&record.source_uri)
        .map_err(|error| format!("invalid source URI: {error}"))?;
    if record.source_kind.trim().is_empty() {
        return Err("source_kind is required".to_string());
    }
    if record.display_name.trim().is_empty() {
        return Err("display_name is required".to_string());
    }
    Ok(())
}

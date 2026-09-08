use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use qnc_db_contract::DatabaseContract;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use rusqlite::{params, Connection};

pub mod content;

pub const INGEST_REGISTRY_DB_ID: &str = "qnc.db.ingest_registry";
pub const INGEST_CONTENT_DB_ID: &str = "qnc.db.ingest_content";
pub const INGEST_REGISTRY_DB_LOCAL_URI: &str = "qnc://local/db/ingest_registry";
pub const INGEST_CONTENT_DB_LOCAL_URI: &str = "qnc://local/db/ingest_content";
const INGEST_REGISTRY_DB_CONTRACT: &str =
    include_str!("../../../contracts/databases/ingest-registry.database.json");
const INGEST_CONTENT_DB_CONTRACT: &str =
    include_str!("../../../contracts/databases/ingest-content.database.json");
const APPLICATION_ID: &str = "qnc.ingest";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngestDbTransport {
    pub registry_uri: String,
}

impl IngestDbTransport {
    pub fn local_default() -> Self {
        Self {
            registry_uri: INGEST_REGISTRY_DB_LOCAL_URI.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSelectionRecord {
    pub source_uri: String,
    pub source_kind: String,
    pub display_name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub private_local_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSessionRecord {
    pub session_id: String,
    pub source_uri: String,
    pub card_id: Option<String>,
    pub selected_at_utc: String,
}

pub struct IngestStore {
    registry: Connection,
}

impl std::fmt::Debug for IngestStore {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IngestStore")
            .finish_non_exhaustive()
    }
}

impl IngestStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, String> {
        Self::open_with_transport(root, IngestDbTransport::local_default())
    }

    pub fn open_with_transport(
        root: impl AsRef<Path>,
        transport: IngestDbTransport,
    ) -> Result<Self, String> {
        validate_db_contracts()?;
        validate_ingest_db_transport(&transport)?;
        let root = root.as_ref();
        let data_dir = root.join("data");
        std::fs::create_dir_all(&data_dir)
            .map_err(|error| format!("create ingest data directory failed: {error}"))?;

        let resolver = ResolverConfig::new(data_dir.clone())
            .with_local_binding(&transport.registry_uri, data_dir.join("ingest_registry.db"));

        let registry_path = sqlite_db_path(&resolver, &transport.registry_uri)?;
        let registry = open_connection(&registry_path)?;

        let store = Self { registry };
        store.create_registry_schema()?;
        Ok(store)
    }

    pub fn record_source_selection(
        &mut self,
        record: &SourceSelectionRecord,
    ) -> Result<SourceSessionRecord, String> {
        validate_source_record(record)?;
        let card_id = card_id(record);
        let selected_at_utc = current_utc_text();
        let session_id = session_id(&record.source_uri);
        let tx = self
            .registry
            .transaction()
            .map_err(|error| format!("begin ingest registry transaction failed: {error}"))?;

        tx.execute(
            "INSERT INTO source_locations (
                source_uri, source_kind, display_name, first_seen_at_utc, last_seen_at_utc
             )
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(source_uri) DO UPDATE SET
                source_kind = excluded.source_kind,
                display_name = excluded.display_name,
                last_seen_at_utc = ?4",
            params![
                record.source_uri,
                record.source_kind,
                record.display_name,
                selected_at_utc
            ],
        )
        .map_err(|error| format!("upsert source location failed: {error}"))?;

        if let Some(path) = &record.private_local_path {
            tx.execute(
                "INSERT INTO source_location_bindings (
                    source_uri, local_path, updated_at_utc
                 )
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(source_uri) DO UPDATE SET
                    local_path = excluded.local_path,
                    updated_at_utc = ?3",
                params![
                    record.source_uri,
                    path.to_string_lossy().to_string(),
                    selected_at_utc
                ],
            )
            .map_err(|error| format!("upsert source binding failed: {error}"))?;
        }

        if let Some(card_id) = card_id.as_deref() {
            tx.execute(
                "INSERT INTO source_cards (
                    card_id, serial_number, volume_name, display_name,
                    first_seen_at_utc, last_seen_at_utc
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                 ON CONFLICT(card_id) DO UPDATE SET
                    serial_number = excluded.serial_number,
                    volume_name = excluded.volume_name,
                    display_name = excluded.display_name,
                    last_seen_at_utc = ?5",
                params![
                    card_id,
                    record.serial_number,
                    record.volume_name,
                    record.display_name,
                    selected_at_utc
                ],
            )
            .map_err(|error| format!("upsert source card failed: {error}"))?;
        }

        tx.execute(
            "INSERT INTO source_sessions (
                session_id, source_uri, card_id, selected_at_utc, status
             )
             VALUES (?1, ?2, ?3, ?4, 'selected')",
            params![
                session_id,
                record.source_uri,
                card_id.as_deref(),
                selected_at_utc
            ],
        )
        .map_err(|error| format!("insert source session failed: {error}"))?;

        tx.commit()
            .map_err(|error| format!("commit ingest registry transaction failed: {error}"))?;

        Ok(SourceSessionRecord {
            session_id,
            source_uri: record.source_uri.clone(),
            card_id,
            selected_at_utc,
        })
    }

    pub fn registry_connection(&self) -> &Connection {
        &self.registry
    }

    fn create_registry_schema(&self) -> Result<(), String> {
        self.registry
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS source_cards (
                    card_id TEXT PRIMARY KEY,
                    serial_number TEXT NOT NULL,
                    volume_name TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    first_seen_at_utc TEXT NOT NULL,
                    last_seen_at_utc TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS source_locations (
                    source_uri TEXT PRIMARY KEY,
                    source_kind TEXT NOT NULL,
                    display_name TEXT NOT NULL,
                    first_seen_at_utc TEXT NOT NULL,
                    last_seen_at_utc TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS source_sessions (
                    session_id TEXT PRIMARY KEY,
                    source_uri TEXT NOT NULL,
                    card_id TEXT,
                    selected_at_utc TEXT NOT NULL,
                    completed_at_utc TEXT,
                    status TEXT NOT NULL,
                    FOREIGN KEY(source_uri) REFERENCES source_locations(source_uri),
                    FOREIGN KEY(card_id) REFERENCES source_cards(card_id)
                );

                CREATE TABLE IF NOT EXISTS source_location_bindings (
                    source_uri TEXT PRIMARY KEY,
                    local_path TEXT NOT NULL,
                    updated_at_utc TEXT NOT NULL,
                    FOREIGN KEY(source_uri) REFERENCES source_locations(source_uri)
                );

                CREATE VIEW IF NOT EXISTS public_source_cards AS
                    SELECT card_id, serial_number, volume_name, display_name,
                           first_seen_at_utc, last_seen_at_utc
                    FROM source_cards;

                CREATE VIEW IF NOT EXISTS public_source_locations AS
                    SELECT source_uri, source_kind, display_name,
                           first_seen_at_utc, last_seen_at_utc
                    FROM source_locations;

                CREATE VIEW IF NOT EXISTS public_source_sessions AS
                    SELECT session_id, source_uri, card_id, selected_at_utc,
                           completed_at_utc, status
                    FROM source_sessions;
                ",
            )
            .map_err(|error| format!("create ingest registry schema failed: {error}"))
    }
}

fn open_connection(path: &Path) -> Result<Connection, String> {
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

fn sqlite_db_path(resolver: &ResolverConfig, uri: &str) -> Result<PathBuf, String> {
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

fn validate_ingest_db_transport(transport: &IngestDbTransport) -> Result<(), String> {
    validate_db_uri(&transport.registry_uri, INGEST_REGISTRY_DB_ID)?;
    Ok(())
}

fn validate_db_contracts() -> Result<(), String> {
    for (name, contents, expected_id) in [
        (
            "contracts/databases/ingest-registry.database.json",
            INGEST_REGISTRY_DB_CONTRACT,
            INGEST_REGISTRY_DB_ID,
        ),
        (
            "contracts/databases/ingest-content.database.json",
            INGEST_CONTENT_DB_CONTRACT,
            INGEST_CONTENT_DB_ID,
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
            .validate_write(APPLICATION_ID)
            .map_err(|error| format!("{error:?}"))?;
    }
    Ok(())
}

fn validate_db_uri(uri: &str, expected_db_id: &str) -> Result<(), String> {
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

fn validate_source_record(record: &SourceSelectionRecord) -> Result<(), String> {
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

fn card_id(record: &SourceSelectionRecord) -> Option<String> {
    let serial = record.serial_number.trim();
    let volume = record.volume_name.trim();
    if serial.is_empty() && volume.is_empty() {
        return None;
    }
    let key = format!(
        "{}:{}:{}",
        record.source_kind.trim().to_ascii_lowercase(),
        serial.to_ascii_lowercase(),
        volume.to_ascii_lowercase()
    );
    Some(format!("card_{:016x}", fnv1a64(&key)))
}

fn session_id(source_uri: &str) -> String {
    let key = format!("{}:{}", source_uri, current_utc_nanos_text());
    format!("ingest_session_{:016x}", fnv1a64(&key))
}

fn current_utc_text() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    format!("unix:{seconds}")
}

fn current_utc_nanos_text() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("unix_nanos:{nanos}")
}

fn fnv1a64(value: &str) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn opens_registry_without_creating_a_second_global_clip_catalog() {
        let root = temp_root("open");
        {
            let store = IngestStore::open(&root).expect("open ingest store");
            let registry_tables = count_tables(store.registry_connection(), "source_%");
            assert!(registry_tables >= 4);
            assert!(!root.join("data/ingest_content.db").exists());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn source_selection_writes_card_location_and_session() {
        let root = temp_root("source");
        {
            let mut store = IngestStore::open(&root).expect("open ingest store");
            let session = store
                .record_source_selection(&SourceSelectionRecord {
                    source_uri: "qnc://local/source/test_card".to_string(),
                    source_kind: "local".to_string(),
                    display_name: "G:".to_string(),
                    serial_number: "de666c9f".to_string(),
                    volume_name: "camera".to_string(),
                    private_local_path: Some(PathBuf::from(r"G:\")),
                })
                .expect("record source selection");

            assert!(session.session_id.starts_with("ingest_session_"));
            assert_eq!(session.source_uri, "qnc://local/source/test_card");
            assert!(session
                .card_id
                .as_deref()
                .is_some_and(|id| id.starts_with("card_")));

            let public_location_count: i64 = store
                .registry_connection()
                .query_row(
                    "SELECT COUNT(*) FROM public_source_locations WHERE source_uri = ?1",
                    params!["qnc://local/source/test_card"],
                    |row| row.get(0),
                )
                .expect("public source location count");
            assert_eq!(public_location_count, 1);

            let private_path_count: i64 = store
                .registry_connection()
                .query_row(
                    "SELECT COUNT(*) FROM source_location_bindings WHERE source_uri = ?1 AND local_path = ?2",
                    params!["qnc://local/source/test_card", r"G:\"],
                    |row| row.get(0),
                )
                .expect("private binding count");
            assert_eq!(private_path_count, 1);
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn public_location_view_does_not_expose_private_local_path() {
        let root = temp_root("public");
        {
            let mut store = IngestStore::open(&root).expect("open ingest store");
            store
                .record_source_selection(&SourceSelectionRecord {
                    source_uri: "qnc://local/source/card_public".to_string(),
                    source_kind: "local".to_string(),
                    display_name: "CARD".to_string(),
                    serial_number: "12345678".to_string(),
                    volume_name: "CARD".to_string(),
                    private_local_path: Some(PathBuf::from(r"H:\")),
                })
                .expect("record source selection");

            let columns = store
                .registry_connection()
                .prepare("SELECT * FROM public_source_locations")
                .expect("prepare public view")
                .column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>();
            assert!(!columns.iter().any(|name| name == "local_path"));
        }
        let _ = fs::remove_dir_all(root);
    }

    fn count_tables(conn: &Connection, like: &str) -> i64 {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'view') AND name LIKE ?1",
            params![like],
            |row| row.get(0),
        )
        .expect("count tables")
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "qnc_ingest_store_{label}_{}_{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ))
    }
}

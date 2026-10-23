use crate::*;

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

use qnc_workstation_identity::IdentitySnapshot;
use rusqlite::{params, Connection};

pub(super) struct ProjectOrigin {
    pub name: String,
    pub created_at: String,
    pub identity: IdentitySnapshot,
}

pub(super) fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS project_origin (
            project_id TEXT PRIMARY KEY,
            project_name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            identity_json TEXT NOT NULL CHECK(json_valid(identity_json))
        );
        CREATE VIEW IF NOT EXISTS public_project_origin AS
            SELECT project_id, project_name, created_at,
                json_extract(identity_json, '$.contract_version') AS identity_contract_version,
                json_extract(identity_json, '$.workstation_name') AS workstation_name,
                json_extract(identity_json, '$.user_name') AS user_name,
                json_extract(identity_json, '$.hardware_serial.value') AS serial_number,
                json_extract(identity_json, '$.hardware_serial.kind') AS serial_kind,
                json_extract(identity_json, '$.hardware_serial.source') AS serial_source,
                identity_json
            FROM project_origin;",
    )
    .map_err(|error| error.to_string())
}

pub(super) fn insert(
    conn: &Connection,
    project_id: &str,
    origin: &ProjectOrigin,
) -> Result<(), String> {
    let identity = serde_json::to_string(&origin.identity).map_err(|error| error.to_string())?;
    // Insert only on creation. Reopening/copying a project must never recapture its origin.
    conn.execute(
        "INSERT INTO project_origin (project_id, project_name, created_at, identity_json) VALUES (?1, ?2, ?3, ?4)",
        params![project_id, origin.name, origin.created_at, identity],
    ).map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn display_date(conn: &Connection, timestamp: &str) -> Result<String, String> {
    conn.query_row(
        "SELECT COALESCE(strftime('%d.%m.%Y.', substr(?1, 7), 'unixepoch', 'localtime'), '')",
        params![timestamp],
        |row| row.get(0),
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_workstation_identity::{
        IdentityField, IdentitySource, ReadFailure, ReadIssue, CONTRACT_VERSION,
    };

    #[test]
    fn unavailable_hardware_is_stored_as_null_not_fabricated() {
        let db = Connection::open_in_memory().unwrap();
        init_schema(&db).unwrap();
        let origin = ProjectOrigin {
            name: "Synthetic".into(),
            created_at: "epoch_1788696000".into(),
            identity: IdentitySnapshot {
                contract_version: CONTRACT_VERSION.into(),
                workstation_name: Some("Test workstation".into()),
                user_name: Some("test-user".into()),
                hardware_serial: None,
                issues: vec![ReadIssue {
                    field: IdentityField::HardwareSerial,
                    source: IdentitySource::SmbiosSystemSerial,
                    reason: ReadFailure::Unavailable,
                }],
            },
        };
        insert(&db, "test-id", &origin).unwrap();
        let value: Option<String> = db
            .query_row("SELECT serial_number FROM public_project_origin", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(value.is_none());
        assert!(insert(&db, "test-id", &origin).is_err());
        let stored: String = db
            .query_row("SELECT identity_json FROM public_project_origin", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            serde_json::from_str::<IdentitySnapshot>(&stored).unwrap(),
            origin.identity
        );
    }
}

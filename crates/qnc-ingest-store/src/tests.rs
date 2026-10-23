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

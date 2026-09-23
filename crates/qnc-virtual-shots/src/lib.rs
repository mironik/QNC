//! Public virtual shot component.
//!
//! This crate does not own SQLite and never opens a project DB file directly.
//! Durable state is read and written through `qnc-content-store`.

pub use qnc_content_store::{SavedShort, ShortClip};

pub const MODULE_ID: &str = "qnc.module.virtual-shots";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn list_shorts(
    target: &qnc_content_store::ContentTarget,
) -> Result<Vec<qnc_content_store::ShortClip>, String> {
    target
        .open(qnc_content_store::Access::ReadOnly)?
        .list_shorts()
}

pub fn save_short(
    transport: &mut qnc_content_store::ContentWriteTransport,
    key: String,
    project_id: String,
    clip_id: String,
    clip_name: String,
    in_frame: u64,
    out_frame: u64,
) -> Result<(), String> {
    transport.save_short(key, project_id, clip_id, clip_name, in_frame, out_frame)
}

pub fn mark_stills_ready(
    transport: &mut qnc_content_store::ContentWriteTransport,
    key: String,
    shot_id: String,
    in_uri: String,
    out_uri: String,
) -> Result<(), String> {
    transport.mark_short_stills_ready(key, shot_id, in_uri, out_uri)
}

pub fn mark_stills_failed(
    transport: &mut qnc_content_store::ContentWriteTransport,
    key: String,
    shot_id: String,
    error: String,
) -> Result<(), String> {
    transport.mark_short_stills_failed(key, shot_id, error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_content_store::{
        Access, ContentStore, ContentTarget, ContentWriteData, ContentWriteTransport,
    };

    const URI: &str = "qnc://local/db/ingest_content/p1";

    fn target() -> (tempfile::TempDir, ContentTarget) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("project.db");
        {
            let conn = rusqlite::Connection::open(&file).unwrap();
            conn.execute_batch(
                "CREATE TABLE project_settings (project_id TEXT);
                 INSERT INTO project_settings (project_id) VALUES ('p1');
                 CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;",
            )
            .unwrap();
        }
        drop(ContentStore::open_owner_binding(&file, URI, Access::ReadWrite).unwrap());
        let conn = rusqlite::Connection::open(&file).unwrap();
        conn.execute(
            "INSERT INTO clips (
                clip_id, source_uri, original_uri, name, catalog_json, revision, final,
                selected, import_status
             ) VALUES (
                'clip-a', 'qnc://local/source/card-a',
                'qnc://local/source/card-a/clip-a.mxf', 'Mironik', '{}', 1, 1, 0, 'imported'
             )",
            [],
        )
        .unwrap();
        drop(conn);
        (dir, ContentTarget::from_owner_binding(&file, URI).unwrap())
    }

    #[test]
    fn writes_short_through_content_transport() {
        let (_dir, target) = target();
        let mut transport = ContentWriteTransport::start(target.clone()).unwrap();
        save_short(
            &mut transport,
            "short".into(),
            "p1".into(),
            "clip-a".into(),
            "Mironik".into(),
            10,
            40,
        )
        .unwrap();
        let completion = loop {
            let mut completions = transport.poll();
            if let Some(completion) = completions.pop() {
                break completion;
            }
        };
        let shot = match completion.result.unwrap().data {
            ContentWriteData::SavedShort(shot) => *shot,
            data => panic!("unexpected write result: {data:?}"),
        };
        assert_eq!(shot.shot_id, "clip-a_shot_001");
        let shorts = list_shorts(&target).unwrap();
        assert_eq!(shorts[0].shot_id, "clip-a_shot_001");
        assert_eq!((shorts[0].in_frame, shorts[0].out_frame), (10, 40));
    }
}

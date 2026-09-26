//! Artifact requests on a project database that also holds the clip catalog, as a
//! real project database does (moved from the former content store tests).

use super::*;
use qnc_content_store::{Access as ContentAccess, ContentStore};
use rusqlite::Connection;
use std::path::Path;

const CONTENT: &str = "qnc://local/db/ingest_content/p1";
const PROJECT_DB: &str = "qnc://local/db/project_db/p1";

/// A project database with clips c1, c2 (detected) and c3 (imported).
fn project() -> (tempfile::TempDir, ProjectDbTarget) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    Connection::open(&file)
        .unwrap()
        .execute_batch(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings (project_id) VALUES ('p1');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;",
        )
        .unwrap();
    drop(ContentStore::open_owner_binding(&file, CONTENT, ContentAccess::ReadWrite).unwrap());
    let conn = Connection::open(&file).unwrap();
    for (clip, status) in [("c1", "detected"), ("c2", "detected"), ("c3", "imported")] {
        conn.execute(
            "INSERT INTO clips (
                clip_id, source_uri, original_uri, name, catalog_json, revision, final,
                selected, import_status
             ) VALUES (?1, 'qnc://local/source/card',
                'qnc://local/source/card/media/' || ?1 || '.wav', ?1, '{}', 1, 1, 0, ?2)",
            [clip, status],
        )
        .unwrap();
    }
    drop(conn);
    (dir, ProjectDbTarget::from_owner_binding(&file, PROJECT_DB).unwrap())
}

fn file(dir: &tempfile::TempDir) -> std::path::PathBuf {
    dir.path().join("project.db")
}

fn filmstrip(id: &str) -> FilmstripArtifactRecord {
    FilmstripArtifactRecord {
        clip_id: id.into(),
        status: "ready".into(),
        duration_sec: "10.00".into(),
        frame_count: 2,
        artifact_uri: format!("qnc://local/project/p1/products/filmstrip/{id}"),
        frames: vec![
            FilmstripFrameRecord {
                index: 0,
                seek_sec: "0.00".into(),
                artifact_uri: format!("qnc://local/project/p1/products/filmstrip/{id}/000_0_00.jpg"),
            },
            FilmstripFrameRecord {
                index: 1,
                seek_sec: "5.00".into(),
                artifact_uri: format!("qnc://local/project/p1/products/filmstrip/{id}/001_5_00.jpg"),
            },
        ],
    }
}

fn wave(id: &str) -> WaveArtifactRecord {
    WaveArtifactRecord {
        clip_id: id.into(),
        status: "ready".into(),
        artifact_uri: format!("qnc://local/db/ingest_content/p1/wave/{id}"),
        source_uri: format!("qnc://local/source/card/media/{id}.wav"),
        source_sample_rate_hz: 48_000,
        peak_count: 3,
        a1_peaks: vec![0.0, 0.5, 1.0],
        a2_peaks: vec![0.1, 0.4, 0.8],
        a3_peaks: Vec::new(),
        a4_peaks: Vec::new(),
        warning: None,
        render_version: qnc_wave::WAVE_RENDER_VERSION,
    }
}

fn count(path: &Path, sql: &str) -> u32 {
    Connection::open(path).unwrap().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn a_project_without_artifacts_reads_none() {
    let (_dir, target) = project();
    let mut reader = ArtifactReader::open(&target).unwrap();
    assert!(reader.read_filmstrip("c1").unwrap().is_none());
    assert!(reader.read_wave("c1").unwrap().is_none());
}

#[test]
fn filmstrip_publication_writes_public_frame_uris_without_touching_media() {
    let (dir, target) = project();
    let writer = ArtifactWriter::start(target.clone()).unwrap();
    writer
        .call(&Operation::PublishFilmstrip(Box::new(filmstrip("c1"))))
        .unwrap();
    let mut reader = ArtifactReader::open(&target).unwrap();
    assert_eq!(reader.read_filmstrip("c1").unwrap(), Some(filmstrip("c1")));
    assert!(reader.read_filmstrip("c2").unwrap().is_none());
    assert_eq!(count(&file(&dir), "SELECT count(*) FROM public_filmstrip_frames"), 2);
    let uri: String = Connection::open(file(&dir))
        .unwrap()
        .query_row(
            "SELECT artifact_uri FROM public_filmstrip_frames WHERE frame_index=1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(uri, "qnc://local/project/p1/products/filmstrip/c1/001_5_00.jpg");
    assert!(writer
        .call(&Operation::PublishFilmstrip(Box::new(filmstrip("missing"))))
        .is_err());
}

#[test]
fn the_serial_writer_takes_publications_without_waiting() {
    let (_dir, target) = project();
    let writer = ArtifactWriter::start(target.clone()).unwrap();
    let pending: Vec<_> = ["c1", "c2"]
        .iter()
        .map(|id| {
            writer
                .submit(&Operation::PublishWave(Box::new(wave(id))))
                .unwrap()
        })
        .collect();
    for reply in pending {
        let result = loop {
            if let Some(result) = reply.try_take() {
                break result;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        };
        assert_eq!(result.unwrap(), Data::Changed);
    }
    let mut reader = ArtifactReader::open(&target).unwrap();
    assert_eq!(reader.read_wave("c1").unwrap().unwrap().a1_peaks.len(), 3);
    assert_eq!(reader.read_wave("c2").unwrap().unwrap().a2_peaks.len(), 3);
}

#[test]
fn a_wave_belongs_to_the_stored_media_and_reads_refuse_writes() {
    let (dir, target) = project();
    let writer = ArtifactWriter::start(target.clone()).unwrap();
    let mut foreign = wave("c1");
    foreign.source_uri = "qnc://local/source/card/media/other.wav".into();
    assert!(writer
        .call(&Operation::PublishWave(Box::new(foreign)))
        .is_err());
    writer
        .call(&Operation::PublishWave(Box::new(wave("c1"))))
        .unwrap();
    assert_eq!(count(&file(&dir), "SELECT count(*) FROM public_wave_artifacts"), 1);
    let mut reader = target
        .open(Access::ReadOnly, vec![ArtifactsModule::factory()])
        .unwrap();
    let payload = serde_json::to_value(Operation::PublishWave(Box::new(wave("c2")))).unwrap();
    assert!(reader.execute(MODULE_ID, payload).is_err(), "read-only");
}

#[test]
fn a_missing_clip_loses_its_artifacts_first_and_an_imported_one_keeps_them() {
    let (dir, target) = project();
    let writer = ArtifactWriter::start(target.clone()).unwrap();
    for id in ["c1", "c3"] {
        writer
            .call(&Operation::PublishFilmstrip(Box::new(filmstrip(id))))
            .unwrap();
        writer
            .call(&Operation::PublishWave(Box::new(wave(id))))
            .unwrap();
    }
    // The catalog cannot remove c1 while its artifacts refer to it.
    let remove = |ids: &[&str]| {
        let mut store =
            ContentStore::open_owner_binding(&file(&dir), CONTENT, ContentAccess::ReadWrite)
                .unwrap();
        store.execute(&qnc_content_store::Request {
            version: qnc_content_store::VERSION.into(),
            db_uri: CONTENT.into(),
            operation: qnc_content_store::Operation::RemoveMissing {
                clips: ids
                    .iter()
                    .map(|id| qnc_content_store::InventoryClip {
                        clip_id: (*id).into(),
                        source_uri: "qnc://local/source/card".into(),
                        original_uri: format!("qnc://local/source/card/media/{id}.wav"),
                        revision: 1,
                        final_record: true,
                    })
                    .collect(),
            },
        })
    };
    assert!(remove(&["c1"]).is_err(), "its artifacts still refer to it");
    let forgotten = writer
        .forget_clips(vec!["c1".into(), "c3".into()])
        .unwrap();
    assert_eq!(forgotten, vec!["c1".to_string()], "an imported clip keeps them");
    assert!(remove(&["c1"]).is_ok());
    let mut reader = ArtifactReader::open(&target).unwrap();
    assert!(reader.read_filmstrip("c1").unwrap().is_none());
    assert!(reader.read_wave("c3").unwrap().is_some());
    assert!(reader.read_filmstrip("c3").unwrap().is_some());
}

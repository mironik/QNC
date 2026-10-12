use super::*;
use rusqlite::Connection;

const PROJECT_DB: &str = "qnc://local/db/project_db/p1";

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
    (dir, ProjectDbTarget::from_owner_binding(&file, PROJECT_DB).unwrap())
}

fn unit(fingerprint: &str, frames: u64) -> RenderedUnit {
    RenderedUnit {
        fingerprint: fingerprint.into(),
        file_uri: format!("qnc://local/source/project-p1/file/products/prerender/{fingerprint}%2Emov"),
        frames,
        samples: frames * 960,
        encoder: "IntelQsv".into(),
    }
}

#[test]
fn a_published_unit_is_read_back_and_an_unknown_one_is_not() {
    let (_dir, target) = project();
    let mut reader = PrerenderReader::open(&target).unwrap();
    assert!(reader.rendered(vec!["0123456789abcdef".into()]).unwrap().is_empty(), "no table yet: nothing rendered");
    let writer = PrerenderWriter::start(target.clone()).unwrap();
    writer.publish(unit("0123456789abcdef", 102)).unwrap();
    writer.publish(unit("0123456789abcdef", 102)).unwrap();
    let mut reader = PrerenderReader::open(&target).unwrap();
    let found = reader.rendered(vec!["0123456789abcdef".into(), "fedcba9876543210".into()]).unwrap();
    assert_eq!(found, vec![unit("0123456789abcdef", 102)], "the same unit once");
}

#[test]
fn a_bad_unit_is_refused() {
    let (_dir, target) = project();
    let writer = PrerenderWriter::start(target.clone()).unwrap();
    assert!(writer.publish(unit("not-a-fingerprint", 10)).is_err());
    assert!(writer.publish(unit("0123456789abcdef", 0)).is_err());
    let mut reader = PrerenderReader::open(&target).unwrap();
    assert!(reader.rendered(vec!["../etc".into()]).is_err());
}

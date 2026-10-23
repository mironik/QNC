use super::*;
use rusqlite::Connection;

const PROJECT_DB: &str = "qnc://local/db/project_db/p1";

/// A project database with one clip of 500 frames in its catalog.
fn project() -> (tempfile::TempDir, ProjectDbTarget) {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("project.db");
    Connection::open(&file)
        .unwrap()
        .execute_batch(
            "CREATE TABLE project_settings (project_id TEXT);
             INSERT INTO project_settings (project_id) VALUES ('p1');
             CREATE VIEW public_project_settings AS SELECT project_id FROM project_settings;
             CREATE TABLE clips (clip_id TEXT, duration_frames INTEGER);
             INSERT INTO clips VALUES ('c1', 500);
             CREATE VIEW public_clips AS SELECT clip_id, duration_frames FROM clips;",
        )
        .unwrap();
    (dir, ProjectDbTarget::from_owner_binding(&file, PROJECT_DB).unwrap())
}

fn wait(parts: &mut IngestParts) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while parts.is_busy() && std::time::Instant::now() < deadline {
        parts.poll();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn a_saved_part_is_listed_with_its_own_id_and_a_deleted_one_is_gone() {
    let (_dir, target) = project();
    let mut parts = IngestParts::default();
    parts.configure(Some(&target));
    wait(&mut parts);
    assert!(parts.parts().is_empty());
    parts.save("c1", "Mironik 2002", (100, 250)).unwrap();
    parts.save("c1", "Mironik 2002", (300, 500)).unwrap();
    wait(&mut parts);
    let ids: Vec<_> = parts.parts().iter().map(|part| (part.part_id.as_str(), part.name.as_str(), part.in_frame, part.out_frame)).collect();
    assert_eq!(ids, [("c1-part-01", "Mironik 2002 P01", 100, 250), ("c1-part-02", "Mironik 2002 P02", 300, 500)]);
    parts.delete("c1-part-01").unwrap();
    wait(&mut parts);
    parts.save("c1", "Mironik 2002", (0, 10)).unwrap();
    wait(&mut parts);
    let ids: Vec<_> = parts.parts().iter().map(|part| part.part_id.as_str()).collect();
    assert_eq!(ids, ["c1-part-02", "c1-part-03"], "an id is never used again");
    assert_eq!(read_parts(&target, "c1").unwrap().len(), 2, "the import reads them too");
    assert!(read_parts(&target, "c2").unwrap().is_empty());
    PartsWriter::start(target.clone()).unwrap().mark_cut("c1-part-02", "Mironik 2002_01").unwrap();
    let uncut: Vec<_> = read_uncut(&target).unwrap().into_iter().map(|part| part.part_id).collect();
    assert_eq!(uncut, ["c1-part-03"], "a cut part is not cut again");
}

#[test]
fn a_part_outside_its_clip_or_of_no_clip_is_refused_and_says_why() {
    let (_dir, target) = project();
    let mut parts = IngestParts::default();
    parts.configure(Some(&target));
    assert!(parts.save("c1", "x", (10, 10)).is_err(), "IN and OUT are needed");
    parts.save("c1", "x", (100, 501)).unwrap();
    wait(&mut parts);
    assert!(parts.take_message().is_some_and(|message| message.contains("500")));
    parts.save("nope", "x", (0, 10)).unwrap();
    wait(&mut parts);
    assert!(parts.take_message().is_some());
    assert!(parts.parts().is_empty());
}

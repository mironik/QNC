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

fn word(from: u64, text: &str, at: f64) -> WordLetters {
    let letters = text.chars().enumerate().map(|(index, letter)| Letter { letter, from: Some(at + index as f64), to: Some(at + index as f64 + 0.5) }).collect();
    WordLetters { word_from: from, text: text.into(), letters }
}

#[test]
fn the_letters_of_a_word_are_kept_and_made_again_replace_them() {
    let (_dir, target) = project();
    assert!(read_clip(&target, "clip-a").unwrap().is_empty(), "no table yet: no letters");
    save(&target, "clip-a", "test", vec![word(10, "pa", 10.0), word(20, "dosad", 20.0)]).unwrap();
    save(&target, "clip-a", "test", vec![word(10, "pa", 11.0)]).unwrap();
    assert_eq!(read_clip(&target, "clip-a").unwrap(), vec![word(10, "pa", 11.0), word(20, "dosad", 20.0)]);
    let mut bad = word(30, "ne", 30.0);
    bad.letters[0].to = None;
    assert!(save(&target, "clip-a", "test", vec![bad]).is_err(), "a letter with a start and no end is refused");
}

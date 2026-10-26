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

fn transcript(from: u64, to: u64, said: &str) -> ClipTranscript {
    let word = Word { from, to: from + 10, text: said.into() };
    ClipTranscript {
        clip_id: "c1".into(),
        from_frame: from,
        to_frame: to,
        language: "hr".into(),
        engine: "qnc.transcribe.whisper@0.1.0 ggml-large-v3-turbo.bin".into(),
        lines: vec![Line { from, to: from + 10, text: said.into(), words: vec![word] }],
    }
}

#[test]
fn a_kept_transcript_is_read_back_and_a_range_said_again_replaces_it() {
    let (_dir, target) = project();
    assert!(read_clip(&target, "c1").unwrap().is_empty(), "a project without transcripts reads none");
    save(&target, transcript(100, 200, "Dobar")).unwrap();
    save(&target, transcript(0, 50, "Vidite")).unwrap();
    save(&target, transcript(100, 200, "Dobar dan")).unwrap();
    let kept = read_clip(&target, "c1").unwrap();
    assert_eq!(kept.iter().map(|t| t.from_frame).collect::<Vec<_>>(), [0, 100], "in the order of their ranges");
    assert_eq!(kept[1].lines[0].text, "Dobar dan");
    assert!(read_clip(&target, "c2").unwrap().is_empty());
}

#[test]
fn a_word_outside_its_range_is_refused_and_nothing_is_kept() {
    let (_dir, target) = project();
    let mut outside = transcript(100, 200, "Dobar");
    outside.lines[0].words[0].to = 250;
    assert!(save(&target, outside).is_err());
    assert!(read_clip(&target, "c1").unwrap().is_empty());
}

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

fn shot(from: u64, to: u64, looks: &[&str]) -> ShotRecord {
    ShotRecord { from, to, looks: looks.iter().map(|l| l.to_string()).collect(), key_frame: from, brightness: 100.0, sharpness: 50.0, motion: 0.02 }
}

fn analysis(clip: &str, shots: Vec<ShotRecord>) -> ClipShots {
    ClipShots { clip_id: clip.into(), media_uri: "qnc://local/source/s/file/c.mxf".into(), pictures: 12, shots, parts: Vec::new(), error: String::new(), took_ms: 900 }
}

#[test]
fn a_kept_analysis_is_read_back_and_a_new_one_replaces_it() {
    let (_dir, target) = project();
    assert_eq!(read_clip(&target, "c1").unwrap(), None, "a project without the table reads none");
    assert!(read_analysed(&target).unwrap().is_empty());
    save(&target, analysis("c1", vec![shot(0, 100, &["still"]), shot(100, 300, &["moving"])]), None).unwrap();
    save(&target, analysis("c2", Vec::new()), None).unwrap();
    let failed = ClipShots { error: "no index".into(), ..analysis("c3", Vec::new()) };
    save(&target, failed.clone(), None).unwrap();
    assert_eq!(read_clip(&target, "c3").unwrap(), Some(failed));
    save(&target, analysis("c1", vec![shot(0, 300, &[])]), None).unwrap();
    assert_eq!(read_clip(&target, "c1").unwrap().unwrap().shots, [shot(0, 300, &[])]);
    assert_eq!(read_analysed(&target).unwrap(), ["c1", "c2", "c3"]);
}

#[test]
fn shots_with_a_gap_or_an_error_beside_shots_are_refused() {
    let (_dir, target) = project();
    assert!(save(&target, analysis("c1", vec![shot(0, 100, &[]), shot(120, 300, &[])]), None).is_err());
    let both = ClipShots { error: "x".into(), ..analysis("c1", vec![shot(0, 100, &[])]) };
    assert!(save(&target, both, None).is_err());
    assert_eq!(read_clip(&target, "c1").unwrap(), None, "nothing written");
}

#[test]
fn usable_parts_are_kept_and_an_earlier_table_without_them_is_analysed_again() {
    let (dir, target) = project();
    // An analysis of the earlier form (without usable parts).
    Connection::open(dir.path().join("project.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE clip_shots (clip_id TEXT PRIMARY KEY, media_uri TEXT NOT NULL, pictures INTEGER NOT NULL,
               shots_json TEXT NOT NULL, error TEXT NOT NULL, took_ms INTEGER NOT NULL, made_at_utc TEXT NOT NULL);
             CREATE VIEW public_clip_shots AS SELECT * FROM clip_shots;
             INSERT INTO clip_shots VALUES ('old', 'qnc://local/source/s/file/o.mxf', 3, '[]', '', 1, 'x');",
        )
        .unwrap();
    assert_eq!(read_clip(&target, "old").unwrap(), None, "an earlier form is not read");
    let part = PartRecord { from: 10, to: 200, camera: "steady".into(), key_frame: 50 };
    save(&target, ClipShots { parts: vec![part.clone()], ..analysis("c1", vec![shot(0, 300, &[])]) }, None).unwrap();
    assert_eq!(read_analysed(&target).unwrap(), ["c1"], "the earlier row is gone, its clip is analysed again");
    assert_eq!(read_clip(&target, "c1").unwrap().unwrap().parts, [part]);
    let jerky = PartRecord { camera: "jerk".into(), ..PartRecord { from: 10, to: 200, camera: String::new(), key_frame: 50 } };
    assert!(save(&target, ClipShots { parts: vec![jerky], ..analysis("c2", Vec::new()) }, None).is_err());
}

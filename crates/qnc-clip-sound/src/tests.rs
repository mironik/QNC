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

fn sound(clip: &str, a2_channel: u16) -> ClipSound {
    ClipSound {
        clip_id: clip.into(),
        a1: LaneSound { channel: 0, off: false, level_db: 0 },
        a2: LaneSound { channel: a2_channel, off: false, level_db: -3 },
    }
}

fn wait(sounds: &mut ClipSounds, target: &ProjectDbTarget, clip: &str) -> Option<ClipSound> {
    for _ in 0..500 {
        if let Some(sound) = sounds.for_clip(Some(target), Some(clip)) {
            return sound;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("not read")
}

#[test]
fn the_sound_kept_for_a_clip_is_read_back_and_a_clip_without_one_has_none() {
    let (_dir, target) = project();
    assert_eq!(read(&target, "clip-a").unwrap(), None, "no table yet: no sound");
    let mut sounds = ClipSounds::new();
    assert_eq!(wait(&mut sounds, &target, "clip-a"), None);
    sounds.keep(&target, sound("clip-a", 2));
    assert_eq!(sounds.for_clip(Some(&target), Some("clip-a")), Some(Some(sound("clip-a", 2))), "kept at once in the form");
    for _ in 0..500 {
        if read(&target, "clip-a").unwrap().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(read(&target, "clip-a").unwrap(), Some(sound("clip-a", 2)), "in the database");
    let mut other_form = ClipSounds::new();
    assert_eq!(wait(&mut other_form, &target, "clip-a"), Some(sound("clip-a", 2)), "another form hears it the same");
    assert_eq!(wait(&mut other_form, &target, "clip-b"), None);
}

#[test]
fn a_bad_sound_is_refused() {
    let (_dir, target) = project();
    let writer = ProjectDbWriter::start(target, vec![ClipSoundModule::factory()]).unwrap();
    let mut bad = sound("clip-a", 1);
    bad.a1.level_db = 40;
    assert!(writer.call(MODULE_ID, encode(&Operation::Set(bad)).unwrap()).is_err());
}

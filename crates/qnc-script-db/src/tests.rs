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

fn sot(clip: &str) -> Block {
    Block {
        kind: BlockKind::Sot,
        clip_id: clip.into(),
        clip_name: clip.into(),
        from_frame: 100,
        to_frame: 350,
        start_tc: "10:00:02:00".into(),
        end_tc: "10:00:06:49".into(),
        length: "5:00".into(),
        seconds: 5.0,
        ..Block::off("rekao je")
    }
}

fn texts(target: &ProjectDbTarget) -> Vec<String> {
    read(target).unwrap().into_iter().map(|b| b.text).collect()
}

#[test]
fn blocks_are_kept_in_reading_order_and_added_after_the_chosen_one() {
    let (_dir, target) = project();
    assert!(read(&target).unwrap().is_empty(), "a project without a script reads an empty one");
    let first = add(&target, None, Block::off("prvi")).unwrap();
    add(&target, None, Block::off("treci")).unwrap();
    add(&target, Some(first), sot("2002")).unwrap();
    assert_eq!(texts(&target), ["prvi", "rekao je", "treci"]);
    let blocks = read(&target).unwrap();
    assert_eq!(blocks[1].kind, BlockKind::Sot);
    assert_eq!((blocks[1].from_frame, blocks[1].to_frame, blocks[1].seconds), (100, 350, 5.0));
}

#[test]
fn a_block_moves_changes_its_text_and_leaves() {
    let (_dir, target) = project();
    let a = add(&target, None, Block::off("a")).unwrap();
    let b = add(&target, None, Block::off("b")).unwrap();
    move_block(&target, b, true).unwrap();
    assert_eq!(texts(&target), ["b", "a"]);
    move_block(&target, b, true).unwrap(); // already first: stays
    assert_eq!(texts(&target), ["b", "a"]);
    set_text(&target, a, "a ispravljen".into()).unwrap();
    remove(&target, b).unwrap();
    assert_eq!(texts(&target), ["a ispravljen"]);
    add(&target, None, Block::off("c")).unwrap();
    assert_eq!(texts(&target), ["a ispravljen", "c"]);
}

#[test]
fn an_empty_off_or_a_statement_without_a_range_is_refused() {
    let (_dir, target) = project();
    assert!(add(&target, None, Block::off("  ")).is_err());
    let mut bad = sot("2002");
    bad.to_frame = bad.from_frame;
    assert!(add(&target, None, bad).is_err());
    let id = add(&target, None, Block::off("x")).unwrap();
    assert!(set_text(&target, id, String::new()).is_err());
    assert_eq!(texts(&target), ["x"]);
}

#[test]
fn the_lead_in_and_tag_are_text_blocks_and_only_an_off_takes_a_recording() {
    let (_dir, target) = project();
    let intro = add(&target, None, Block::written(BlockKind::Intro, "Najava spikera.")).unwrap();
    let off = add(&target, None, Block::off("Prvi off.")).unwrap();
    add(&target, None, Block::written(BlockKind::Outro, "Odjava.")).unwrap();
    assert!(set_voice(&target, intro, "qnc://x".into(), 3.0, "18:00:00:00".into()).is_err());
    set_voice(&target, off, "qnc://local/source/project-p1/file/products/voice/off.mxf".into(), 4.5, "18:01:02:03".into()).unwrap();
    let blocks = read(&target).unwrap();
    assert_eq!(blocks.iter().map(|b| b.kind).collect::<Vec<_>>(), [BlockKind::Intro, BlockKind::Off, BlockKind::Outro]);
    assert_eq!((blocks[1].voice_seconds, blocks[1].voice_tc.as_str()), (4.5, "18:01:02:03"));
}

#[test]
fn a_script_of_the_first_day_keeps_its_blocks_when_its_table_is_brought_up() {
    let (dir, target) = project();
    Connection::open(dir.path().join("project.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE script_blocks (block_id INTEGER PRIMARY KEY AUTOINCREMENT, position INTEGER NOT NULL, kind TEXT NOT NULL CHECK (kind IN ('off', 'sot')),
                text TEXT NOT NULL, clip_id TEXT NOT NULL, clip_name TEXT NOT NULL, from_frame INTEGER NOT NULL, to_frame INTEGER NOT NULL, start_tc TEXT NOT NULL,
                end_tc TEXT NOT NULL, length TEXT NOT NULL, seconds REAL NOT NULL, updated_at_utc TEXT NOT NULL);
             CREATE VIEW public_script_blocks AS SELECT block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, updated_at_utc FROM script_blocks;
             INSERT INTO script_blocks VALUES (1, 0, 'off', 'stari off', '', '', 0, 0, '', '', '', 0, 'x');",
        )
        .unwrap();
    assert_eq!(texts(&target), ["stari off"], "read before the owner brings it up");
    add(&target, None, Block::written(BlockKind::Intro, "najava")).unwrap();
    assert_eq!(texts(&target), ["stari off", "najava"]);
}

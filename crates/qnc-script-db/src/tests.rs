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

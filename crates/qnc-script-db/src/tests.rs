use super::*;
use rusqlite::Connection;
use std::collections::BTreeMap;

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

/// A prepared profile QNC brings.
fn prepared(id: &str) -> ScriptProfile {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../catalogs/script-profiles/{id}.json"));
    let value = qnc_ui_text::resolve_contract(&std::fs::read_to_string(file).unwrap()).unwrap();
    serde_json::from_value(value).unwrap()
}

/// A project whose script is written with the general TV profile.
fn written() -> (tempfile::TempDir, ProjectDbTarget) {
    let (dir, target) = project();
    set_profile(&target, prepared("general_tv")).unwrap();
    (dir, target)
}

fn off(text: &str) -> Block {
    Block::written("off", text)
}

fn sot(clip: &str) -> Block {
    Block {
        clip_id: clip.into(),
        clip_name: clip.into(),
        from_frame: 100,
        to_frame: 350,
        start_tc: "10:00:02:00".into(),
        end_tc: "10:00:06:49".into(),
        length: "5:00".into(),
        seconds: 5.0,
        ..Block::written("sot", "rekao je")
    }
}

fn texts(target: &ProjectDbTarget) -> Vec<String> {
    read(target).unwrap().into_iter().map(|b| b.text).collect()
}

#[test]
fn blocks_are_kept_in_reading_order_and_added_after_the_chosen_one() {
    let (_dir, target) = written();
    assert!(read(&target).unwrap().is_empty(), "a project without a script reads an empty one");
    let first = add(&target, None, off("prvi")).unwrap();
    add(&target, None, off("treci")).unwrap();
    add(&target, Some(first), sot("2002")).unwrap();
    assert_eq!(texts(&target), ["prvi", "rekao je", "treci"]);
    let blocks = read(&target).unwrap();
    assert_eq!(blocks[1].kind, "sot");
    assert_eq!((blocks[1].from_frame, blocks[1].to_frame, blocks[1].seconds), (100, 350, 5.0));
}

#[test]
fn a_block_moves_changes_its_text_and_leaves() {
    let (_dir, target) = written();
    let a = add(&target, None, off("a")).unwrap();
    let b = add(&target, None, off("b")).unwrap();
    move_block(&target, b, true).unwrap();
    assert_eq!(texts(&target), ["b", "a"]);
    move_block(&target, b, true).unwrap(); // already first: stays
    assert_eq!(texts(&target), ["b", "a"]);
    set_text(&target, a, "a ispravljen".into()).unwrap();
    remove(&target, b).unwrap();
    assert_eq!(texts(&target), ["a ispravljen"]);
    add(&target, None, off("c")).unwrap();
    assert_eq!(texts(&target), ["a ispravljen", "c"]);
}

#[test]
fn empty_text_or_a_clip_block_without_a_range_is_refused() {
    let (_dir, target) = written();
    assert!(add(&target, None, off("  ")).is_err());
    let mut bad = sot("2002");
    bad.to_frame = bad.from_frame;
    assert!(add(&target, None, bad).is_err());
    let mut standup = sot("2002");
    standup.kind = "standup".into();
    standup.to_frame = 0;
    assert!(add(&target, None, standup).is_err(), "every clip kind needs a range, not only SOT");
    let id = add(&target, None, off("x")).unwrap();
    assert!(set_text(&target, id, String::new()).is_err());
    assert_eq!(texts(&target), ["x"]);
}

#[test]
fn only_a_block_of_a_recorded_kind_takes_a_recording() {
    let (_dir, target) = written();
    let intro = add(&target, None, Block::written("intro", "Najava spikera.")).unwrap();
    let off_block = add(&target, None, off("Prvi off.")).unwrap();
    add(&target, None, Block::written("outro", "Odjava.")).unwrap();
    assert!(set_voice(&target, intro, "qnc://x".into(), 3.0, "18:00:00:00".into()).is_err());
    set_voice(&target, off_block, "qnc://local/source/project-p1/file/products/voice/off.mxf".into(), 4.5, "18:01:02:03".into()).unwrap();
    let blocks = read(&target).unwrap();
    assert_eq!(blocks.iter().map(|b| b.kind.as_str()).collect::<Vec<_>>(), ["intro", "off", "outro"]);
    assert_eq!((blocks[1].voice_seconds, blocks[1].voice_tc.as_str()), (4.5, "18:01:02:03"));
}

#[test]
fn the_project_keeps_its_profile_and_writes_only_its_kinds() {
    let (_dir, target) = project();
    assert_eq!(read_profile(&target).unwrap(), None);
    assert!(add(&target, None, off("bez profila")).is_err(), "a script without a profile is not written");
    let inews = prepared("inews_mos");
    set_profile(&target, inews.clone()).unwrap();
    assert_eq!(read_profile(&target).unwrap(), Some(inews));
    let mut cg = Block::written("cg", "Ivan Horvat, gradonacelnik");
    add(&target, None, cg.clone()).unwrap();
    cg.kind = "lower_third".into();
    assert!(add(&target, None, cg).is_err(), "a kind the profile does not have");
    // A profile without a kind the script has is refused; the one kept stays.
    assert!(set_profile(&target, prepared("general_tv")).is_err(), "general TV has no cg");
    assert_eq!(read_profile(&target).unwrap().unwrap().profile_id, "inews_mos");
}

#[test]
fn a_script_written_before_profiles_keeps_its_blocks_and_recordings() {
    let (dir, target) = project();
    Connection::open(dir.path().join("project.db"))
        .unwrap()
        .execute_batch(
            "CREATE TABLE script_blocks (block_id INTEGER PRIMARY KEY AUTOINCREMENT, position INTEGER NOT NULL, kind TEXT NOT NULL CHECK (kind IN ('off', 'sot', 'intro', 'outro')),
                text TEXT NOT NULL, clip_id TEXT NOT NULL, clip_name TEXT NOT NULL, from_frame INTEGER NOT NULL, to_frame INTEGER NOT NULL, start_tc TEXT NOT NULL,
                end_tc TEXT NOT NULL, length TEXT NOT NULL, seconds REAL NOT NULL, voice_uri TEXT NOT NULL DEFAULT '', voice_seconds REAL NOT NULL DEFAULT 0,
                voice_tc TEXT NOT NULL DEFAULT '', updated_at_utc TEXT NOT NULL);
             CREATE VIEW public_script_blocks AS SELECT block_id, position, kind, text, clip_id, clip_name, from_frame, to_frame, start_tc, end_tc, length, seconds, voice_uri, voice_seconds, voice_tc, updated_at_utc FROM script_blocks;
             INSERT INTO script_blocks VALUES (1, 0, 'off', 'snimljen off', '', '', 0, 0, '', '', '', 0, 'qnc://v.mxf', 3.5, '18:00:00:00', 'x');",
        )
        .unwrap();
    assert_eq!(texts(&target), ["snimljen off"], "read before the owner brings it up");
    set_profile(&target, prepared("general_tv")).unwrap();
    add(&target, None, Block::written("standup", "x")).unwrap_err();
    add(&target, None, Block::written("intro", "najava")).unwrap();
    let blocks = read(&target).unwrap();
    assert_eq!(texts(&target), ["snimljen off", "najava"]);
    assert_eq!((blocks[0].voice_uri.as_str(), blocks[0].voice_seconds), ("qnc://v.mxf", 3.5));
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
    set_profile(&target, prepared("general_tv")).unwrap();
    add(&target, None, Block::written("intro", "najava")).unwrap();
    assert_eq!(texts(&target), ["stari off", "najava"]);
}

#[test]
fn a_block_keeps_only_the_fields_of_its_kind_and_a_choice_only_one_of_its_choices() {
    let (_dir, target) = project();
    set_profile(&target, prepared("inews_mos")).unwrap();
    let id = add(&target, None, sot("2002")).unwrap();
    let mut fields = BTreeMap::from([("caption_name".to_string(), "Ivan Horvat".to_string()), ("caption_role".to_string(), "  ".to_string())]);
    set_fields(&target, id, fields.clone()).unwrap();
    assert_eq!(read(&target).unwrap()[0].fields, BTreeMap::from([("caption_name".to_string(), "Ivan Horvat".to_string())]), "an empty value is none");
    fields.insert("line1".into(), "x".into());
    assert!(set_fields(&target, id, fields).is_err(), "a SOT has no line1");
    let cg = add(&target, None, Block::written("cg", "Potpis")).unwrap();
    assert!(set_fields(&target, cg, BTreeMap::from([("cg_template".to_string(), "4line".to_string())])).is_err());
    set_fields(&target, cg, BTreeMap::from([("cg_template".to_string(), "locator".to_string()), ("line1".to_string(), "Zagreb".to_string())])).unwrap();
    let blocks = read(&target).unwrap();
    assert_eq!(blocks[1].fields.get("line1").map(String::as_str), Some("Zagreb"));
    let profile = read_profile(&target).unwrap().unwrap();
    assert_eq!(profile.cue_label_with("cg", 1, &blocks[1].fields), "*CG locator");
}

fn piece(clip: &str, from: u64, to: u64, text: &str) -> Piece {
    Piece { clip_id: clip.into(), clip_name: clip.into(), from_frame: from, to_frame: to, start_tc: format!("tc{from}"), end_tc: format!("tc{to}"), length: "1:00".into(), seconds: 1.0, text: text.into() }
}

#[test]
fn a_subsegment_keeps_its_parts_their_words_one_under_another() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "a drugi dio")).unwrap();
    add_piece(&target, id, piece("2679", 10, 60, "i treci")).unwrap();
    let block = read(&target).unwrap().remove(0);
    assert_eq!(block.pieces.len(), 3, "the block's own range is its first part");
    assert_eq!(block.text, "rekao je\na drugi dio\ni treci");
    assert_eq!(block.seconds, 7.0);
    assert_eq!((block.clip_id.as_str(), block.from_frame, block.start_tc.as_str(), block.end_tc.as_str()), ("2002", 100, "10:00:02:00", "tc60"));
    assert!(add_piece(&target, id, piece("2002", 50, 50, "")).is_err(), "a part without a range is refused");
    let text = add(&target, None, off("tekst")).unwrap();
    assert!(add_piece(&target, text, piece("2002", 1, 2, "")).is_err(), "a text block takes no parts");
    let new = add(&target, None, Block::of_pieces("sot", vec![piece("3396", 1, 51, "nova izjava")])).unwrap();
    let block = read(&target).unwrap().into_iter().find(|b| b.id == new).unwrap();
    assert_eq!((block.pieces.len(), block.text.as_str(), block.from_frame), (1, "nova izjava", 1));
}

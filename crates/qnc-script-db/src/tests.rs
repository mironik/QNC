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

fn take(number: u16, seconds: f64, tc: &str, text: &str) -> VoiceTake {
    VoiceTake { take: number, wav_uri: format!("qnc://local/source/project-p1/file/products/voice/P_OFF1_T{number}.wav"), seconds, tc: tc.into(), text: text.into(), heard: None }
}

#[test]
fn a_marker_stays_with_its_word_goes_with_it_and_is_one_step_of_undo() {
    let (_dir, target) = written();
    let id = add(&target, None, off("prva druga treca cetvrta")).unwrap();
    assert!(set_marks(&target, id, vec![0]).is_err(), "not before the first word");
    assert!(set_marks(&target, id, vec![4]).is_err(), "not after the last");
    set_marks(&target, id, vec![3, 1]).unwrap();
    assert_eq!(read(&target).unwrap()[0].marks, [1, 3]);
    set_text(&target, id, "nova prva druga cetvrta".into()).unwrap();
    assert_eq!(read(&target).unwrap()[0].marks, [2, 3], "before druga and cetvrta, which moved");
    set_text(&target, id, "nova prva cetvrta".into()).unwrap();
    assert_eq!(read(&target).unwrap()[0].marks, [2], "druga deleted, its marker with it");
    undo(&target).unwrap();
    assert_eq!(read(&target).unwrap()[0].marks, [2, 3]);
}

#[test]
fn the_words_heard_are_kept_with_their_take_and_no_step_of_undo() {
    let (_dir, target) = written();
    let id = add(&target, None, off("Prvi off.")).unwrap();
    let clip = "qnc://local/source/project-p1/file/products/voice/P_OFF1.mxf".to_string();
    add_voice_take(&target, id, "P_OFF1".into(), clip, take(1, 2.0, "18:00:00:00", "Prvi off.")).unwrap();
    assert_eq!(read_voice(&target, id).unwrap().unwrap().takes[0].heard, None);
    let words = vec![HeardWord { text: "prvi".into(), start: 0.1, end: 0.4 }];
    set_take_heard(&target, id, 1, words.clone()).unwrap();
    assert_eq!(read_voice(&target, id).unwrap().unwrap().takes[0].heard, Some(words.clone()));
    assert!(set_take_heard(&target, id, 2, words).is_err(), "only a take the record has");
}

#[test]
fn every_take_of_an_off_is_one_more_channel_of_one_record_and_the_last_is_used() {
    let (_dir, target) = written();
    let intro = add(&target, None, Block::written("intro", "Najava.")).unwrap();
    let off_block = add(&target, None, off("Prvi off.")).unwrap();
    let clip = "qnc://local/source/project-p1/file/products/voice/P_OFF1.mxf".to_string();
    assert!(add_voice_take(&target, intro, "P_INTRO".into(), clip.clone(), take(1, 3.0, "18:00:00:00", "Najava.")).is_err(), "only a recorded kind");
    assert!(add_voice_take(&target, off_block, "P_OFF1".into(), clip.clone(), take(2, 3.0, "18:00:00:00", "x")).is_err(), "takes come in order");
    add_voice_take(&target, off_block, "P_OFF1".into(), clip.clone(), take(1, 4.5, "18:01:02:03", "Prvi off.")).unwrap();
    add_voice_take(&target, off_block, "P_OFF1".into(), clip.clone(), take(2, 4.0, "18:05:00:00", "Prvi off, ispravljen.")).unwrap();
    assert!(add_voice_take(&target, off_block, "P_OTHER".into(), clip.clone(), take(3, 4.0, "18:06:00:00", "x")).is_err(), "the name stays");
    let voice = read_voice(&target, off_block).unwrap().unwrap();
    assert_eq!((voice.name.as_str(), voice.chosen, voice.takes.len()), ("P_OFF1", 2, 2));
    assert_eq!(voice.chosen_take().unwrap().text, "Prvi off, ispravljen.");
    let block = read(&target).unwrap().into_iter().find(|b| b.id == off_block).unwrap();
    assert_eq!((block.voice_uri.as_str(), block.voice_seconds, block.voice_tc.as_str()), (clip.as_str(), 4.0, "18:01:02:03"), "the clip starts at its first take, its length is the take used");
    set_voice_chosen(&target, off_block, 1).unwrap();
    assert!(set_voice_chosen(&target, off_block, 3).is_err(), "only a take the record has");
    assert_eq!(read_voice(&target, off_block).unwrap().unwrap().chosen, 1);
    let block = read(&target).unwrap().into_iter().find(|b| b.id == off_block).unwrap();
    assert_eq!(block.voice_seconds, 4.5, "the length is the chosen take's");
    remove(&target, off_block).unwrap();
    assert!(read_voices(&target).unwrap().is_empty(), "a removed block takes its record");
}

#[test]
fn a_record_has_at_most_eight_takes() {
    let (_dir, target) = written();
    let off_block = add(&target, None, off("Off.")).unwrap();
    let clip = "qnc://local/source/project-p1/file/products/voice/P_OFF1.mxf".to_string();
    for number in 1..=8 {
        add_voice_take(&target, off_block, "P_OFF1".into(), clip.clone(), take(number, 2.0, "18:00:00:00", "Off.")).unwrap();
    }
    assert!(add_voice_take(&target, off_block, "P_OFF1".into(), clip, take(9, 2.0, "18:00:00:00", "Off.")).is_err());
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
    Piece { clip_id: clip.into(), clip_name: clip.into(), from_frame: from, to_frame: to, start_tc: format!("tc{from}"), end_tc: format!("tc{to}"), length: "1:00".into(), seconds: 1.0, text: text.into(), gap_before: 0, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false, a1_lead_frames: 0, a1_tail_frames: 0, v_lead_frames: 0, v_tail_frames: 0, level_keys: Vec::new(), smooth_frames: 0, smooth_clip: String::new() }
}

#[test]
fn a_smooth_cut_is_the_joins_and_its_clip_is_no_step_of_undo() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "drugi")).unwrap();
    add_piece(&target, id, piece("2679", 10, 60, "treci")).unwrap();
    set_smooth(&target, id, 1, 8).unwrap();
    set_smooth_clip(&target, id, 1, "smooth-1".into()).unwrap();
    let pieces = read(&target).unwrap().remove(0).pieces;
    assert_eq!((pieces[1].smooth_frames, pieces[1].smooth_clip.as_str(), pieces[2].smooth_frames), (8, "smooth-1", 0));
    assert!(set_smooth(&target, id, 0, 8).is_err(), "the first part has no join");
    // A trim leaves the join its smooth cut, a move leaves it between the same places.
    let mut trimmed = pieces[1].clone();
    trimmed.to_frame = 540;
    set_piece(&target, id, 1, trimmed).unwrap();
    assert_eq!(read(&target).unwrap().remove(0).pieces[1].smooth_frames, 8);
    move_piece(&target, id, 2, true).unwrap();
    let pieces = read(&target).unwrap().remove(0).pieces;
    assert_eq!((pieces[1].clip_id.as_str(), pieces[1].smooth_frames), ("2679", 8));
    // The clip the background made is not a step: undo takes back the last change of the story.
    undo(&target).unwrap();
    let pieces = read(&target).unwrap().remove(0).pieces;
    assert_eq!(pieces[1].clip_id.as_str(), "2002", "the move is taken back");
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

#[test]
fn a_join_keeps_its_pause_and_cut_the_first_part_has_none() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "a drugi dio")).unwrap();
    set_join(&target, id, 1, 12, true).unwrap();
    let block = read(&target).unwrap().remove(0);
    assert_eq!((block.pieces[1].gap_before, block.pieces[1].cut), (12, true));
    assert_eq!(block.text, "rekao je
a drugi dio", "the words stay");
    assert!(set_join(&target, id, 0, 5, false).is_err(), "the first part has no join");
    assert!(set_join(&target, id, 2, 5, false).is_err(), "no such part");
}

#[test]
fn a_join_keeps_its_j_and_l_through_a_trim_and_a_move_and_the_first_part_has_none() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "drugi")).unwrap();
    add_piece(&target, id, piece("2679", 10, 60, "treci")).unwrap();
    set_join_sound(&target, id, 1, 4, 6).unwrap();
    let join = |target: &ProjectDbTarget| read(target).unwrap()[0].pieces.iter().map(|p| (p.a1_lead_frames, p.a1_tail_frames)).collect::<Vec<_>>();
    assert_eq!(join(&target), [(0, 0), (4, 6), (0, 0)]);
    // A trimmed edge keeps the J and L of its join.
    set_piece(&target, id, 1, piece("2002", 505, 550, "drugi")).unwrap();
    assert_eq!(join(&target)[1], (4, 6));
    // They stay with the join's place when the parts change places.
    move_piece(&target, id, 2, true).unwrap();
    assert_eq!(join(&target), [(0, 0), (4, 6), (0, 0)]);
    assert!(set_join_sound(&target, id, 0, 1, 1).is_err(), "the first part has no join");
}

#[test]
fn a_join_keeps_the_j_and_l_of_its_picture_apart_from_its_sound() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "drugi")).unwrap();
    set_join_sound(&target, id, 1, 4, 6).unwrap();
    set_join_picture(&target, id, 1, 3, 0).unwrap();
    let part = read(&target).unwrap()[0].pieces[1].clone();
    assert_eq!((part.v_lead_frames, part.v_tail_frames, part.a1_lead_frames, part.a1_tail_frames), (3, 0, 4, 6), "the picture and the sound of the join apart");
    set_piece(&target, id, 1, piece("2002", 505, 550, "drugi")).unwrap();
    assert_eq!(read(&target).unwrap()[0].pieces[1].v_lead_frames, 3, "a trimmed edge keeps them");
    assert!(set_join_picture(&target, id, 0, 1, 1).is_err(), "the first part has no join");
}

#[test]
fn a_part_moves_past_its_neighbour_and_leaves_the_joins_stay_in_their_places() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "drugi")).unwrap();
    add_piece(&target, id, piece("2679", 10, 60, "treci")).unwrap();
    set_join(&target, id, 1, 8, true).unwrap();
    set_join(&target, id, 2, 3, false).unwrap();
    move_piece(&target, id, 2, true).unwrap();
    let block = read(&target).unwrap().remove(0);
    let parts: Vec<_> = block.pieces.iter().map(|p| (p.from_frame, p.gap_before, p.cut)).collect();
    assert_eq!(parts, [(100, 0, false), (10, 8, true), (500, 3, false)], "the pauses stay with their places");
    assert_eq!(block.text, "rekao je\ntreci\ndrugi");
    move_piece(&target, id, 0, false).unwrap();
    assert_eq!(read(&target).unwrap()[0].pieces[0].from_frame, 10, "the first part is another");
    assert!(move_piece(&target, id, 0, true).is_err(), "nothing before the first part");
    assert!(move_piece(&target, id, 2, false).is_err(), "nothing after the last part");
    remove_piece(&target, id, 0).unwrap();
    let block = read(&target).unwrap().remove(0);
    assert_eq!((block.pieces.len(), block.pieces[0].gap_before, block.pieces[0].cut, block.from_frame), (2, 0, false, 100), "the new first part has no join");
    remove_piece(&target, id, 1).unwrap();
    assert!(remove_piece(&target, id, 0).is_err(), "the last part stays: the block is removed instead");
    assert!(remove_piece(&target, id, 5).is_err());
}

#[test]
fn words_cut_out_of_a_part_leave_what_was_before_and_after_them_joined_by_a_cut() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "drugi")).unwrap();
    set_join(&target, id, 1, 6, false).unwrap();
    // A filler in the middle of the second part: it becomes two parts, the first keeps the join.
    cut_out(&target, id, 1, Some(piece("2002", 500, 520, "drugi")), Some(piece("2002", 530, 550, "dio"))).unwrap();
    let parts: Vec<_> = read(&target).unwrap()[0].pieces.iter().map(|p| (p.from_frame, p.to_frame, p.gap_before, p.cut)).collect();
    assert_eq!(parts, [(100, 350, 0, false), (500, 520, 6, false), (530, 550, 0, true)], "the new join is a cut, no pause");
    // Words at the start of a statement of one range: only what was after them stays.
    let one = add(&target, None, sot("2679")).unwrap();
    cut_out(&target, one, 0, None, Some(piece("2679", 140, 350, "ostatak"))).unwrap();
    let block = read(&target).unwrap().into_iter().find(|b| b.id == one).unwrap();
    assert_eq!((block.pieces.len(), block.from_frame), (1, 140));
    assert!(cut_out(&target, id, 2, Some(piece("2002", 520, 560, "")), None).is_err(), "outside the part's range");
    assert!(cut_out(&target, id, 2, Some(piece("2679", 530, 540, "")), None).is_err(), "another clip");
    cut_out(&target, id, 2, None, None).unwrap();
    assert_eq!(read(&target).unwrap()[0].pieces.len(), 2, "every word cut: the part leaves");
    assert!(cut_out(&target, one, 0, None, None).is_err(), "the last part stays: the block is removed instead");
}

#[test]
fn a_trimmed_part_keeps_its_clip_pause_and_join() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "a drugi dio")).unwrap();
    set_join(&target, id, 1, 6, true).unwrap();
    set_piece(&target, id, 1, piece("2002", 490, 545, "a drugi dio")).unwrap();
    let block = read(&target).unwrap().remove(0);
    let part = &block.pieces[1];
    assert_eq!((part.from_frame, part.to_frame, part.gap_before, part.cut), (490, 545, 6, true));
    assert!(set_piece(&target, id, 1, piece("2679", 1, 9, "")).is_err(), "a part keeps its clip");
    assert!(set_piece(&target, id, 1, piece("2002", 9, 9, "")).is_err(), "a part keeps at least one frame");
}

/// Undo and redo of the script (user 2026-11-06: "Backspace undo does not work"): every change
/// of the blocks is one step back, redo takes it again, a new change forgets redo; a refused
/// change is no step.
#[test]
fn every_change_of_the_script_is_one_undo_step_and_redo_takes_it_again() {
    let (_dir, target) = written();
    assert!(!undo(&target).unwrap(), "nothing to undo yet");
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "drugi")).unwrap();
    let parts = |target: &qnc_db_broker::ProjectDbTarget| read(target).unwrap().first().map_or(0, |block| block.pieces.len());
    cut_out(&target, id, 1, Some(piece("2002", 500, 520, "drugi")), Some(piece("2002", 530, 550, "dio"))).unwrap();
    assert_eq!(parts(&target), 3);
    assert!(cut_out(&target, id, 2, Some(piece("2002", 520, 560, "")), None).is_err());
    assert!(undo(&target).unwrap(), "the cut back");
    assert_eq!(parts(&target), 2, "the words are there again");
    assert!(redo(&target).unwrap());
    assert_eq!(parts(&target), 3, "and cut again");
    assert!(undo(&target).unwrap() && undo(&target).unwrap());
    assert_eq!((read(&target).unwrap().len(), parts(&target)), (1, 0), "the part added is taken back too: one range again");
    assert!(undo(&target).unwrap());
    assert!(read(&target).unwrap().is_empty(), "and the block");
    assert!(!undo(&target).unwrap());
    assert!(redo(&target).unwrap());
    assert_eq!(read(&target).unwrap()[0].id, id, "the block comes back with its id");
    add_piece(&target, id, piece("2002", 600, 650, "treći")).unwrap();
    assert!(!redo(&target).unwrap(), "a new change forgets redo");
}

/// A roll, a slot moved, several slots' sound (audit 2026-11-08 #15): one write and one step
/// of undo; one change refused writes none of them.
#[test]
fn changes_of_parts_together_are_one_write_and_one_undo_step() {
    let (_dir, target) = written();
    let id = add(&target, None, sot("2002")).unwrap();
    add_piece(&target, id, piece("2002", 500, 550, "drugi")).unwrap();
    let ranges = |target: &ProjectDbTarget| read(target).unwrap()[0].pieces.iter().map(|p| (p.from_frame, p.to_frame)).collect::<Vec<_>>();
    let roll = vec![
        Operation::SetPiece { id, index: 0, piece: piece("2002", 100, 360, "rekao je") },
        Operation::SetPiece { id, index: 1, piece: piece("2002", 510, 550, "drugi") },
    ];
    together(&target, roll).unwrap();
    assert_eq!(ranges(&target), [(100, 360), (510, 550)]);
    assert!(undo(&target).unwrap());
    assert_eq!(ranges(&target), [(100, 350), (500, 550)], "one undo takes both back");
    let refused = vec![
        Operation::SetPiece { id, index: 0, piece: piece("2002", 100, 360, "rekao je") },
        Operation::SetPiece { id, index: 1, piece: piece("2679", 1, 9, "") },
    ];
    assert!(together(&target, refused).is_err());
    assert_eq!(ranges(&target), [(100, 350), (500, 550)], "the first is not written either");
    assert!(together(&target, vec![Operation::Remove { id }, Operation::Remove { id }]).is_err(), "only changes of parts");
}

/// A step of undo kept before the table had a column (audit 2026-11-08 #17): undo puts the
/// blocks back with the column's default, it does not fail.
#[test]
fn an_undo_step_kept_before_a_column_was_added_puts_the_blocks_back() {
    let (dir, target) = written();
    let id = add(&target, None, off("staro")).unwrap();
    set_text(&target, id, "novo".into()).unwrap();
    // The step as an older table kept it: without the columns added since.
    let conn = Connection::open(dir.path().join("project.db")).unwrap();
    let rows: String = conn.query_row("SELECT rows_json FROM script_undo ORDER BY step DESC LIMIT 1", [], |r| r.get(0)).unwrap();
    let mut old: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(&rows).unwrap();
    for row in &mut old {
        row.remove("pieces_json");
        row.remove("fields_json");
    }
    conn.execute("UPDATE script_undo SET rows_json = ?1 WHERE step = (SELECT MAX(step) FROM script_undo)", [serde_json::to_string(&old).unwrap()]).unwrap();
    drop(conn);
    assert!(undo(&target).unwrap());
    assert_eq!(texts(&target), ["staro"]);
}

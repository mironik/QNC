use super::*;

fn general_tv() -> ScriptProfile {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/script-profiles/general_tv.json");
    serde_json::from_value(qnc_ui_text::resolve_contract(&std::fs::read_to_string(file).unwrap()).unwrap()).unwrap()
}

fn sot(text: &str) -> Block {
    Block { clip_id: "c".into(), clip_name: "Mironik 2002".into(), start_tc: "10:00:02:00".into(), end_tc: "10:00:06:49".into(), length: "5:00".into(), seconds: 5.0, ..Block::written("sot", text) }
}

#[test]
fn a_voice_record_says_whether_it_is_there_and_was_recorded_with_the_text() {
    let take = |take: u16, text: &str| VoiceTake { take, wav_uri: format!("qnc://p/T{take}.wav"), seconds: 2.0, tc: "21:00:00:00".into(), text: text.into() };
    let voice = Voice { block_id: 1, name: "P_OFF1".into(), clip_uri: "qnc://p/P_OFF1.mxf".into(), chosen: 2, takes: vec![take(1, "Stari tekst."), take(2, "Novi tekst.")] };
    assert_eq!(VoiceRow::of(&voice, true, "Novi tekst. ").status, VoiceStatus::Recorded, "the take used was recorded with this text");
    assert_eq!(VoiceRow::of(&voice, true, "Opet promijenjen.").status, VoiceStatus::TextChanged);
    assert_eq!(VoiceRow::of(&voice, false, "Novi tekst.").status, VoiceStatus::Missing);
    let first = Voice { chosen: 1, ..voice };
    assert_eq!(VoiceRow::of(&first, true, "Novi tekst.").status, VoiceStatus::TextChanged, "the status follows the take used");
}

#[test]
fn a_text_block_lasts_as_long_as_its_words_take_to_read() {
    assert_eq!(reading_seconds("jedan dva tri  cetiri pet", 2.5), 2.0);
    assert_eq!(reading_seconds("   ", 2.5), 0.0);
    assert_eq!(seconds_label(83.6), "1:24");
}

#[test]
fn marks_are_the_numbered_cues_of_the_profile_and_lengths_its_rules() {
    let profile = general_tv();
    let mut recorded = Block::written("off", "Drugi off koji je snimljen.");
    (recorded.voice_uri, recorded.voice_seconds) = ("qnc://v.mxf".into(), 7.0);
    let blocks = [Block::written("intro", "Najava."), Block::written("off", "Uvod u prilog."), sot("Rekao je."), recorded, Block::written("nat", "")];
    let rows = rows(&blocks, Some(&profile), 2.5);
    let marks: Vec<&str> = rows.iter().map(|row| row.mark.as_str()).collect();
    assert_eq!(marks, [profile.kind("intro").unwrap().short.as_str(), "VO 1", "SOT 1", "VO 2", "NAT"]);
    assert_eq!(rows.iter().map(|row| row.seconds).collect::<Vec<_>>(), [0.4, 1.2, 5.0, 7.0, 0.0]);
    assert!(rows[1].recorded && !rows[0].recorded && rows[2].is_clip());
}

#[test]
fn the_script_as_text_has_every_block_in_reading_order_with_its_mark() {
    let profile = general_tv();
    let blocks = [Block::written("off", "Uvod u prilog."), sot("Rekao je.")];
    let view = ScriptView { blocks: rows(&blocks, Some(&profile), 2.5), profile: Some(profile), ..ScriptView::default() };
    assert_eq!(view.text(), "VO 1 (0:01)\nUvod u prilog.\n\nSOT 1 Mironik 2002 10:00:02:00 - 10:00:06:49 (5:00)\nRekao je.");
    assert_eq!(view.text_kinds().iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["intro", "off", "outro"]);
    assert_eq!(view.clip_kinds().iter().map(|k| k.id.as_str()).collect::<Vec<_>>(), ["sot", "standup", "nat"]);
}

#[test]
fn the_fields_of_a_block_are_in_the_pasted_script_and_a_draft_is_shown_until_committed() {
    let profile = general_tv();
    let mut statement = sot("Rekao je.");
    statement.id = 4;
    statement.fields.insert("caption_name".into(), "Ivan Horvat".into());
    let name = profile.kind("sot").unwrap().fields[0].label.clone();
    let mut view = ScriptView { blocks: rows(&[statement], Some(&profile), 2.5), profile: Some(profile), ..ScriptView::default() };
    assert!(view.text().ends_with(&format!("Rekao je.\n{name}: Ivan Horvat")));
    view.field_drafts.insert((4, "caption_name".into()), "Ivan Horvat Mlađi".into());
    assert_eq!(view.field_value(4, "caption_name"), "Ivan Horvat Mlađi");
    assert_eq!(view.field_value(4, "caption_role"), "");
}

#[test]
fn the_story_has_the_first_and_last_words_of_its_blocks_in_its_length() {
    assert_eq!(in_cue("Unatoč lošem vremenu, više od petsto"), "Unatoč lošem vremenu,...");
    assert_eq!(out_cue("Ana Anić, Vijesti, Zagreb."), "Ana Anić, Vijesti, Zagreb.");
    assert_eq!(out_cue("Novi sastanak sutra u devet sati."), "...sutra u devet sati.");
    let profile = general_tv();
    let blocks = [Block::written("intro", "Najava spikera ne ulazi."), Block::written("off", "Prvi off priloga ovdje."), sot("Ne odustajemo od naših zahtjeva.")];
    let rows = rows(&blocks, Some(&profile), 2.5);
    assert_eq!(story_cues(&rows), ("Prvi off priloga...".to_string(), "...odustajemo od naših zahtjeva.".to_string()));
    let view = ScriptView { blocks: rows, profile: Some(profile), ..ScriptView::default() };
    let story = export_story(&view);
    assert_eq!(story.items[2].cues, Some(("Ne odustajemo od...".to_string(), "...odustajemo od naših zahtjeva.".to_string())), "a SOT shows its cues");
    assert_eq!(story.items[1].cues, None);
}

#[test]
fn a_statement_shows_the_words_of_its_range_with_their_timecodes() {
    use qnc_transcript_db::{ClipTranscript, Line, Word};
    let word = |from, to, text: &str| Word { from, to, text: text.into() };
    let line = |words: Vec<Word>| Line { from: words[0].from, to: words.last().unwrap().to, text: String::new(), words };
    let transcripts = vec![ClipTranscript {
        clip_id: "c".into(),
        from_frame: 0,
        to_frame: 1000,
        language: "hr".into(),
        engine: "test".into(),
        lines: vec![line(vec![word(90, 100, "prije"), word(100, 120, "Dobar"), word(120, 140, "dan.")]), line(vec![word(150, 170, "Kako"), word(170, 195, "ste?"), word(300, 320, "poslije")])],
    }];
    let words = crate::view::timed_words(&transcripts, &part("c", 100, 200, 0), |frame| format!("tc{frame}"));
    let shown: Vec<(&str, &str, bool)> = words.iter().map(|w| (w.text.as_str(), w.tc.as_str(), w.line)).collect();
    // Only the words of the part, each line of speech with its timecode in front.
    assert_eq!(shown, [("Dobar", "tc100", true), ("dan.", "tc120", false), ("Kako", "tc150", true), ("ste?", "tc170", false)]);
}

#[test]
fn a_part_slides_as_a_slot_its_neighbours_edges_or_pauses_give_way() {
    // Parts 0..50 of a, then b 100..150 straight after, then c 10..60 after a pause of 8.
    let mut pieces = vec![part("a", 0, 50, 0), part("b", 100, 150, 0), part("c", 10, 60, 8)];
    let length = |_: &str| 80;
    assert_eq!(crate::slide_range(&pieces, 1, length), Some((-49, 8)), "a keeps a frame and its clip; the pause after never goes below none");
    assert_eq!(crate::slide_range(&pieces, 0, length), None, "the first part does not move whole");
    assert_eq!(crate::slide_range(&pieces, 2, length), None, "nor the last");
    crate::slide_pieces(&mut pieces, 1, 5);
    let shown: Vec<_> = pieces.iter().map(|p| (p.from_frame, p.to_frame, p.gap_before)).collect();
    assert_eq!(shown, [(0, 55, 0), (100, 150, 0), (10, 60, 3)], "the part keeps its own range; a plays on, the pause after shrinks");
}

#[test]
fn the_sound_of_a_join_moves_as_a_storys_cut() {
    use crate::{JoinSide, JoinSound};
    let join = |lead, tail, side| JoinSound { index: 1, lead, tail, side, picture: false };
    let room = (-40, -40);
    // Both sides together: the sound cut (or the overlap between them) moves whole.
    assert_eq!(join(0, 0, JoinSide::Both).moved(3, room), join(-3, 3, JoinSide::Both), "the sound cut 3 frames after the picture cut");
    assert_eq!(join(4, 6, JoinSide::Both).moved(2, room), join(2, 8, JoinSide::Both));
    // One side alone, either way of the cut (user 2026-11-04): the first sound ending before the
    // cut and the second starting after it leave a hole between them.
    assert_eq!(join(0, 0, JoinSide::Tail).moved(-5, room), join(0, -5, JoinSide::Tail), "the sound before ends 5 early");
    assert_eq!(join(0, -5, JoinSide::Lead).moved(4, room), join(-4, -5, JoinSide::Lead), "the sound after starts 4 late");
    // Each stays inside its own part.
    assert_eq!(join(0, 0, JoinSide::Tail).moved(-100, room), join(0, -40, JoinSide::Tail));
}

fn part(clip: &str, from: u64, to: u64, gap: u64) -> Piece {
    Piece { clip_id: clip.into(), clip_name: clip.into(), from_frame: from, to_frame: to, start_tc: String::new(), end_tc: String::new(), length: String::new(), seconds: 1.0, text: String::new(), gap_before: gap, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false, a1_lead_frames: 0, a1_tail_frames: 0, v_lead_frames: 0, v_tail_frames: 0 }
}

#[test]
fn a_subsegment_is_a_card_of_the_pool_with_its_first_clips_poster() {
    let profile = general_tv();
    let mut several = Block::of_pieces("sot", vec![part("2002", 100, 150, 0), part("2679", 10, 60, 4)]);
    several.id = 7;
    let blocks = [sot("Rekao je."), several];
    let view = ScriptView { blocks: rows(&blocks, Some(&profile), 2.5), profile: Some(profile), ..ScriptView::default() };
    let clip = qnc_clip_row::ClipRow { clip_id: "2002".into(), name: "Mironik 2002".into(), thumb_uri: Some("qnc://poster".into()), ..Default::default() };
    let clips = [clip];
    let cards = view.pool_cards(&clips);
    assert_eq!(view.subsegment_count(), 1, "a block of one range is no SubSegment");
    assert_eq!((cards[0].id.as_str(), cards[0].seconds), ("7", 2.0));
    assert!(cards[0].title.starts_with("SOT 2"), "{}", cards[0].title);
    assert_eq!(qnc_clip_row::PoolItem::card_poster(&cards[0]).0, Some("qnc://poster"));
}

#[test]
fn the_filmstrip_of_a_subsegment_lays_each_part_after_its_pause() {
    let pieces = vec![part("a", 100, 150, 0), part("b", 10, 60, 5)];
    let shown = subsegment_view(&Block::of_pieces("sot", pieces.clone()), None);
    let at: Vec<_> = subsegment_pictures(&shown, &pieces).iter().map(|p| (p.clip_id.clone(), p.program_start, p.program_end, p.source_in, p.source_out)).collect();
    assert_eq!(at, vec![("a".to_string(), 0, 50, 100, 150), ("b".to_string(), 55, 105, 10, 60)]);
}

#[test]
fn the_zoom_of_a_subsegment_edge_shows_the_pictures_either_side_of_its_cut() {
    let pieces = vec![part("a", 100, 150, 0), part("a", 300, 340, 0), part("b", 10, 60, 5)];
    let block = Block::of_pieces("sot", pieces.clone());
    let shown = subsegment_view(&block, None);
    assert_eq!(shown.parts.iter().map(|p| (p.start, p.end)).collect::<Vec<_>>(), vec![(0, 50), (50, 90), (95, 145)]);
    // The IN of the second part: the last picture of the first part, then its own first, marked.
    let zoom = subsegment_zoom(&shown, &pieces, (1, false), (50, None), |_| Some(1000)).unwrap();
    let pictures: Vec<_> = zoom.source_edit_stills.iter().map(|s| (s.picture.as_ref().map(|p| (p.clip_id.as_str(), p.source_frame)), s.cut)).collect();
    assert_eq!(pictures, vec![(Some(("a", 149)), false), (Some(("a", 300)), true)]);
    let sound = zoom.zoom_sound.unwrap();
    assert_eq!((sound.from, sound.to, sound.cut), (0, 100, 50));
    assert_eq!(sound.lanes[0].len(), 3, "every part within a second of the cut is heard on A1");
    // A click on a picture chooses its slot (user 2026-11-04): the left one the part before, the
    // right one the part after.
    let clicks: Vec<_> = zoom.source_edit_stills.iter().map(|s| s.click.as_ref().map(|(action, target)| (action.as_str(), target.as_str()))).collect();
    assert_eq!(clicks, vec![Some((crate::SUBSEGMENT_SELECT, "0")), Some((crate::SUBSEGMENT_SELECT, "1"))]);
    // The IN of the third part follows a pause: on the left is still the OUT of the part before.
    let zoom = subsegment_zoom(&shown, &pieces, (2, false), (50, None), |_| Some(1000)).unwrap();
    assert_eq!(zoom.source_edit_stills[0].picture.as_ref().map(|p| (p.clip_id.as_str(), p.source_frame)), Some(("a", 339)));
    // The OUT of the first part: its last picture, marked, then the next part's first.
    let zoom = subsegment_zoom(&shown, &pieces, (0, true), (50, None), |_| Some(1000)).unwrap();
    let pictures: Vec<_> = zoom.source_edit_stills.iter().map(|s| (s.picture.as_ref().map(|p| p.source_frame), s.cut)).collect();
    assert_eq!(pictures, vec![(Some(149), true), (Some(300), false)]);
    // The first part's IN has no OUT before it; the last part's OUT no IN after it.
    let first = subsegment_zoom(&shown, &pieces, (0, false), (50, None), |_| Some(1000)).unwrap();
    assert!(first.source_edit_stills[0].click.is_none());
    let last = subsegment_zoom(&shown, &pieces, (2, true), (50, None), |_| Some(1000)).unwrap();
    assert!(last.source_edit_stills[1].click.is_none());
}

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
    let take = |take: u16, text: &str| VoiceTake { take, wav_uri: format!("qnc://p/T{take}.wav"), seconds: 2.0, tc: "21:00:00:00".into(), text: text.into(), heard: None, source: None };
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
    let words = crate::view::timed_words(&transcripts, &part("c", 100, 200, 0), &[], |frame| format!("tc{frame}"));
    let shown: Vec<(&str, &str, bool)> = words.iter().map(|w| (w.text.as_str(), w.tc.as_str(), w.line)).collect();
    // Only the words of the part, each line of speech with its timecode in front.
    assert_eq!(shown, [("Dobar", "tc100", true), ("dan.", "tc120", false), ("Kako", "tc150", true), ("ste?", "tc170", false)]);
}


#[test]
fn a_word_cut_by_letters_shows_the_letters_the_part_still_says() {
    use qnc_letter_db::{Letter, WordLetters};
    let word = qnc_transcript_db::Word { from: 100, to: 120, text: "pa,".into() };
    let line = qnc_transcript_db::Line { from: 100, to: 120, text: "pa,".into(), words: vec![word] };
    let transcript = qnc_transcript_db::ClipTranscript { clip_id: "c".into(), from_frame: 0, to_frame: 500, language: "hr".into(), engine: "e".into(), lines: vec![line] };
    let letter = |letter: char, from: Option<f64>, to: Option<f64>| Letter { letter, from, to };
    let laid = [WordLetters { word_from: 100, text: "pa,".into(), letters: vec![letter('p', Some(100.0), Some(108.0)), letter('a', Some(108.0), Some(120.0)), letter(',', None, None)] }];
    let words = crate::view::timed_words(std::slice::from_ref(&transcript), &part("c", 108, 200, 0), &laid, |_| String::new());
    assert_eq!(words.iter().map(|word| (word.text.as_str(), word.from, word.to)).collect::<Vec<_>>(), [("a,", 108, 120)]);
    assert_eq!(crate::view::timed_words(std::slice::from_ref(&transcript), &part("c", 100, 200, 0), &laid, |_| String::new())[0].text, "pa,", "the whole word");
    assert!(crate::view::timed_words(std::slice::from_ref(&transcript), &part("c", 120, 200, 0), &laid, |_| String::new()).is_empty(), "none of it");
}

fn part(clip: &str, from: u64, to: u64, gap: u64) -> Piece {
    Piece { clip_id: clip.into(), clip_name: clip.into(), from_frame: from, to_frame: to, start_tc: String::new(), end_tc: String::new(), length: String::new(), seconds: 1.0, text: String::new(), gap_before: gap, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false, a1_lead_frames: 0, a1_tail_frames: 0, v_lead_frames: 0, v_tail_frames: 0, level_keys: Vec::new(), smooth_frames: 0, smooth_clip: String::new() }
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


/// docs/99 step 4: the text of a statement's part edited before Enter is the script's (not egui
/// memory): typed it is kept, Esc gives it back, and a draft of words said differently is gone.
#[test]
fn the_edited_text_of_a_statement_is_the_scripts_until_enter_or_esc() {
    let mut statement = sot("Rekao je danas.");
    statement.id = 4;
    let word = |text: &str, from: u64| TimedWord { from, to: from + 10, text: text.into(), tc: String::new(), line: false, letters: Vec::new() };
    let mut script = Script::new();
    script.view.blocks = rows(&[statement], Some(&general_tv()), 2.5);
    script.view.blocks[0].words = vec![vec![word("Rekao", 0), word("je", 10), word("danas.", 20)]];
    let preview = SourcePreview::default();
    let said = "Rekao je danas.";
    assert_eq!(script.pick_key(TEXT_DRAFT, Some("4:0:Rekao danas."), &preview), Some(true));
    assert_eq!(script.view.text_draft(4, 0, said), Some("Rekao danas."), "what is typed is kept");
    assert_eq!(script.view.text_draft(4, 0, "Rekao je."), None, "words said differently: no draft");
    script.pick_key(TEXT_DRAFT, Some(&format!("4:0:{said}")), &preview);
    assert_eq!(script.view.text_draft(4, 0, said), None, "typed back as said: no draft");
    script.pick_key(TEXT_DRAFT, Some("4:0:Rekao."), &preview);
    script.pick_key(TEXT_REVERT, Some("4:0"), &preview);
    assert_eq!(script.view.text_draft(4, 0, said), None, "Esc gives the text back");
}

#[test]
fn every_off_of_the_story_is_asked_for_b_roll_a_slot_between_its_marks_with_its_note() {
    let profile = general_tv();
    // Ten words at 2.5 a second: 4 seconds; marks before the 4th and the 8th word.
    let mut first = Block::written("off", "Ribari su jutros isplovili iz luke, brodovi su se vratili.");
    first.id = 3;
    first.marks = vec![3, 7];
    first.broll_note = "luka, brodovi".into();
    let second = Block { id: 5, ..Block::written("off", "Navečer su se vratili.") };
    let blocks = [Block { id: 2, ..Block::written("intro", "Dobar dan.") }, first, Block { id: 4, ..sot("Rekao je.") }, second];
    let view = ScriptView { blocks: rows(&blocks, Some(&profile), 2.5), profile: Some(profile), ..ScriptView::default() };
    let offs = view.broll_offs();
    assert_eq!(offs.iter().map(|(id, ..)| *id).collect::<Vec<_>>(), [3, 5], "the anchor's text and a statement have no B-roll");
    let (_, seconds, slots, note) = &offs[0];
    assert_eq!(slots.iter().map(|(text, _)| text.as_str()).collect::<Vec<_>>(), ["Ribari su jutros", "isplovili iz luke, brodovi", "su se vratili."]);
    let shared: f64 = slots.iter().map(|(_, seconds)| seconds).sum();
    assert!((shared - seconds).abs() < 0.01 && (seconds - 4.0).abs() < 0.01, "the shots share the OFF's length: {slots:?}");
    assert_eq!(note, "luka, brodovi");
    assert_eq!(offs[1].2.len(), 1, "an OFF without marks is one slot");
}

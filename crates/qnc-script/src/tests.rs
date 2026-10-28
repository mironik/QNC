use super::*;

fn general_tv() -> ScriptProfile {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/script-profiles/general_tv.json");
    serde_json::from_value(qnc_ui_text::resolve_contract(&std::fs::read_to_string(file).unwrap()).unwrap()).unwrap()
}

fn sot(text: &str) -> Block {
    Block { clip_id: "c".into(), clip_name: "Mironik 2002".into(), start_tc: "10:00:02:00".into(), end_tc: "10:00:06:49".into(), length: "5:00".into(), seconds: 5.0, ..Block::written("sot", text) }
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
    assert_eq!(marks, [profile.kind("intro").unwrap().short.as_str(), "OFF 1", "SOT 1", "OFF 2", "NAT"]);
    assert_eq!(rows.iter().map(|row| row.seconds).collect::<Vec<_>>(), [0.4, 1.2, 5.0, 7.0, 0.0]);
    assert!(rows[1].recorded && !rows[0].recorded && rows[2].is_clip());
}

#[test]
fn the_script_as_text_has_every_block_in_reading_order_with_its_mark() {
    let profile = general_tv();
    let blocks = [Block::written("off", "Uvod u prilog."), sot("Rekao je.")];
    let view = ScriptView { blocks: rows(&blocks, Some(&profile), 2.5), profile: Some(profile), ..ScriptView::default() };
    assert_eq!(view.text(), "OFF 1 (0:01)\nUvod u prilog.\n\nSOT 1 Mironik 2002 10:00:02:00 - 10:00:06:49 (5:00)\nRekao je.");
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

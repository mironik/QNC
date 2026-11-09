use super::*;
use qnc_script_db::VoiceTake;

fn general_tv() -> ScriptProfile {
    let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../catalogs/script-profiles/general_tv.json");
    serde_json::from_value(qnc_ui_text::resolve_contract(&std::fs::read_to_string(file).unwrap()).unwrap()).unwrap()
}

fn block(id: i64, kind: &str, text: &str) -> Block {
    Block { id, ..Block::written(kind, text) }
}

fn sot(id: i64, clip: &str, from: u64, to: u64) -> Block {
    Block { id, clip_id: clip.into(), from_frame: from, to_frame: to, ..Block::written("sot", "") }
}

#[test]
fn a_draft_is_the_script_in_reading_order_with_its_recorded_takes_and_pauses_for_the_rest() {
    let take = |take: u16, seconds: f64| VoiceTake { take, wav_uri: String::new(), seconds, tc: String::new(), text: String::new(), heard: None };
    let voices = [Voice { block_id: 2, name: "P_OFF1".into(), clip_uri: String::new(), chosen: 2, takes: vec![take(1, 4.0), take(2, 3.0)] }];
    let blocks = [
        block(1, "intro", "Najava se ne broji u prilog."),
        block(2, "off", "Snimljeni off."),
        sot(3, "c1", 100, 200),
        // Five words at 2.5 a second: a pause of 2 s before the next part.
        block(4, "off", "jedan dva tri cetiri pet"),
        sot(5, "c2", 10, 60),
    ];
    let clip_id = |name: &str| format!("voice-{name}");
    let rates = DraftRates { words_per_second: 2.5, fps: 50.0, clip_id: &clip_id };
    let pieces = draft_pieces(&blocks, &voices, &general_tv(), &rates).unwrap();
    let shown: Vec<_> = pieces.iter().map(|p| (p.clip_id.as_str(), p.from_frame, p.to_frame, p.gap_before, p.a1)).collect();
    assert_eq!(shown, vec![("voice-P_OFF1_T2", 0, 150, 0, Some((Some(0), 0, false))), ("c1", 100, 200, 0, None), ("c2", 10, 60, 100, None)]);
}

#[test]
fn a_script_with_nothing_for_the_draft_says_so() {
    let clip_id = |name: &str| name.to_string();
    let rates = DraftRates { words_per_second: 2.5, fps: 50.0, clip_id: &clip_id };
    assert!(draft_pieces(&[block(1, "off", "Nije snimljen.")], &[], &general_tv(), &rates).is_err());
}

#[test]
fn the_skeleton_is_the_script_as_story_segments_without_the_offs_not_recorded() {
    let take = |take: u16, seconds: f64| VoiceTake { take, wav_uri: String::new(), seconds, tc: String::new(), text: String::new(), heard: None };
    let voices = [Voice { block_id: 2, name: "P_OFF1".into(), clip_uri: String::new(), chosen: 2, takes: vec![take(1, 4.0), take(2, 3.0)] }];
    let blocks = [
        block(1, "intro", "Najava se ne broji u prilog."),
        block(2, "off", "Snimljeni off."),
        sot(3, "c1", 100, 200),
        block(4, "off", "Nije snimljen."),
        sot(5, "c2", 10, 60),
    ];
    let clip_id = |name: &str| format!("voice-{name}");
    let rates = DraftRates { words_per_second: 2.5, fps: 50.0, clip_id: &clip_id };
    let parts = skeleton(&blocks, &voices, &general_tv(), &rates).unwrap();
    let shown: Vec<_> = parts.iter().map(|p| (p.kind, p.clip_id.as_str(), p.from_frame, p.to_frame, p.origin.as_str())).collect();
    assert_eq!(shown, vec![("offovi", "voice-P_OFF1_T2", 0, 150, "script:2:0"), ("tonovi", "c1", 100, 200, "script:3:0"), ("tonovi", "c2", 10, 60, "script:5:0")]);
    assert_eq!(parts[0].a1, Some((Some(0), 0, false)));
}

#[test]
fn a_good_reading_has_its_in_and_out_where_its_sound_is_a_bad_one_has_none() {
    let heard = |text: &str, start: f64, end: f64| qnc_script_db::HeardWord { text: text.into(), start, end };
    let take = |words| VoiceTake { take: 1, wav_uri: String::new(), seconds: 6.0, tc: String::new(), text: String::new(), heard: Some(words) };
    let good = take(vec![heard("prva", 1.0, 1.4), heard("druga", 1.5, 2.0), heard("treca", 2.1, 3.0)]);
    assert_eq!(take_sound_range("prva druga treca", &good, 50.0), Some((45, 155)));
    let bad = take(vec![heard("prva", 1.0, 1.4)]);
    assert_eq!(take_sound_range("prva druga treca", &bad, 50.0), None);
}

#[test]
fn a_marker_in_the_text_of_an_off_is_the_frame_its_word_was_heard_at() {
    let heard = |text: &str, start: f64| qnc_script_db::HeardWord { text: text.into(), start, end: start + 0.2 };
    let take = VoiceTake { take: 1, wav_uri: String::new(), seconds: 4.0, tc: String::new(), text: String::new(), heard: Some(vec![heard("prva", 0.2), heard("druga", 1.5), heard("treca", 3.0)]) };
    let voices = [Voice { block_id: 2, name: "P".into(), clip_uri: String::new(), chosen: 1, takes: vec![take] }];
    let mut off = block(2, "off", "prva druga treca");
    off.marks = vec![1, 2];
    let clip_id = |name: &str| name.to_string();
    let rates = DraftRates { words_per_second: 2.5, fps: 50.0, clip_id: &clip_id };
    let parts = skeleton(&[off], &voices, &general_tv(), &rates).unwrap();
    assert_eq!(parts[0].marks, [75, 150]);
}

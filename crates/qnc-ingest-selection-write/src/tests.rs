use super::*;
use qnc_import_fit::Playback;

#[test]
fn an_idle_writer_has_no_outcome_and_can_be_cancelled_repeatedly() {
    let mut writer = SelectionWriter::new();
    assert!(!writer.is_busy());
    assert!(writer.poll().is_none());
    writer.cancel();
    writer.cancel();
    assert!(!writer.is_busy());
}

#[test]
fn clips_in_the_way_give_a_choice_to_import_the_others_or_cancel() {
    let proxy = Needs { playback: Playback::Proxy, copies_proxy: false };
    let found = vec![("c1".to_string(), "C0012.MP4".to_string(), Conflict::ProxyMissing)];
    let choice = choice_of(&found, proxy, 3);
    assert_eq!(choice.items, ["C0012.MP4"]);
    assert_eq!(choice.options.iter().map(|o| o.action_id.as_str()).collect::<Vec<_>>(), ANSWERS);
    assert!(choice.options[0].strong && choice.options[0].label.contains('2'), "the proposed answer imports the other two");
    assert!(choice.solution.contains("Proxy if available"));
    let only = choice_of(&found, proxy, 1);
    assert_eq!(only.options.len(), 1, "nothing else to import: only cancel");
    assert_eq!(only.options[0].action_id, CANCEL_IMPORT);
}

#[test]
fn an_answer_without_a_choice_drops_nothing_and_another_action_is_not_an_answer() {
    let mut writer = SelectionWriter::new();
    assert_eq!(writer.answer("ingest_import_selected"), None);
    assert_eq!(writer.answer(SKIP_CONFLICTS), None, "no choice is waiting");
    assert!(writer.choice().is_none());
}

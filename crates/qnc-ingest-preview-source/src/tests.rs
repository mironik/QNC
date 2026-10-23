use super::*;

#[test]
fn choosing_a_clip_says_what_could_be_opened_and_a_key_without_a_player_says_why() {
    let mut preview = IngestPreviewSource::new();
    assert_eq!(preview.choose("clip-1", false, None), Chosen::NotSaved);
    assert_eq!(preview.choose("clip-1", true, None), Chosen::Waiting, "no project loaded: nothing is opened");
    let mut timeline = qnc_timeline::TimelineProjection::default();
    let refused = preview.route("play_pause", None, &mut timeline).unwrap_err();
    assert_eq!(refused, qnc_ui_text::text("player.broadcast_player_connected"));
    let shown = preview.shown(Some("clip-1"), &timeline, std::iter::empty(), None);
    assert_eq!(shown.assets.clip_id, "clip-1");
}

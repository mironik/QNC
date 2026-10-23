use super::*;

#[test]
fn a_segment_row_shows_picture_audio_spans_markers_and_slots() {
    let layers = layers(false);
    assert!(layers.carrier && layers.audio_a1 && layers.base_video && layers.audio_a2);
    assert!(layers.virtual_spans && layers.markers && layers.marker_slots && layers.playhead);
    assert!(!layers.in_out);
}

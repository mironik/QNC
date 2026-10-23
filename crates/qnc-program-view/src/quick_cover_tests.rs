use super::*;

fn slot(id: &str, start: u64, end: u64, has_cover: bool, selected: bool) -> Slot {
    Slot { slot_id: id.into(), start_frame: start, end_frame: end, has_cover, selected }
}

#[test]
fn a_chosen_slot_with_a_b_roll_gives_way_to_the_empty_slot_under_the_playhead() {
    let mut view = SegmentsView {
        slots: vec![slot("s1", 0, 100, true, true), slot("s2", 100, 200, false, false)],
        playhead: Some(150),
        ..Default::default()
    };
    assert_eq!(view.quick_cover_slot().unwrap().slot_id, "s2");
    view.playhead = Some(50);
    assert!(view.quick_cover_slot().is_err(), "nothing empty to take: it says so");
    view.slots[0].has_cover = false;
    assert_eq!(view.quick_cover_slot().unwrap().slot_id, "s1");
}

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

#[test]
fn the_lead_in_and_lead_out_are_drawn_in_the_rows_of_the_first_and_last_segment() {
    let row = |id: &str, (start, end): (u64, u64), lead: bool| SegmentRow {
        segment_id: id.into(),
        kind: SegmentKind::Ton,
        clip_id: "c".into(),
        source_in_frame: 0,
        source_out_frame: end - start,
        a1_source_channel: 0,
        a1_level_db: 0,
        a1_off: lead,
        start_frame: start,
        end_frame: end,
        duration_label: String::new(),
        duration_color_key: "under_3",
        selected: false,
        lead,
    };
    let view = SegmentsView {
        rows: vec![row("li0", (0, 70), true), row("li1", (70, 100), true), row("a", (100, 200), false), row("b", (200, 300), false), row("lo", (300, 400), true)],
        total_frames: 400,
        ..Default::default()
    };
    let shown: Vec<_> = view.wrap_rows().into_iter().map(|(window, row)| (window, row.segment_id.as_str())).collect();
    assert_eq!(shown, vec![((0, 200), "a"), ((200, 400), "b")]);
    assert_eq!(view.user_segment_at(10).unwrap().segment_id, "a");
    assert_eq!(view.user_segment_at(350).unwrap().segment_id, "b");
    assert_eq!(view.user_segment_at(400).unwrap().segment_id, "b");
}

use super::*;

fn part(clip: &str, from: u64, to: u64, gap: u64) -> Piece {
    Piece { clip_id: clip.into(), clip_name: clip.into(), from_frame: from, to_frame: to, start_tc: String::new(), end_tc: String::new(), length: String::new(), seconds: 1.0, text: String::new(), gap_before: gap, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false }
}

#[test]
fn the_filmstrip_of_the_parts_lays_each_part_after_its_pause() {
    let pieces = vec![part("a", 100, 150, 0), part("b", 10, 60, 5)];
    let shown = subsegment_view(1, &pieces, None);
    let at: Vec<_> = subsegment_pictures(&shown, &pieces).iter().map(|p| (p.clip_id.clone(), p.program_start, p.program_end, p.source_in, p.source_out)).collect();
    assert_eq!(at, vec![("a".to_string(), 0, 50, 100, 150), ("b".to_string(), 55, 105, 10, 60)]);
}

#[test]
fn the_zoom_of_an_edge_shows_the_pictures_either_side_of_its_cut() {
    let pieces = vec![part("a", 100, 150, 0), part("a", 300, 340, 0), part("b", 10, 60, 5)];
    let shown = subsegment_view(1, &pieces, None);
    assert_eq!(shown.parts.iter().map(|p| (p.start, p.end)).collect::<Vec<_>>(), vec![(0, 50), (50, 90), (95, 145)]);
    // The IN of the second part: the last picture of the first part, then its own first, marked.
    let zoom = subsegment_zoom(&shown, &pieces, (1, false), (50, None), |_| Some(1000)).unwrap();
    let pictures: Vec<_> = zoom.source_edit_stills.iter().map(|s| (s.picture.as_ref().map(|p| (p.clip_id.as_str(), p.source_frame)), s.cut)).collect();
    assert_eq!(pictures, vec![(Some(("a", 149)), false), (Some(("a", 300)), true)]);
    let sound = zoom.zoom_sound.unwrap();
    assert_eq!((sound.from, sound.to, sound.cut), (0, 100, 50));
    assert_eq!(sound.lanes[0].len(), 3, "every part within a second of the cut is heard on A1");
    // A click on the left picture takes the OUT of the part before, on the right one this IN.
    let clicks: Vec<_> = zoom.source_edit_stills.iter().map(|s| s.click.as_ref().map(|(_, target)| target.as_str())).collect();
    assert_eq!(clicks, vec![Some("0:out"), Some("1:in")]);
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

#[test]
fn a_change_of_a_part_comes_back_as_a_write_of_the_owners_parts() {
    let pieces = vec![part("a", 100, 150, 0), part("b", 10, 60, 5)];
    let mut edit = SubsegmentEdit::new();
    let clips = Clips::new();
    assert!(edit.act(JOIN_CUT, Some("7:1"), &pieces, &clips).is_none(), "nothing is open");
    edit.open(7, &pieces);
    assert!(edit.take_pieces_request().is_some(), "the parts play when they open");
    assert_eq!(edit.act(JOIN_CUT, Some("7:1"), &pieces, &clips), Some(Answer::Write(PieceWrite::Join { index: 1, gap: 5, cut: true })));
    assert_eq!(edit.act(PART_MOVE, Some("7:1:-1"), &pieces, &clips), Some(Answer::Write(PieceWrite::Move { index: 1, earlier: true })));
    assert!(edit.act(PART_REMOVE, Some("8:1"), &pieces, &clips).is_none(), "another key's part is not these");
}

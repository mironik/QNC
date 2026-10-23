use super::*;

fn source(source_in: u64, duration: u64) -> SyncSource {
    SyncSource {
        clip_id: "c".into(),
        clip_name: "Clip".into(),
        source_in,
        source_out: duration,
        duration_frames: duration,
        timebase: (50, 1),
    }
}

fn armed() -> SyncCover {
    let mut sync = SyncCover::new();
    sync.toggle();
    sync.arm(source(10, 40));
    sync
}

#[test]
fn space_plays_from_the_marker_before_the_playhead_to_the_end_of_the_source() {
    let mut sync = armed();
    assert!(sync.wants_space());
    let preview = sync.start(&[0, 20, 60], 25, 100, (1, 1)).unwrap();
    assert_eq!(preview.window, (20, 50), "anchor 20, 30 source frames left");
    assert_eq!(preview.source_in, 10);
    assert!(!sync.wants_space(), "IN is used once (v5 set_active)");
}

#[test]
fn sync_stops_at_the_source_out_or_the_next_marker_whichever_comes_first() {
    let mut sync = armed();
    let preview = sync.start(&[0, 20, 35], 25, 100, (1, 1)).unwrap();
    assert_eq!(preview.window, (20, 35), "the next marker closes it");
    let mut sync = SyncCover::new();
    sync.toggle();
    sync.arm(SyncSource {
        source_out: 18,
        ..source(10, 40)
    });
    let preview = sync.start(&[0, 20, 35], 25, 100, (1, 1)).unwrap();
    assert_eq!(preview.window, (20, 28), "the source OUT closes it");
}

#[test]
fn sync_needs_a_marker_before_the_playhead_and_one_timebase() {
    let mut sync = armed();
    assert!(sync.start(&[30], 25, 100, (1, 1)).is_err());
    assert!(sync.start(&[0], 25, 100, (3, 1)).is_err());
    let mut off = SyncCover::new();
    off.arm(source(0, 10));
    assert!(!off.wants_space(), "IN arms only while Sync is on");
}

#[test]
fn frames_follow_the_window_and_o_closes_the_slot() {
    let mut sync = armed();
    sync.start(&[0, 20], 25, 100, (1, 1)).unwrap();
    assert_eq!(
        sync.program_frame(Some(1)),
        Some(21),
        "the window may start on frame 1 (live player log)"
    );
    assert_eq!(sync.program_frame(Some(12)), Some(32));
    assert_eq!(
        sync.source_view(),
        Some((22, Some((10, 22)))),
        "the source plays in parallel"
    );
    let slot = sync.finish(32).unwrap().clone();
    assert_eq!((slot.start, slot.end, slot.source_out), (20, 32, 22));
    assert_eq!(
        sync.source_view(),
        Some((22, Some((10, 22)))),
        "OUT and the slot marks"
    );
    assert_eq!(sync.missing_marker(&[0, 20]), Some(32));
    assert_eq!(sync.missing_marker(&[0, 20, 32]), None);
    assert_eq!(
        sync.resolve([("a|b", 0, 20), ("b|c", 20, 32)].into_iter()),
        Some("b|c".into())
    );
    assert!(sync.take_commit(false).is_none(), "O waits for Enter");
    let (slot_id, slot) = sync.take_commit(true).unwrap();
    assert_eq!(
        (slot_id.as_str(), slot.source.source_in, slot.source_out),
        ("b|c", 10, 22)
    );
    assert!(sync.holds_enter(), "until the write lands");
    sync.landed();
    assert!(!sync.holds_enter());
}

#[test]
fn the_end_of_the_source_stops_and_waits_for_enter() {
    let mut sync = armed();
    sync.start(&[0, 20], 25, 100, (1, 1)).unwrap();
    assert_eq!(sync.program_frame(Some(29)), Some(49), "stopped on its last picture");
    assert!(sync.is_active(), "it waits: nothing is closed or written");
    assert!(sync.at_end());
    sync.finish(sync.draft_end().unwrap()).unwrap(); // Enter
    sync.resolve([("b|c", 20, 50)].into_iter());
    assert!(sync.take_commit(true).is_some());
}

#[test]
fn a_window_end_missed_between_repaints_still_stops_at_the_marker() {
    let mut sync = armed();
    sync.start(&[0, 20, 35], 25, 100, (1, 1)).unwrap();
    assert_eq!(sync.program_frame(Some(5)), Some(25));
    assert_eq!(
        sync.program_frame(Some(0)),
        Some(34),
        "the player went back to the window start: its end was reached"
    );
    assert_eq!(sync.draft_end(), Some(35));
    assert!(sync.missing_marker(&[0, 20, 35]).is_none() || sync.is_active(), "the next marker ends it");
}

#[test]
fn a_pause_writes_nothing_the_arrows_move_the_end_space_plays_on_delete_drops_it() {
    let mut sync = armed();
    sync.start(&[0, 20], 25, 100, (1, 1)).unwrap();
    assert_eq!(sync.program_frame(Some(8)), Some(28));
    // A pause confirms a frame or two before the last: no end, no write.
    assert_eq!(sync.program_frame(Some(7)), Some(27));
    assert!(sync.is_active() && !sync.at_end());
    assert_eq!(sync.draft_end(), Some(28));
    assert!(sync.nudge(3));
    assert_eq!(sync.draft_end(), Some(31), "the arrows move the end");
    // The OUT follows the new end on the source.
    assert_eq!(sync.source_view(), Some((21, Some((10, 21)))));
    let window = sync.resume().expect("space plays on from the end");
    assert_eq!(window.window, (31, 50));
    assert_eq!(window.source_in, 10 + 11, "the source from where it got to");
    assert_eq!(sync.program_frame(Some(4)), Some(35));
    assert!(sync.discard(), "Delete drops the draft");
    assert!(!sync.is_active() && !sync.holds_enter());
}

#[test]
fn a_source_of_half_or_twice_the_story_rate_counts_in_story_frames() {
    // XDCAM 1080i50 (25) in a 50p story: every field is a story frame.
    let mut sync = armed();
    let preview = sync.start(&[0, 20], 25, 200, (2, 1)).unwrap();
    assert_eq!(preview.window, (20, 80), "30 source frames are 60 story frames");
    assert_eq!(sync.program_frame(Some(11)), Some(31));
    assert_eq!(sync.source_view(), Some((15, Some((10, 15)))));
    let slot = sync.finish(31).unwrap().clone();
    assert_eq!(slot.source_out, 16, "11 story frames need 6 source pictures");
    // 50p in a 25 story: every other picture.
    let mut sync = armed();
    let preview = sync.start(&[0, 20], 25, 200, (1, 2)).unwrap();
    assert_eq!(preview.window, (20, 35), "30 source frames are 15 story frames");
    let slot = sync.finish(30).unwrap().clone();
    assert_eq!(slot.source_out, 30);
}

#[test]
fn switching_sync_off_drops_everything() {
    let mut sync = armed();
    sync.start(&[0], 5, 100, (1, 1)).unwrap();
    assert!(sync.toggle(), "a running play was stopped");
    assert!(!sync.enabled() && !sync.is_active() && !sync.holds_enter());
}

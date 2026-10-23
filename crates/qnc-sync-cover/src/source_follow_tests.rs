use super::*;

#[test]
fn the_draft_follows_the_playing_source_from_its_in() {
    let mut sync = SyncCover::new();
    sync.toggle();
    sync.arm(SyncSource { clip_id: "c".into(), clip_name: "c".into(), source_in: 100, source_out: 400, duration_frames: 400, timebase: (50, 1) });
    let preview = sync.start(&[20, 60], 30, 200, (1, 1)).expect("start");
    assert_eq!(preview.window, (20, 60));
    assert_eq!(sync.source_window_frame(100), Some(0));
    let frame = sync.source_window_frame(110);
    assert_eq!(sync.program_frame(frame), Some(30), "ten source frames after IN");
    assert_eq!(sync.draft_span(), Some((20, 31)));
    assert_eq!(sync.source_view().map(|view| view.0), Some(110));
}

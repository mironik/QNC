//! The keys of the segment panel (Wrap rows of the story): the sound of the stored segment
//! (A1) or cover (A2) under the Wrap playhead, Enter that confirms what the panel took, the
//! arrows of the Wrap playhead and of a taken slot or marker, Ctrl+I/O on a slot edge and
//! Ctrl+Shift+I/O on a segment edge. I and O do nothing here (user 2026-10-16).
//!
//! What the keys change is the program model's (`qnc-program-segments`); the A1/A2 lane keys
//! are those of every timeline (`qnc_timeline::LaneKey`); a sound draft is heard in the one
//! preview before Enter. It owns no player, database or form, and knows no application.

use std::ops::ControlFlow;

use qnc_program_segments::{ProgramSegments, SegmentCommand};
use qnc_source_preview::{SoundDraft, SourcePreview};
use qnc_timeline::LaneKey;
use qnc_wrap_session::WrapSession;

pub const MODULE_ID: &str = "qnc.module.segments-keys";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The sound of a stored segment or cover on the Wrap row (user 2026-10-16): Ctrl+1, Ctrl+2
/// take it under the Wrap playhead; while it is taken the lane keys draft it, heard at once;
/// Escape lets it go and the stored sound is heard again. `Continue` for any other key.
pub fn sound(
    action_id: &str,
    (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments),
) -> ControlFlow<Option<bool>> {
    ControlFlow::Break(Some(match action_id {
        "select_audio_a1" | "select_audio_a2" => {
            let taken = segments.take_part_sound(u8::from(action_id == "select_audio_a2"), wrap.playhead());
            say_part_lane(segments);
            taken
        }
        lane if segments.part_sound_taken()
            && matches!(LaneKey::of(lane), Some(LaneKey::Channel(_) | LaneKey::Off(_) | LaneKey::Level(_))) =>
        {
            draft(LaneKey::of(lane).unwrap_or(LaneKey::Drop), (preview, wrap, segments))
        }
        "clear_focus" if segments.part_sound_taken() => {
            let released = segments.release_part_sound();
            preview.hear_program_draft(None, None, wrap.playhead()); // the stored sound again
            released
        }
        _ => return ControlFlow::Continue(()),
    }))
}

/// The J or L of a cover (user 2026-10-21): Ctrl+J, Ctrl+L take it under the Wrap playhead;
/// while it is taken the arrows (and the wheel) make it longer or shorter, heard at once in
/// the program; Escape drops it and the stored J and L are heard again. `Continue` for any
/// other key.
pub fn overlap(
    action_id: &str,
    (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments),
) -> ControlFlow<Option<bool>> {
    let taken = segments.overlap_taken();
    let handled = match action_id {
        "select_cover_lead" | "select_cover_tail" => true,
        "step_back_frame" | "step_forward_frame" | "clear_focus" => taken,
        _ => false,
    };
    if !handled {
        return ControlFlow::Continue(());
    }
    let done = segments.apply_action(action_id);
    preview.hear_program_draft(overlap_draft(segments), None, wrap.playhead());
    ControlFlow::Break(Some(done))
}

/// The J and L drafted on the Wrap row, as the program plays them before Enter.
fn overlap_draft(segments: &ProgramSegments) -> Option<SoundDraft> {
    let (id, overlap, (channel, level_db, off)) = segments.overlap_draft()?;
    Some(SoundDraft { lane: 1, id, channel, level_db, off, overlap: Some(overlap) })
}

/// A channel picked in the column of the taken sound (a click, or any device that names
/// the channel): drafted and heard at once, as up/down. False when lane `lane` is not the
/// taken one.
pub fn pick(
    lane: u8,
    channel: u16,
    (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments),
) -> bool {
    let taken = segments.part_sound_lanes().and_then(|lanes| lanes.taken()).map(|(taken, _)| taken);
    taken == Some(lane) && draft(LaneKey::Pick(channel), (preview, wrap, segments))
}

/// A lane key on the taken sound: drafted, said in the status line and heard in the program.
fn draft(key: LaneKey, (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments)) -> bool {
    let changed = segments.part_sound_lanes().is_some_and(|lanes| qnc_timeline::lane_key(lanes, key));
    say_part_lane(segments);
    preview.hear_program_draft(sound_draft(segments), stored_sound(segments), wrap.playhead());
    changed
}

/// The keyboard left the segment panel (a click, Tab or Enter): what it took is dropped and
/// the stored sound is heard again. True when an edit was dropped.
pub fn left(preview: &mut SourcePreview, wrap: &WrapSession, segments: &mut ProgramSegments) -> bool {
    if segments.part_sound_taken() || segments.overlap_taken() {
        segments.release_part_sound();
        preview.hear_program_draft(None, None, wrap.playhead());
    }
    segments.leave_panel()
}

/// Another edit took over from a sound draft: the stored sound is heard again.
pub fn dropped(preview: &mut SourcePreview, wrap: &WrapSession, segments: &mut ProgramSegments) {
    if segments.take_dropped_sound() {
        preview.hear_program_draft(None, None, wrap.playhead());
    }
}

/// Enter on the segment panel confirms what it took (a marker, slot, slot edge, sound);
/// false when there is nothing to confirm (the walk goes on).
pub fn confirm(preview: &mut SourcePreview, segments: &mut ProgramSegments) -> bool {
    let sound = segments.part_sound_taken() || segments.overlap_taken();
    let confirmed = segments.confirm_program();
    if sound && confirmed {
        preview.forget_program_draft(); // written: the program opens from the story
    }
    confirmed
}

/// I/O, Ctrl+I/O and Ctrl+Shift+I/O on the segment panel. I and O do nothing here; Ctrl+I,
/// Ctrl+O take the start, the end of the slot under the Wrap playhead (user 2026-10-13: slots
/// are the montage); Ctrl+Shift+I, O the start, the end of the segment, whose clip opens on
/// that edge with its mark taken on the source timeline (user 2026-10-10).
pub fn marks(
    action_id: &str,
    segments: &mut ProgramSegments,
    source: &mut qnc_source_timeline::SourceTimeline,
) -> ControlFlow<Option<bool>> {
    ControlFlow::Break(Some(match action_id {
        "mark_in" | "mark_out" => false,
        "select_mark_in" | "select_mark_out" => segments.take_slot_edge_at_playhead(action_id == "select_mark_in"),
        "select_segment_start" | "select_segment_end" => {
            let taken = segments.take_edge(action_id == "select_segment_start");
            source.set_mark_taken(taken);
            taken
        }
        _ => return ControlFlow::Continue(()),
    }))
}

/// The left/right arrow on the segment panel: what it took moves, else the Wrap playhead.
/// Outside Wrap (the source was last) the step is a click on the Wrap row one frame from
/// the Wrap playhead, which brings Wrap back (live 2026-10-20: the wheel over the Wrap rows
/// did nothing after the source timeline had been used).
pub fn step(action_id: &str, wrap: &mut WrapSession, segments: &mut ProgramSegments) -> bool {
    let frames = if action_id == "step_back_frame" { -1 } else { 1 };
    if segments.handles(action_id) {
        segments.apply_action(action_id)
    } else if wrap.is_active() {
        wrap.step(frames)
    } else if segments.view().is_empty() {
        false
    } else {
        let frame = segments.view().playhead.unwrap_or(0).saturating_add_signed(frames);
        segments.apply(SegmentCommand::Cue(frame))
    }
}

/// The taken lane of the Wrap row in the status line, in the timeline's words.
fn say_part_lane(segments: &mut ProgramSegments) {
    if let Some(message) = segments.part_sound_lanes().map(|lanes| qnc_timeline::lane_message(lanes)) {
        segments.say(message);
    }
}

/// The sound drafted on the Wrap row, as the program plays it before Enter.
fn sound_draft(segments: &ProgramSegments) -> Option<SoundDraft> {
    let (lane, id, draft) = segments.part_sound_draft_now()?;
    Some(SoundDraft { lane, id, channel: draft.channel, level_db: draft.level_db, off: draft.off, overlap: None })
}

/// The stored sound of the part taken on the Wrap row.
fn stored_sound(segments: &ProgramSegments) -> Option<SoundDraft> {
    let (lane, id, stored) = segments.part_sound_stored()?;
    Some(SoundDraft { lane, id, channel: stored.channel, level_db: stored.level_db, off: stored.off, overlap: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i_and_o_do_nothing_on_the_segment_panel_and_its_sound_keys_wait_for_a_taken_sound() {
        let (mut preview, mut wrap, mut segments) = (SourcePreview::default(), WrapSession::new(), ProgramSegments::new());
        let mut source = qnc_source_timeline::SourceTimeline::default();
        assert_eq!(marks("mark_in", &mut segments, &mut source), ControlFlow::Break(Some(false)));
        assert_eq!(marks("play_pause", &mut segments, &mut source), ControlFlow::Continue(()));
        assert_eq!(sound("step_forward_frame", (&mut preview, &mut wrap, &mut segments)), ControlFlow::Continue(()), "no sound taken");
        assert_eq!(sound("clear_focus", (&mut preview, &mut wrap, &mut segments)), ControlFlow::Continue(()));
    }
}

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

/// F: match frame (`qnc-match-frame`).
pub use qnc_match_frame::MATCH_FRAME;

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
            preview.hear_part_level(None);
            preview.hear_program_draft(None, None, wrap.playhead()); // the stored sound again
            released
        }
        _ => return ControlFlow::Continue(()),
    }))
}

/// The parts of a cut (user 2026-10-21, one cut for every marker and join 2026-10-29). With a
/// cut taken (an M marker or the join of two clips), Ctrl+I/O move only its picture, Ctrl+1 its
/// A1 sound, Ctrl+2 its A2 sound. While a part is taken the arrows (and the wheel) change it,
/// heard and seen at once; Escape drops it and the stored story plays again. `Continue` for
/// any other key.
pub fn overlap(
    action_id: &str,
    (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments),
) -> ControlFlow<Option<bool>> {
    let drafting = segments.cut_picture_taken() || segments.cut_sound_taken();
    let done = match action_id {
        // With the cut's sound taken: Ctrl+I its J (the sound after starts), Ctrl+O its L (the
        // sound before ends), user 2026-10-29; else the picture alone.
        side @ ("select_mark_in" | "select_mark_out") if segments.cut_sound_taken() => segments.take_sound_edge(side == "select_mark_in"),
        "select_mark_in" | "select_mark_out" if segments.editing_marker() => segments.take_cut_part(None, true, 0),
        lane @ ("select_audio_a1" | "select_audio_a2") if segments.editing_marker() => {
            segments.take_cut_part(None, false, u8::from(lane == "select_audio_a2"))
        }
        "step_back_frame" | "step_forward_frame" | "clear_focus" if drafting => segments.apply_action(action_id),
        _ => return ControlFlow::Continue(()),
    };
    preview.hear_program_draft(overlap_draft(segments), None, wrap.playhead());
    ControlFlow::Break(Some(done))
}

/// A click or a drag on a side of the taken cut's A2 sound (user 2026-10-29): taken or moved,
/// heard at once as the arrows are.
pub fn cut_edge(
    command: qnc_program_segments::SegmentCommand,
    (preview, wrap, segments): (&mut SourcePreview, &mut WrapSession, &mut ProgramSegments),
) -> bool {
    let done = segments.apply(command);
    preview.hear_program_draft(overlap_draft(segments), None, wrap.playhead());
    done
}

/// The picture J or the cut's sound drafted on the Wrap row, as the program plays it before Enter.
fn overlap_draft(segments: &ProgramSegments) -> Option<SoundDraft> {
    if let Some(covers) = segments.cut_sound_draft() {
        return Some(SoundDraft { lane: 1, id: String::new(), channel: 0, level_db: 0, off: false, overlaps: covers, pictures: Vec::new(), joins: Vec::new() });
    }
    // The A1 sound of a cut between two clips (user 2026-10-29).
    if let Some(joins) = segments.join_sound_draft() {
        return Some(SoundDraft { lane: 1, id: String::new(), channel: 0, level_db: 0, off: false, overlaps: Vec::new(), pictures: Vec::new(), joins });
    }
    // The picture of a cut between two clips (user 2026-10-29).
    let pictures = segments.join_picture_draft()?;
    Some(SoundDraft { lane: 1, id: String::new(), channel: 0, level_db: 0, off: false, overlaps: Vec::new(), pictures, joins: Vec::new() })
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
    // Only its level or off changed: the player hears that part alone at once, continuous
    // with the wheel (user 2026-10-22); another channel opens the program with the draft.
    let (draft, stored) = (sound_draft(segments), stored_sound(segments));
    let live = match (&draft, &stored, part_span(segments)) {
        (Some(draft), Some(stored), Some((output, from, to))) if draft.channel == stored.channel && !stored.off && !preview.program_drafted() => {
            preview.hear_part_level(Some((output, from, to, i16::from(draft.level_db) - i16::from(stored.level_db), draft.off)))
        }
        _ => false,
    };
    if !live {
        preview.hear_part_level(None);
        preview.hear_program_draft(draft, stored, wrap.playhead());
    }
    changed
}

/// The output and program frames of the part taken on the Wrap row: a segment on A1, a
/// cover on A2.
fn part_span(segments: &ProgramSegments) -> Option<(u16, u64, u64)> {
    let (lane, id, _) = segments.part_sound_draft_now()?;
    let view = segments.view();
    if lane == 0 {
        view.rows.iter().find(|row| row.segment_id == id).map(|row| (0, row.start_frame, row.end_frame))
    } else {
        view.covers.iter().find(|cover| cover.cover_id == id).map(|cover| (1, cover.start_frame, cover.end_frame))
    }
}

/// The keyboard left the segment panel (a click, Tab or Enter): what it took is dropped and
/// the stored sound is heard again. True when an edit was dropped.
pub fn left(preview: &mut SourcePreview, wrap: &WrapSession, segments: &mut ProgramSegments) -> bool {
    if segments.part_sound_taken() || segments.cut_picture_taken() || segments.cut_sound_taken() {
        segments.release_part_sound();
        preview.hear_part_level(None);
        preview.hear_program_draft(None, None, wrap.playhead());
    }
    segments.leave_panel()
}

/// Another edit took over from a sound draft: the stored sound is heard again.
pub fn dropped(preview: &mut SourcePreview, wrap: &WrapSession, segments: &mut ProgramSegments) {
    if segments.take_dropped_sound() {
        preview.hear_part_level(None);
        preview.hear_program_draft(None, None, wrap.playhead());
    }
}

/// Enter on the segment panel confirms what it took (a marker, slot, slot edge, sound);
/// false when there is nothing to confirm (the walk goes on).
pub fn confirm(preview: &mut SourcePreview, segments: &mut ProgramSegments) -> bool {
    let sound = segments.part_sound_taken() || segments.cut_picture_taken() || segments.cut_sound_taken();
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
    let frames = qnc_edit_step::frames(action_id).unwrap_or(1);
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

/// The Export button opens the box of the project's export settings (user 2026-10-24); while
/// it is open the keys are its own (up/down choose, left/right the PAL and NTSC tabs, Enter
/// exports, Escape closes). `None` for any other key; a pointer event (`pointer`) goes on to
/// its panel.
pub fn export(action_id: &str, pointer: bool, preview: &SourcePreview, segments: &mut ProgramSegments) -> Option<bool> {
    if action_id == "export_open" {
        if let Some(export) = preview.export_settings() {
            segments.open_export(&export, preview.story_outputs());
        }
        return Some(true);
    }
    if !segments.export_open() || pointer {
        return None;
    }
    match action_id {
        "step_prev_part" | "step_prev_segment" | "page_prev" => segments.export_step(true),
        "step_next_part" | "step_next_segment" | "page_next" => segments.export_step(false),
        // Left and right: the PAL and NTSC tabs (user 2026-10-25).
        "step_back_frame" => segments.export_tab(false),
        "step_forward_frame" => segments.export_tab(true),
        "activate_focused_item" => _ = segments.apply(SegmentCommand::ExportConfirm),
        "clear_focus" | "close_player" => _ = segments.apply(SegmentCommand::ExportCancel),
        _ => {}
    }
    Some(true)
}

/// Where match frame took the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matched {
    /// The clip and frame of the story opened on the source timeline.
    Source(bool),
    /// The story's place of the source picture is the Wrap playhead.
    Story(bool),
}

/// F (match frame, `qnc-match-frame`, docs/98 1.3): from the segment panel (`on_segments`) the
/// clip and frame under the Wrap playhead open on the source timeline; elsewhere the story's
/// place of the source picture shown becomes the Wrap playhead (F again: the next place it is
/// used). `None` when there is no such picture.
pub fn match_frame(on_segments: bool, preview: &mut SourcePreview, wrap: &mut WrapSession, segments: &mut ProgramSegments) -> Option<Matched> {
    let view = segments.view();
    // Covers first: a cover is over its segment.
    let mut pieces: Vec<qnc_match_frame::Piece<'_>> = view.covers.iter().map(|cover| qnc_match_frame::Piece { clip_id: &cover.clip_id, source_in: cover.source_in_frame, source_out: cover.source_out_frame, start: cover.start_frame, end: cover.end_frame }).collect();
    pieces.extend(view.rows.iter().filter(|row| !row.clip_id.is_empty()).map(|row| qnc_match_frame::Piece { clip_id: &row.clip_id, source_in: row.source_in_frame, source_out: row.source_out_frame, start: row.start_frame, end: row.end_frame }));
    if on_segments {
        let (clip, frame) = qnc_match_frame::source_at(&pieces, wrap.playhead()).map(|(clip, frame)| (clip.to_string(), frame))?;
        wrap.leave();
        return Some(Matched::Source(preview.open_at(&clip, frame) || preview.cue(frame)));
    }
    let (Some(clip), Some(frame)) = (preview.view().clip_id.clone(), preview.clip_frame()) else { return None };
    let at = qnc_match_frame::story_at(&pieces, &clip, frame, wrap.playhead() + 1)?;
    Some(Matched::Story(segments.apply(SegmentCommand::Cue(at))))
}

/// A click on a word in the zoom of the Wrap rows (user 2026-11-04): the taken cut goes to the
/// nearer edge of the word (`qnc-edit-words`) by the same steps the arrows give it, so every
/// kind of taken cut (marker, slot edge, J/L) keeps its rules. The arrow and how many times;
/// `None` without a zoom.
pub fn word_steps(segments: &ProgramSegments, from: u64, to: u64) -> Option<(&'static str, u64)> {
    let cut = segments.view().zoom_sound.as_ref()?.cut;
    let goal = qnc_edit_words::Word::new(from, to).goal(None, cut);
    Some((if goal > cut { "step_forward_frame" } else { "step_back_frame" }, goal.abs_diff(cut).min(500)))
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
    Some(SoundDraft { lane, id, channel: draft.channel, level_db: draft.level_db, off: draft.off, overlaps: Vec::new(), pictures: Vec::new(), joins: Vec::new() })
}

/// The stored sound of the part taken on the Wrap row.
fn stored_sound(segments: &ProgramSegments) -> Option<SoundDraft> {
    let (lane, id, stored) = segments.part_sound_stored()?;
    Some(SoundDraft { lane, id, channel: stored.channel, level_db: stored.level_db, off: stored.off, overlaps: Vec::new(), pictures: Vec::new(), joins: Vec::new() })
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

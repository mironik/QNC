//! The SubSegment panel (user 2026-10-29): a statement put together from several ranges, open in
//! place of the source timeline. Its parts lie one after another on the one public
//! `qnc-timeline` as the Broadcast Player plays them (a pause is a gap of black and silence); a
//! line marks every join. Under it, for every join, its pause in frames (one frame shorter or
//! longer) and whether its sound is cut or crossed, "we will test which is better". Passive:
//! it paints the script's view and gives catalog actions; it never plays, reads or writes.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_script::{SubsegmentView, DRAG_EDGE, JOIN_CUT, JOIN_GAP, PART_CHANNEL, PART_LEVEL, PART_MOVE, PART_OFF, PART_REMOVE, SUBSEGMENT_CUE, TAKE_EDGE};
use qnc_timeline::{
    AudioLane, TimelineFocusPaint, TimelineInput, TimelineIntent, TimelineLayerFlags, TimelineMarkerPart, TimelineMarkerPin,
    TimelineMetrics, TimelinePicture, TimelineProjection, TimelineTheme, TimelineVirtualSpan,
};

pub const MODULE_ID: &str = "qnc.module.subsegment-panel";

/// The SubSegment as a body a dock draws under its header in place of its timeline; `None`
/// when no SubSegment is open.
pub fn body<'a>(subsegment: Option<&'a SubsegmentView>, theme: TimelineTheme) -> Option<impl FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a> {
    subsegment.map(move |view| move |ui: &mut egui::Ui| show(ui, view, theme))
}

/// The dock body of whatever is open in place of the source timeline: `first` (the enlarged
/// slot of a story), else the SubSegment.
pub fn or_body<'a>(
    first: Option<impl FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a>,
    subsegment: Option<&'a SubsegmentView>,
    theme: TimelineTheme,
) -> Option<Box<dyn FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a>> {
    match first {
        Some(first) => Some(Box::new(first)),
        None => body(subsegment, theme).map(|body| Box::new(body) as Box<dyn FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a>),
    }
}

/// Paints the SubSegment into the available rectangle.
pub fn show(ui: &mut egui::Ui, view: &SubsegmentView, theme: TimelineTheme) -> Option<ActionEvent> {
    let rect = ui.available_rect_before_wrap();
    ui.painter().rect_filled(rect, 0.0, theme.background);
    // No title row above the timeline (user 2026-10-29: not needed); a clip of the pool brings
    // the source timeline back.
    let mut answer = None;
    let duration = view.total_frames.max(1);
    let mut state = TimelineProjection::new(0, duration).with_cue_enabled(true);
    if let Some(frame) = view.playhead {
        state = state.with_playhead(frame.min(duration));
    }
    let labels: Vec<String> = view.parts.iter().map(|part| format!("{} {}", part.clip_name, part.length)).collect();
    let ids: Vec<String> = (0..view.parts.len()).map(|index| format!("part-{index}")).collect();
    let spans: Vec<TimelineVirtualSpan<'_>> = view
        .parts
        .iter()
        .zip(&labels)
        .zip(&ids)
        .map(|((part, label), id)| TimelineVirtualSpan { id, label, start_frame: part.start, end_frame: part.end, has_base_video: true, selected: false })
        .collect();
    // A line on every edge (the IN and OUT of every part): a right click near one takes it.
    let edge_ids: Vec<String> = (0..view.parts.len()).flat_map(|index| [format!("edge-{index}-in"), format!("edge-{index}-out")]).collect();
    let markers: Vec<TimelineMarkerPin<'_>> = view
        .parts
        .iter()
        .enumerate()
        .flat_map(|(index, part)| [(index, false, part.start), (index, true, part.end)])
        .zip(&edge_ids)
        // The taken edge is the one a drag drags (the mouse rule: only what is taken moves).
        .map(|((index, out, frame), id)| {
            let taken = view.trim.is_some_and(|(at, trim_out, _)| at == index && trim_out == out);
            TimelineMarkerPin { id, frame, taken, locked: false, part: TimelineMarkerPart::Whole, origin: None, sound_tail: None, sound_lead: None }
        })
        .collect();
    // The edge being trimmed, as the trim playhead of a cover's edge.
    let trim_label = qnc_ui_text::text("subsegment.trimming");
    let trim: Vec<(u64, &str)> = view.trim.map(|(_, _, frame)| (frame, trim_label)).into_iter().collect();
    // The parts' filmstrip on V1, as a Wrap row shows its segments (user 2026-10-29).
    let pictures: Vec<TimelinePicture<'_>> = view
        .pictures
        .iter()
        .filter_map(|picture| {
            Some(TimelinePicture { start_frame: picture.program_start, end_frame: picture.program_end, frame: picture.strip.frames.get(picture.frame)?, strip: &picture.strip })
        })
        .collect();
    let intent = ui
        .push_id("qnc_subsegment_timeline", |ui| {
            qnc_timeline::show(
                ui,
                TimelineInput {
                    state: &state,
                    layers: TimelineLayerFlags::a1_v_a2().with_overlays().with_audio_bands(),
                    metrics: TimelineMetrics::default(),
                    theme,
                    expanded_audio: AudioLane::None,
                    focus: TimelineFocusPaint::Playhead,
                    show_lane_labels: true,
                    shot_in_frame: 0,
                    shot_out_frame: duration,
                    draft_in_frame: 0,
                    draft_out_frame: duration,
                    draft_in_active: false,
                    draft_out_active: false,
                    a1_peaks: &view.a1,
                    a2_peaks: &[],
                    a3_peaks: &[],
                    a4_peaks: &[],
                    virtual_spans: &spans,
                    covers: &[],
                    marker_slots: &[],
                    markers: &markers,
                    base_video_blank: false,
                    filmstrip_background: None,
                    video_background: None,
                    a1_channel: None,
                    a2_channel: None,
                    wave_over_video: None,
                    trim_playheads: &trim,
                    a1_splits: &[],
                    pictures: &pictures,
                    holding: false,
                },
            )
        })
        .inner;
    // A drag of the taken edge (user 2026-10-29): the edge follows the pointer as a draft;
    // letting go keeps it, as Enter.
    let dragging = egui::Id::new("qnc_subsegment_edge_drag");
    let dragged = qnc_timeline::take_marker_drag(ui.ctx()).and_then(|(id, frame)| drag_target(&id, frame));
    let was_dragging = ui.ctx().data(|d| d.get_temp::<bool>(dragging)).unwrap_or(false);
    ui.ctx().data_mut(|d| d.insert_temp(dragging, dragged.is_some() || (was_dragging && ui.ctx().input(|i| i.pointer.primary_down()))));
    let drag_event = match dragged {
        Some(target) => Some(ActionEvent::key(DRAG_EDGE).with_target(target)),
        None if was_dragging && !ui.ctx().input(|i| i.pointer.primary_down()) => Some(ActionEvent::key("activate_focused_item")),
        None => None,
    };
    answer = answer.or(drag_event);
    let event = match intent {
        // A click on a part's picture is a click on its row (user 2026-10-29: the filmstrip did
        // not take the playhead): the parts are not chosen, the playhead goes there.
        TimelineIntent::CueFrame(frame) | TimelineIntent::SelectVirtual { frame, .. } => Some(ActionEvent::key(SUBSEGMENT_CUE).with_target(frame.to_string())),
        // A right click on a join takes the IN of the part after it, as Ctrl+I.
        // A right click takes the IN or OUT nearest to where it was (user 2026-10-29: every IN
        // and OUT of every part); at a join the side clicked on decides.
        TimelineIntent::TakeMarker { frame, .. } => nearest_edge(view, frame).map(|(index, out)| ActionEvent::key(TAKE_EDGE).with_target(format!("{index}:{}", if out { "out" } else { "in" }))),
        // The wheel moves the playhead, or the taken edge, a frame as the arrows do.
        TimelineIntent::Wheel(wheel) if !wheel.ctrl => Some(ActionEvent::key(if wheel.up == qnc_wheel_jog::source_step_forward("wheel_source_forward").unwrap_or(true) { "step_forward_frame" } else { "step_back_frame" })),
        // Ctrl+wheel: the level of the taken A1 (Ctrl+1), as on a cover's lane.
        TimelineIntent::Wheel(wheel) => Some(ActionEvent::key(wheel.action(qnc_wheel_jog::SOURCE_ACTIONS))),
        _ => None,
    };
    answer = answer.or(event);
    // The trim zoom of a taken edge, as a cover's: its wheel moves the edge; a double click on
    // a word is not a cut here (the source timeline is not shown).
    let zoomed = view.zoom.as_deref().and_then(|zoom| qnc_trim_overlay::show(ui.ctx(), zoom)).filter(|event| event.action_id != "transcript_cut_word");
    answer = answer.or(zoomed);
    // Every part: the pause and join before it (after the first) and its own A1, the camera
    // channel, level and on/off (user 2026-10-29).
    for (index, part) in view.parts.iter().enumerate() {
        let target = |step: Option<i64>| match step {
            Some(step) => format!("{}:{index}:{step}", view.block_id),
            None => format!("{}:{index}", view.block_id),
        };
        ui.horizontal(|ui| {
            // The part whose A1 is taken (Ctrl+1) is marked as a taken lane is.
            let taken = view.sound == Some(index);
            let name = RichText::new(qnc_ui_text::fill("subsegment.part", &[("n", &(index + 1).to_string()), ("clip", &part.clip_name)]));
            ui.label(if taken { name.color(theme.lane_taken).strong() } else { name.color(theme.text) });
            // Its place among the parts and its leaving (user 2026-10-29).
            let last = index + 1 == view.parts.len();
            if ui.add_enabled(index > 0, egui::Button::new(qnc_ui_text::text("subsegment.move_earlier")).small()).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
                answer = Some(ActionEvent::key(PART_MOVE).with_target(target(Some(-1))));
            }
            if ui.add_enabled(!last, egui::Button::new(qnc_ui_text::text("subsegment.move_later")).small()).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
                answer = Some(ActionEvent::key(PART_MOVE).with_target(target(Some(1))));
            }
            if ui.add_enabled(view.parts.len() > 1, egui::Button::new(qnc_ui_text::text("subsegment.remove")).small()).on_hover_text(qnc_ui_text::text("subsegment.remove_hint")).clicked() {
                answer = Some(ActionEvent::key(PART_REMOVE).with_target(target(None)));
            }
            ui.separator();
            if index > 0 {
                ui.label(RichText::new(qnc_ui_text::fill("subsegment.join", &[("frames", &part.gap_before.to_string())])).color(theme.text));
                if ui.small_button("-").clicked() && part.gap_before > 0 {
                    answer = Some(ActionEvent::key(JOIN_GAP).with_target(target(Some(-1))));
                }
                if ui.small_button("+").clicked() {
                    answer = Some(ActionEvent::key(JOIN_GAP).with_target(target(Some(1))));
                }
                let kind = qnc_ui_text::text(if part.cut { "subsegment.cut" } else { "subsegment.crossfade" });
                if ui.small_button(kind).on_hover_text(qnc_ui_text::text("subsegment.join_hint")).clicked() {
                    answer = Some(ActionEvent::key(JOIN_CUT).with_target(target(None)));
                }
                ui.separator();
            }
            ui.label(RichText::new("A1").color(theme.text)).on_hover_text(qnc_ui_text::text("subsegment.channel_hint"));
            for channel in 0..part.channels.unwrap_or(2).max(1) {
                let chosen = part.a1_channel == Some(channel);
                if ui.selectable_label(chosen, (channel + 1).to_string()).clicked() {
                    answer = Some(ActionEvent::key(PART_CHANNEL).with_target(target(Some(i64::from(channel)))));
                }
            }
            if ui.small_button("-").clicked() {
                answer = Some(ActionEvent::key(PART_LEVEL).with_target(target(Some(-1))));
            }
            ui.label(RichText::new(format!("{:+} dB", part.a1_level_db)).color(theme.text));
            if ui.small_button("+").clicked() {
                answer = Some(ActionEvent::key(PART_LEVEL).with_target(target(Some(1))));
            }
            let off = qnc_ui_text::text(if part.a1_off { "subsegment.a1_off" } else { "subsegment.a1_on" });
            if ui.small_button(off).clicked() {
                answer = Some(ActionEvent::key(PART_OFF).with_target(target(None)));
            }
        });
    }
    answer
}

/// The target of a drag of edge marker `id` ("edge-{part}-in|out") to `frame`: `part:in|out:frame`.
pub fn drag_target(id: &str, frame: u64) -> Option<String> {
    let mut parts = id.strip_prefix("edge-")?.split('-');
    let (index, side) = (parts.next()?.parse::<usize>().ok()?, parts.next()?);
    matches!(side, "in" | "out").then(|| format!("{index}:{side}:{frame}"))
}

/// The edge nearest to `frame`: its part and whether it is the OUT. An OUT lies past the part's
/// last frame, an IN on its first; where an OUT and the next IN meet (a join without a pause),
/// the frame before the join takes the OUT, the frame on or after it the IN.
pub fn nearest_edge(view: &SubsegmentView, frame: u64) -> Option<(usize, bool)> {
    view.parts
        .iter()
        .enumerate()
        .flat_map(|(index, part)| [(index, false, part.start), (index, true, part.end)])
        .min_by_key(|(_, out, at)| (frame.abs_diff(*at), (*out && frame >= *at) || (!*out && frame < *at)))
        .map(|(index, out, _)| (index, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_script::SubsegmentPart;

    fn part(start: u64, end: u64) -> SubsegmentPart {
        SubsegmentPart { start, end, clip_name: String::new(), length: String::new(), gap_before: 0, cut: false, a1_channel: None, a1_level_db: 0, a1_off: false, channels: None }
    }

    #[test]
    fn a_right_click_takes_every_in_and_out_of_every_part() {
        let view = SubsegmentView { parts: vec![part(0, 50), part(50, 90), part(95, 145)], total_frames: 145, ..Default::default() };
        assert_eq!(nearest_edge(&view, 2), Some((0, false)), "the first part's IN");
        assert_eq!(nearest_edge(&view, 48), Some((0, true)), "the first part's OUT, before the join");
        assert_eq!(nearest_edge(&view, 52), Some((1, false)), "the next part's IN, after the join");
        assert_eq!(nearest_edge(&view, 91), Some((1, true)), "an OUT before a pause");
        assert_eq!(nearest_edge(&view, 94), Some((2, false)), "an IN after a pause");
        assert_eq!(nearest_edge(&view, 140), Some((2, true)), "the last part's OUT");
    }

    #[test]
    fn a_drag_of_an_edge_names_its_part_side_and_frame() {
        assert_eq!(drag_target("edge-2-out", 120).as_deref(), Some("2:out:120"));
        assert_eq!(drag_target("edge-0-in", 3).as_deref(), Some("0:in:3"));
        assert_eq!(drag_target("part-1", 3), None);
    }
}

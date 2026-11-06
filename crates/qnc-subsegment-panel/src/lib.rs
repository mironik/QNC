//! The SubSegment panel (user 2026-10-29): a statement put together from several ranges, open in
//! place of the source timeline. Its parts lie one after another on the one public
//! `qnc-timeline` as the Broadcast Player plays them (a pause is a gap of black and silence); a
//! line marks every join. Under it, for every join, its pause in frames (one frame shorter or
//! longer) and whether its sound is cut or crossed, "we will test which is better". Passive:
//! it paints the script's view and gives catalog actions; it never plays, reads or writes.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_segment_panel::{cover_fill, CoverLengthBand};
use qnc_script::{SubsegmentView, JOIN_CUT, JOIN_GAP, PART_CHANNEL, PART_LEVEL, PART_MOVE, PART_OFF, PART_REMOVE};
use qnc_timeline::{
    AudioLane, TimelineCoverSpan, TimelineFocusPaint, TimelineInput, TimelineLayerFlags, TimelineMarkerPart, TimelineMarkerPin,
    TimelineMetrics, TimelinePicture, TimelineProjection, TimelineTheme, TimelineVirtualSpan,
};

pub const MODULE_ID: &str = "qnc.module.subsegment-panel";

/// The SubSegment as a body a dock draws under its header in place of its timeline; `None`
/// when no SubSegment is open.
pub fn body<'a>(subsegment: Option<&'a SubsegmentView>, theme: TimelineTheme, lengths: &'a [CoverLengthBand]) -> Option<impl FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a> {
    subsegment.map(move |view| move |ui: &mut egui::Ui| show(ui, view, theme, lengths))
}

/// The dock body of whatever is open in place of the source timeline: `first` (the enlarged
/// slot of a story), else the SubSegment.
pub fn or_body<'a>(
    first: Option<impl FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a>,
    subsegment: Option<&'a SubsegmentView>,
    theme: TimelineTheme,
    lengths: &'a [CoverLengthBand],
) -> Option<Box<dyn FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a>> {
    match first {
        Some(first) => Some(Box::new(first)),
        None => body(subsegment, theme, lengths).map(|body| Box::new(body) as Box<dyn FnMut(&mut egui::Ui) -> Option<ActionEvent> + 'a>),
    }
}

/// Paints the SubSegment into the available rectangle.
pub fn show(ui: &mut egui::Ui, view: &SubsegmentView, theme: TimelineTheme, lengths: &[CoverLengthBand]) -> Option<ActionEvent> {
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
    // Every part laid out as a cover is (user 2026-11-03): a strip over its pictures in the
    // colour of its length, with its length, always shown; the part being trimmed or heard is
    // the chosen one.
    let lengths_of: Vec<String> = view
        .parts
        .iter()
        .map(|part| match view.rate {
            Some((num, den)) => qnc_program_view::duration_label(part.end - part.start, num, den),
            None => part.length.clone(),
        })
        .collect();
    let covers: Vec<TimelineCoverSpan<'_>> = view
        .parts
        .iter()
        .enumerate()
        .zip(&ids)
        .zip(&lengths_of)
        .map(|(((index, part), id), length)| TimelineCoverSpan {
            id,
            start_frame: part.start,
            end_frame: part.end,
            selected: view.chosen == Some(index) || view.slide == Some(index) || view.trim.is_some_and(|(at, _, _)| at == index) || view.sound == Some(index),
            pending: false,
            fill: cover_fill(part.end - part.start, view.rate, lengths),
            label: Some(length.as_str()),
        })
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
            let taken = view.trim.is_some_and(|(at, trim_out, _)| (at == index && trim_out == out) || (view.roll && !out && index == at + 1));
            // The J and L of the join before a part (user 2026-11-03), on the IN of the part
            // after it, drawn red on A1 as at a story's cut: where the sound before ends and where
            // the sound after starts.
            let part = &view.parts[index];
            // A side either way of the cut: a negative one is a hole (level 0 between the sounds).
            let split = !out && (part.a1_lead != 0 || part.a1_tail != 0);
            let at = |shift: i64| (frame as i64 + shift).max(0) as u64;
            let (a1_lead, a1_tail) = if split { (Some(at(-part.a1_lead)), Some(at(part.a1_tail))) } else { (None, None) };
            let taken_part = match view.join_sound {
                Some((at, side)) if at == index && !out => match side {
                    qnc_script::JoinSide::Both => TimelineMarkerPart::Sound,
                    qnc_script::JoinSide::Lead => TimelineMarkerPart::SoundLead,
                    qnc_script::JoinSide::Tail => TimelineMarkerPart::SoundTail,
                },
                _ => TimelineMarkerPart::Whole,
            };
            let sound_taken = taken_part != TimelineMarkerPart::Whole;
            TimelineMarkerPin { id, frame, taken: taken || sound_taken, locked: false, part: taken_part, origin: None, sound_tail: None, sound_lead: None, join: false, a1_tail, a1_lead, sound_lane: 0 }
        })
        .collect();
    // The edge being trimmed, as the trim playhead of a cover's edge.
    let trim_label = qnc_ui_text::text("subsegment.trimming");
    let mut trim: Vec<(u64, &str)> = view.trim.map(|(_, _, frame)| (frame, trim_label)).into_iter().collect();
    // A part taken whole (Ctrl+S): its two edges, as a taken slot's two markers.
    if let Some(part) = view.slide.and_then(|index| view.parts.get(index)) {
        trim.extend([(part.start, trim_label), (part.end.saturating_sub(1), trim_label)]);
    }
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
                    layers: TimelineLayerFlags::a1_v_a2().with_overlays().with_audio_bands().with_covers_shown(),
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
                    covers: &covers,
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
                    level_curves: &[],
                    pictures: &pictures,
                    holding: view.trim.is_some() || view.sound.is_some() || view.join_sound.is_some() || view.slide.is_some(),
                },
            )
        })
        .inner;
    // A drag of the taken edge (user 2026-10-29): the edge follows the pointer as a draft;
    // letting go keeps it, as Enter.
    let dragging = egui::Id::new("qnc_subsegment_edge_drag");
    // A whole cut taken: a drag of either of its edges drags the cut (the OUT before it).
    let rolled = |id: String| match view.trim.filter(|_| view.roll) {
        Some((at, _, _)) if id == format!("edge-{}-in", at + 1) => format!("edge-{at}-out"),
        _ => id,
    };
    // The mouse map of a program's row (AGENTS 0.12, docs/97 step 2): the SubSegment's timeline
    // sends what a Wrap row sends; the script, the owner of its axis, says what it does.
    let axis = |frame: u64| frame;
    let dragged = qnc_timeline::take_marker_drag_event(ui.ctx(), ROW_PANEL, &axis).map(|event| match event.target.clone() {
        Some(id) if event.action_id == "timeline_drag_marker" => ActionEvent { target: Some(rolled(id)), ..event },
        _ => event,
    });
    let was_dragging = ui.ctx().data(|d| d.get_temp::<bool>(dragging)).unwrap_or(false);
    ui.ctx().data_mut(|d| d.insert_temp(dragging, dragged.is_some() || (was_dragging && ui.ctx().input(|i| i.pointer.primary_down()))));
    // Letting go of a drag keeps it, as Enter.
    let drag_event = match dragged {
        Some(event) => Some(event),
        None if was_dragging && !ui.ctx().input(|i| i.pointer.primary_down()) => Some(ActionEvent::key("activate_focused_item")),
        None => None,
    };
    answer = answer.or(drag_event);
    answer = answer.or_else(|| qnc_timeline::event(&intent, ROW_PANEL, &axis).filter(|event| qnc_script::ROW_ACTIONS.contains(&event.action_id.as_str())));
    // The trim zoom of a taken edge, as a cover's: its wheel moves the edge; a click on a word
    // moves the taken edge to it (user 2026-11-04), on the SubSegment's axis.
    // The strips under its pictures in the colour of their length, as the timeline's (the one
    // coloured zoom of every timeline, on the SubSegment's frame rate).
    let zoom = view.zoom.as_deref().map(|zoom| qnc_program_view::SegmentsView { timebase: zoom.timebase.or(view.rate), ..zoom.clone() });
    let zoomed = zoom.as_ref().and_then(|zoom| qnc_segment_panel::trim_zoom(ui.ctx(), zoom, lengths)).map(|event| match event.action_id.as_str() {
        "transcript_cut_word" => ActionEvent { action_id: qnc_script::ZOOM_WORD.to_string(), ..event },
        // A picture or its strip: the timeline row's action, on the row's panel (the zoom is the
        // timeline enlarged).
        _ if qnc_script::ROW_ACTIONS.contains(&event.action_id.as_str()) || event.action_id == qnc_script::SUBSEGMENT_SELECT => event.at(ROW_PANEL),
        _ => event,
    });
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
            if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.move_earlier"), index > 0, false).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
                answer = Some(ActionEvent::key(PART_MOVE).with_target(target(Some(-1))));
            }
            if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.move_later"), !last, false).on_hover_text(qnc_ui_text::text("subsegment.move_hint")).clicked() {
                answer = Some(ActionEvent::key(PART_MOVE).with_target(target(Some(1))));
            }
            if qnc_ui_kit::small_button(ui, qnc_ui_text::text("subsegment.remove"), view.parts.len() > 1, false).on_hover_text(qnc_ui_text::text("subsegment.remove_hint")).clicked() {
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

/// The mouse map's panel of a program's row: the SubSegment's timeline is one (AGENTS 0.12).
const ROW_PANEL: &str = "segments";

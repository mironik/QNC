//! The SubSegment panel (user 2026-10-29): a statement put together from several ranges, open in
//! place of the source timeline. Its parts lie one after another on the one public
//! `qnc-timeline` as the Broadcast Player plays them (a pause is a gap of black and silence); a
//! line marks every join. Under it, for every join, its pause in frames (one frame shorter or
//! longer) and whether its sound is cut or crossed, "we will test which is better". Passive:
//! it paints the script's view and gives catalog actions; it never plays, reads or writes.

use eframe::egui::{self, RichText};
use qnc_action_event::ActionEvent;
use qnc_script::{SubsegmentView, JOIN_CUT, JOIN_GAP, SUBSEGMENT_CLOSE, SUBSEGMENT_CUE, TAKE_EDGE};
use qnc_timeline::{
    AudioLane, TimelineFocusPaint, TimelineInput, TimelineIntent, TimelineLayerFlags, TimelineMarkerPart, TimelineMarkerPin,
    TimelineMetrics, TimelineProjection, TimelineTheme, TimelineVirtualSpan,
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
    let mut answer = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(qnc_ui_text::fill("subsegment.title", &[("parts", &view.parts.len().to_string()), ("frames", &view.total_frames.to_string())])).color(theme.text));
        if ui.small_button(qnc_ui_text::text("subsegment.close")).clicked() {
            answer = Some(ActionEvent::key(SUBSEGMENT_CLOSE));
        }
    });
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
    let markers: Vec<TimelineMarkerPin<'_>> = view
        .parts
        .iter()
        .zip(&ids)
        .skip(1)
        .map(|(part, id)| TimelineMarkerPin { id, frame: part.start, taken: false, locked: false, part: TimelineMarkerPart::Whole, origin: None, sound_at: None })
        .collect();
    // The edge being trimmed, as the trim playhead of a cover's edge.
    let trim_label = qnc_ui_text::text("subsegment.trimming");
    let trim: Vec<(u64, &str)> = view.trim.map(|(_, _, frame)| (frame, trim_label)).into_iter().collect();
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
                    pictures: &[],
                    holding: false,
                },
            )
        })
        .inner;
    let event = match intent {
        TimelineIntent::CueFrame(frame) => Some(ActionEvent::key(SUBSEGMENT_CUE).with_target(frame.to_string())),
        // A right click on a join takes the IN of the part after it, as Ctrl+I.
        TimelineIntent::TakeMarker { id, .. } => id.strip_prefix("part-").map(|index| ActionEvent::key(TAKE_EDGE).with_target(format!("{index}:in"))),
        // The wheel moves the playhead, or the taken edge, a frame as the arrows do.
        TimelineIntent::Wheel(wheel) if !wheel.ctrl => Some(ActionEvent::key(if wheel.up == qnc_wheel_jog::source_step_forward("wheel_source_forward").unwrap_or(true) { "step_forward_frame" } else { "step_back_frame" })),
        _ => None,
    };
    answer = answer.or(event);
    // The trim zoom of a taken edge, as a cover's: its wheel moves the edge; a double click on
    // a word is not a cut here (the source timeline is not shown).
    let zoomed = view.zoom.as_deref().and_then(|zoom| qnc_trim_overlay::show(ui.ctx(), zoom)).filter(|event| event.action_id != "transcript_cut_word");
    answer = answer.or(zoomed);
    // The pause and join before every part after the first.
    for (index, part) in view.parts.iter().enumerate().skip(1) {
        let target = |step: Option<i64>| match step {
            Some(step) => format!("{}:{index}:{step}", view.block_id),
            None => format!("{}:{index}", view.block_id),
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(qnc_ui_text::fill("subsegment.join", &[("n", &index.to_string()), ("frames", &part.gap_before.to_string())])).color(theme.text));
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
        });
    }
    answer
}

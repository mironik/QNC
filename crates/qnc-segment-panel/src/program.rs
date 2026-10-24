use super::*;

/// The program between `start` and `end`, as one `qnc-timeline` row.
pub(crate) fn paint_program(
    ui: &mut egui::Ui,
    segments: &SegmentsView,
    (start, end): (u64, u64),
    theme: TimelineTheme,
    lane_labels: bool,
    lengths: &[CoverLengthBand],
    cut_parts: bool,
) -> Option<ActionEvent> {
    let duration = end.saturating_sub(start).max(1);
    let local = |frame: u64| frame.clamp(start, end) - start;
    let mut state = TimelineProjection::new(0, duration).with_cue_enabled(true);
    if let Some(frame) = segments
        .playhead
        .filter(|frame| (start..=end).contains(frame))
    {
        state = state.with_playhead(local(frame));
    }
    let labels = segments
        .rows
        .iter()
        .map(|row| match row.lead {
            // The lead-in and lead-out (user 2026-10-24), glued to the user's segment.
            true => format!("{} {}", qnc_ui_text::text("story.lead"), row.duration_label),
            false => format!("{} {}", row.kind.label(), row.duration_label),
        })
        .collect::<Vec<_>>();
    let spans = segments
        .rows
        .iter()
        .zip(&labels)
        .filter(|(row, _)| row.end_frame > start && row.start_frame < end)
        .map(|(row, label)| TimelineVirtualSpan {
            id: &row.segment_id,
            label,
            start_frame: local(row.start_frame),
            end_frame: local(row.end_frame),
            has_base_video: row.kind.has_base_video(),
            selected: row.selected,
        })
        .collect::<Vec<_>>();
    let slots = segments
        .slots
        .iter()
        .filter(|slot| slot.end_frame > start && slot.start_frame < end)
        .map(|slot| TimelineSlotSpan {
            id: &slot.slot_id,
            start_frame: local(slot.start_frame),
            end_frame: local(slot.end_frame),
            has_cover: slot.has_cover,
            selected: slot.selected,
        })
        .collect::<Vec<_>>();
    // A saved cover shows its whole length (colour and seconds:frames), also where a
    // row shows only part of it (user 2026-10-12).
    let shown = segments
        .covers
        .iter()
        .filter(|cover| cover.end_frame > start && cover.start_frame < end)
        .collect::<Vec<_>>();
    let rate = segments.timebase.unwrap_or((0, 1));
    let labels = shown
        .iter()
        .map(|cover| qnc_program_view::duration_label(cover.end_frame - cover.start_frame, rate.0, rate.1))
        .collect::<Vec<_>>();
    // The cover a Sync is writing, as it grows (user 2026-10-18).
    let draft = segments.sync_draft.filter(|(from, to)| *to > start && *from < end);
    let draft_label = draft.map(|(from, to)| qnc_program_view::duration_label(to - from, rate.0, rate.1));
    let draft_span = draft.zip(draft_label.as_deref()).map(|((from, to), label)| TimelineCoverSpan {
        id: "sync-draft",
        start_frame: local(from),
        end_frame: local(to),
        selected: true,
        pending: true,
        fill: cover_fill(to - from, segments.timebase, lengths),
        label: Some(label),
    });
    let covers = shown
        .iter()
        .zip(&labels)
        .map(|(cover, label)| TimelineCoverSpan {
            id: &cover.cover_id,
            start_frame: local(cover.start_frame),
            end_frame: local(cover.end_frame),
            // The strip of a cover shows while it or its slot is chosen (drawing only).
            selected: cover.selected
                || segments.selected_slot().is_some_and(|slot| slot.slot_id == cover.slot_id)
                || segments.part_sound.as_ref().is_some_and(|part| part.lane == 1 && part.id == cover.cover_id),
            pending: false,
            fill: cover_fill(cover.end_frame - cover.start_frame, segments.timebase, lengths),
            label: Some(label.as_str()),
        })
        .chain(draft_span)
        .collect::<Vec<_>>();
    let markers = segments
        .markers
        .iter()
        // v5 `local_marker_frame`: a marker on a segment border is drawn at the end
        // of the earlier row; only the first row shows frame 0.
        .filter(|pin| (start..=end).contains(&pin.frame) && (pin.frame != start || start == 0))
        .map(|pin| TimelineMarkerPin {
            id: &pin.marker_id,
            frame: local(pin.frame),
            taken: pin.taken,
            locked: pin.locked,
            part: match pin.taken_part {
                qnc_program_view::TakenPart::Whole => qnc_timeline::TimelineMarkerPart::Whole,
                qnc_program_view::TakenPart::Picture => qnc_timeline::TimelineMarkerPart::Picture,
                qnc_program_view::TakenPart::Sound => qnc_timeline::TimelineMarkerPart::Sound,
            },
            origin: pin.origin.filter(|frame| (start..=end).contains(frame)).map(local),
            sound_at: pin.sound_at.filter(|frame| (start..=end).contains(frame)).map(local),
        })
        .collect::<Vec<_>>();
    // v5 `local_peaks_for_row`: this row's part of the program wave.
    let row_wave = |program: &[f32]| qnc_program_waveform::row_peaks(program, segments.total_frames, start, end);
    let (a1, a2) = (row_wave(&segments.peaks.a1), row_wave(&segments.peaks.a2));
    // The trim playheads of the edits being moved, on the row that draws their marker.
    let trim_labels: Vec<(u64, String)> = segments
        .trim_frames
        .iter()
        .filter(|frame| (start..=end).contains(*frame) && (**frame != start || start == 0))
        .map(|frame| (local(*frame), segments.timecode(*frame)))
        .collect();
    let trim_playheads: Vec<(u64, &str)> = trim_labels.iter().map(|(frame, label)| (*frame, label.as_str())).collect();
    // This row's part of the program filmstrip (user 2026-10-15).
    let row_pictures = qnc_program_filmstrip::row_pictures(&segments.pictures, start, end);
    let pictures: Vec<TimelinePicture<'_>> = row_pictures
        .iter()
        .filter_map(|picture| {
            Some(TimelinePicture {
                start_frame: picture.program_start,
                end_frame: picture.program_end,
                frame: picture.strip.frames.get(picture.frame)?,
                strip: &picture.strip,
            })
        })
        .collect();
    let no_picture = segments
        .segment_at(start)
        .is_some_and(|row| !row.kind.has_base_video() && row.end_frame >= end);
    // The sound of the segment or cover taken on this row (user 2026-10-16): the channel
    // column and the lane button show its draft, and its wave follows the drafted level at
    // once (live 2026-10-16: the level was not seen until Enter). The row's wave is drawn
    // at the stored level, so the draft gives the difference.
    let part_choice = |lane: u8| {
        let stored = if lane == 0 {
            segments.part_sound.as_ref().and_then(|part| {
                segments.wrap_rows().into_iter().find(|(window, row)| row.segment_id == part.id && window.0 == start).map(|(_, row)| row.a1_level_db)
            })
        } else {
            segments.part_sound.as_ref().and_then(|part| {
                segments.covers.iter().find(|cover| cover.cover_id == part.id && (start..end).contains(&cover.start_frame)).map(|cover| cover.a2_level_db)
            })
        };
        segments.part_sound.as_ref().filter(|part| part.lane == lane).zip(stored).map(|(part, stored)| {
            let now = qnc_timeline::Lane { channel: part.channel, off: part.off, level_db: part.level_db };
            qnc_timeline::drafted_choice(now, part.channels, true, stored)
        })
    };
    // The A1 split cuts of this row, its own frames.
    let a1_splits = segments
        .a1_splits
        .iter()
        .filter(|(picture, sound)| (start..=end).contains(picture) || (start..=end).contains(sound))
        .map(|&(picture, sound)| (local(picture), local(sound)))
        .collect::<Vec<_>>();
    let intent = qnc_timeline::show(
        ui,
        TimelineInput {
            state: &state,
            layers: layers(segments.video_compact),
            metrics: TimelineMetrics::default(),
            theme,
            expanded_audio: AudioLane::None,
            focus: TimelineFocusPaint::Playhead,
            show_lane_labels: lane_labels,
            shot_in_frame: 0,
            shot_out_frame: duration,
            draft_in_frame: 0,
            draft_out_frame: duration,
            draft_in_active: false,
            draft_out_active: false,
            a1_peaks: &a1,
            a2_peaks: &a2,
            a3_peaks: &[],
            a4_peaks: &[],
            virtual_spans: &spans,
            covers: &covers,
            marker_slots: &slots,
            markers: &markers,
            base_video_blank: no_picture,
            filmstrip_background: None,
            video_background: None,
            a1_channel: part_choice(0),
            a2_channel: part_choice(1),
            // A taken A1 or A2 zooms its lane on every Wrap row (user 2026-10-22), the channel
            // column only on the row of the taken part.
            wave_over_video: segments.part_sound.as_ref().map(|part| (if part.lane == 0 { AudioLane::A1 } else { AudioLane::A2 }, 1)),
            trim_playheads: &trim_playheads,
            a1_splits: &a1_splits,
            pictures: &pictures,
            // Something taken on the row: a left click keeps it first (the mouse map).
            holding: !segments.trim_frames.is_empty() || segments.part_sound.is_some(),
        },
    );
    // The picture or the sound of a marker's cut alone only in the enlarged slot (user
    // 2026-10-22); on a Wrap row a second right click takes the marker whole again.
    let intent = match intent {
        qnc_timeline::TimelineIntent::TakeMarkerPicture { id } | qnc_timeline::TimelineIntent::TakeMarkerSound { id } if !cut_parts => {
            qnc_timeline::TimelineIntent::TakeMarker { id, frame: 0 }
        }
        other => other,
    };
    // A pointer on the row is the action the mouse map gives it on this panel (piece
    // `qnc-mouse-map`, contracts/input/mouse.json; user rule 2026-10-20): the panel decides
    // no click itself.
    let row_frame = |frame: u64| start + frame.min(duration);
    qnc_timeline::take_marker_drag_event(ui.ctx(), PANEL, &row_frame).or_else(|| qnc_timeline::event(&intent, PANEL, &row_frame))
}

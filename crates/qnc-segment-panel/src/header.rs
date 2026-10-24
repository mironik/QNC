use super::*;

/// The header row keeps to the panel's width (user 2026-10-24: at a larger zoom the values,
/// laid right to left, pushed the whole panel over the monitor): the values are clipped
/// after the title, never grow the panel.
pub(crate) fn header(ui: &mut egui::Ui, segments: &SegmentsView, style: SegmentPanelStyle) {
    let height = HEADER_VALUE_SIZE + 10.0;
    let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), egui::Sense::hover());
    let clip = row.intersect(ui.clip_rect());
    let mut left = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
    left.set_clip_rect(clip);
    let title = left.label(RichText::new(qnc_ui_text::text("story.ui.segments")).color(style.text).strong()).rect;
    if segments.is_empty() {
        return;
    }
    let values = egui::Rect::from_min_max(egui::pos2((title.right() + 14.0).min(row.right()), row.top()), row.max);
    let mut right = ui.new_child(egui::UiBuilder::new().max_rect(values).layout(egui::Layout::left_to_right(egui::Align::Center)));
    right.set_clip_rect(values.intersect(clip));
    let ui = &mut right;
    {
        let playhead = segments.playhead.unwrap_or(0);
        let segment_frames = segments
            .segment_at(playhead)
            .or_else(|| segments.selected())
            .map(SegmentRow::duration_frames)
            .unwrap_or(0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            value(ui, &segments.timecode(segments.total_frames), style.total);
            label(ui, qnc_ui_text::text("source.duration"), style.muted);
            ui.add_space(18.0);
            value(ui, &playhead.to_string(), style.playhead);
            label(ui, qnc_ui_text::text("story.ui.frame"), style.muted);
            value(ui, &segments.timecode(playhead), style.playhead);
            label(ui, qnc_ui_text::text("story.ui.playhead"), style.muted);
            ui.add_space(18.0);
            value(ui, &segments.timecode(segment_frames), style.segment);
            label(ui, qnc_ui_text::text("pool.tab.segment"), style.muted);
            // The loudness of A1 and A2 (user 2026-10-24: EBU R 128), NORM while the preview
            // normalises them to the project's targets.
            if let Some(measured) = segments.loudness {
                ui.add_space(18.0);
                if segments.normalize {
                    value(ui, qnc_ui_text::text("story.ui.normalized"), style.playhead);
                }
                let lufs = |value: Option<f64>| value.map_or("--".to_string(), |lufs| format!("{lufs:.1}"));
                let mark = if measured.complete { "" } else { "~" };
                value(ui, &format!("{mark}{} LUFS", lufs(measured.a2_lufs)), style.segment);
                label(ui, "A2", style.muted);
                value(ui, &format!("{mark}{}", lufs(measured.a1_lufs)), style.segment);
                label(ui, "A1", style.muted);
            }
        });
    }
}

pub(crate) fn label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let text = RichText::new(text).color(color).size(HEADER_LABEL_SIZE);
    ui.label(text.strong());
}

pub(crate) fn value(ui: &mut egui::Ui, text: &str, color: Color32) {
    let text = RichText::new(text).color(color).size(HEADER_VALUE_SIZE);
    ui.label(text.strong());
}

use super::*;

pub(crate) fn header(ui: &mut egui::Ui, segments: &SegmentsView, style: SegmentPanelStyle) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(qnc_ui_text::text("story.ui.segments")).color(style.text).strong());
        if segments.is_empty() {
            return;
        }
        let playhead = segments.playhead.unwrap_or(0);
        let segment_frames = segments
            .segment_at(playhead)
            .or_else(|| segments.selected())
            .map(SegmentRow::duration_frames)
            .unwrap_or(0);
        ui.add_space(14.0);
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
        });
    });
}

pub(crate) fn label(ui: &mut egui::Ui, text: &str, color: Color32) {
    let text = RichText::new(text).color(color).size(HEADER_LABEL_SIZE);
    ui.label(text.strong());
}

pub(crate) fn value(ui: &mut egui::Ui, text: &str, color: Color32) {
    let text = RichText::new(text).color(color).size(HEADER_VALUE_SIZE);
    ui.label(text.strong());
}

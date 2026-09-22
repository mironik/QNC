//! The source dock of any form: a frame, a header row (clip label and whatever the form adds)
//! and the source timeline of the shown clip under it. The form passes the label, its own
//! header widgets and the timeline data; this module knows no form and keeps no state.

use eframe::egui::{self, Align, Color32, Layout, Rect, RichText, Stroke, Ui, Vec2};
use qnc_filmstrip::FilmstripBackground;
use qnc_timeline::{TimelineIntent, TimelineProjection, TimelineTheme};

/// Colours and metrics of the dock frame (from the UI contract and the theme).
#[derive(Debug, Clone, Copy)]
pub struct TimelineDockStyle {
    pub fill: Color32,
    pub border: Color32,
    pub text: Color32,
    pub font_ui: f32,
    pub chrome_row_height: f32,
    pub chrome_control_height: f32,
    pub chrome_pad_x: f32,
    pub chrome_pad_y: f32,
    pub header_timeline_gap: f32,
}

/// What the timeline draws: the projection of the shown clip and its artifacts.
pub struct SourceTimeline<'a> {
    pub projection: &'a TimelineProjection,
    pub theme: TimelineTheme,
    pub filmstrip: Option<&'a FilmstripBackground>,
    pub peaks: [&'a [f32]; 4],
}

fn mark_label(ui: &mut Ui, style: &TimelineDockStyle, name: &str, frame: u64) {
    ui.label(
        RichText::new(name)
            .size(style.font_ui)
            .color(style.border),
    );
    ui.label(
        RichText::new(frame.to_string())
            .monospace()
            .strong()
            .size(style.font_ui)
            .color(style.text),
    );
}

/// A chrome row: filled, with a bottom rule, controls laid out left to right.
pub fn show_chrome_row(
    ui: &mut Ui,
    rect: Rect,
    style: &TimelineDockStyle,
    fill: Color32,
    draw_bottom_rule: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    ui.painter().rect_filled(rect, 0.0, fill);
    if draw_bottom_rule {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, style.border),
        );
    }
    let inner = Rect::from_min_max(
        egui::pos2(
            rect.left() + style.chrome_pad_x,
            rect.top() + style.chrome_pad_y,
        ),
        egui::pos2(
            rect.right() - style.chrome_pad_x,
            rect.bottom() - style.chrome_pad_y,
        ),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.set_clip_rect(rect);
            ui.set_min_height(style.chrome_control_height);
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            add_contents(ui);
        },
    );
}

/// Paints the dock into `rect`. `add_header` adds the widgets of the form after the clip label
/// in the header row. Returns the intent of the timeline.
pub fn show_timeline_dock(
    ui: &mut Ui,
    rect: Rect,
    style: &TimelineDockStyle,
    clip_label: &str,
    add_header: impl FnOnce(&mut Ui),
    timeline: SourceTimeline<'_>,
) -> TimelineIntent {
    ui.painter().rect_filled(rect, 0.0, style.fill);
    ui.painter().line_segment(
        [rect.left_top(), rect.right_top()],
        Stroke::new(1.0, style.border),
    );
    let inner = Rect::from_min_max(
        egui::pos2(rect.left() + 8.0, rect.top()),
        egui::pos2(rect.right() - 8.0, rect.bottom()),
    );
    let header_rect = Rect::from_min_size(
        inner.left_top(),
        Vec2::new(inner.width().max(0.0), style.chrome_row_height),
    );
    let timeline_top = header_rect.bottom() + style.header_timeline_gap;
    let timeline_rect = Rect::from_min_size(
        egui::pos2(inner.left(), timeline_top),
        Vec2::new(
            inner.width().max(0.0),
            qnc_timeline::source_player_timeline_height()
                .min((inner.bottom() - timeline_top).max(0.0)),
        ),
    );
    show_chrome_row(ui, header_rect, style, style.fill, true, |ui| {
        ui.label(
            RichText::new(clip_label)
                .color(style.text)
                .strong()
                .size(style.font_ui),
        );
        ui.add_space(10.0);
        if let Some((start, end)) = timeline.projection.visible_source_marks() {
            mark_label(ui, style, "IN", start);
            mark_label(ui, style, "OUT", end);
            mark_label(ui, style, "Trajanje", end.saturating_sub(start));
        }
        add_header(ui);
    });
    qnc_timeline::show_source_player_timeline_with_artifacts(
        ui,
        timeline_rect,
        timeline.projection,
        timeline.theme,
        timeline.filmstrip,
        timeline.peaks[0],
        timeline.peaks[1],
        timeline.peaks[2],
        timeline.peaks[3],
    )
}

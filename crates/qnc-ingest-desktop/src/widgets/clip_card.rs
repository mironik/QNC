use super::*;

pub(super) fn render_clip_card(
    ui: &mut Ui,
    clip: &ClipView,
    focused: bool,
    size: Vec2,
    theme: &Theme,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let stroke = if focused {
        Stroke::new(2.0, theme.danger)
    } else {
        Stroke::new(1.0, theme.border)
    };
    ui.painter().rect_filled(rect, 0.0, theme.surface_alt);
    ui.painter()
        .rect_stroke(rect, 0.0, stroke, StrokeKind::Inside);
    let image_rect = Rect::from_min_size(
        rect.left_top(),
        Vec2::new(rect.width(), rect.width() * 9.0 / 16.0),
    );
    ui.painter()
        .rect_filled(image_rect.shrink(1.0), 0.0, theme.surface);
    if let (Some(uri), Some(image)) = (&clip.thumb_uri, &clip.thumb_image) {
        qnc_ui_kit::paint_rgba_image(
            ui,
            image_rect.shrink(1.0),
            uri,
            image.content_key,
            image.size,
            &image.pixels,
        );
    } else {
        ui.painter().text(
            image_rect.center(),
            Align2::CENTER_CENTER,
            "...",
            FontId::proportional(theme.font_ui),
            theme.text_muted,
        );
    }
    paint_selection_check(ui, image_rect, clip.selected, theme);
    let label = match clip.save_state {
        qnc_ingest_application::SaveState::Pending => Some("Spremanje..."),
        qnc_ingest_application::SaveState::Failed => Some("Upis nije uspio"),
        _ => None,
    };
    if let Some(label) = label {
        let galley = ui.painter().layout_no_wrap(
            label.into(),
            FontId::proportional(theme.font_ui - 2.0),
            theme.text,
        );
        let position = egui::pos2(image_rect.left() + 6.0, image_rect.top() + 4.0);
        ui.painter().rect_filled(
            Rect::from_min_size(position, galley.size()).expand(2.0),
            0.0,
            theme.surface,
        );
        ui.painter().galley(position, galley, theme.text);
    }
    let marker = if clip.imported {
        Color32::from_rgb(55, 210, 145)
    } else {
        theme.text_muted
    };
    ui.painter().circle_filled(
        egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
        4.0,
        marker,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 8.0, image_rect.bottom() + 8.0),
        Align2::LEFT_TOP,
        truncate(
            &clip.name,
            ((rect.width() - 76.0) / 7.0).floor().clamp(8.0, 42.0) as usize,
        ),
        FontId::proportional(theme.font_ui - 1.0),
        theme.text,
    );
    ui.painter().text(
        egui::pos2(rect.right() - 8.0, image_rect.bottom() + 8.0),
        Align2::RIGHT_TOP,
        format_duration(clip.duration_seconds),
        FontId::proportional(theme.font_ui - 1.0),
        theme.text_muted,
    );
    response
}

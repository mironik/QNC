use super::*;

pub(super) fn selection_check_hit_rect(card_rect: Rect) -> Rect {
    selection_check_rect(card_rect).expand(4.0)
}

pub(super) fn selection_check_rect(card_rect: Rect) -> Rect {
    let thumb_height = (card_rect.height() - 34.0).max(72.0);
    let size = 16.0;
    let pad = 6.0;
    Rect::from_min_size(
        egui::pos2(
            card_rect.left() + pad,
            card_rect.top() + thumb_height - pad - size,
        ),
        Vec2::splat(size),
    )
}

pub(super) fn paint_selection_check(ui: &Ui, thumb_rect: Rect, checked: bool, theme: &Theme) {
    let check_rect = selection_check_rect(Rect::from_min_max(
        thumb_rect.left_top(),
        egui::pos2(thumb_rect.right(), thumb_rect.bottom() + 34.0),
    ));
    if checked {
        let fill = Color32::from_rgb(0xff, 0x95, 0x00);
        ui.painter().rect_filled(check_rect, 3.0, fill);
        ui.painter()
            .rect_stroke(check_rect, 3.0, Stroke::new(1.5, fill), StrokeKind::Inside);
        let c = check_rect.center();
        let dark = Color32::from_rgb(0x1a, 0x1a, 0x1a);
        ui.painter().line_segment(
            [egui::pos2(c.x - 3.5, c.y), egui::pos2(c.x - 1.0, c.y + 3.0)],
            Stroke::new(2.0, dark),
        );
        ui.painter().line_segment(
            [
                egui::pos2(c.x - 1.0, c.y + 3.0),
                egui::pos2(c.x + 4.0, c.y - 3.0),
            ],
            Stroke::new(2.0, dark),
        );
    } else {
        ui.painter().rect_filled(
            check_rect,
            3.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 90),
        );
        ui.painter().rect_stroke(
            check_rect,
            3.0,
            Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 255, 255, 140)),
            StrokeKind::Inside,
        );
    }
    let _ = theme;
}

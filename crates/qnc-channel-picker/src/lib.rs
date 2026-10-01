//! The channel picker (user rule 2026-09-30): a thin row of checkboxes 1..N, one per
//! audio channel of the source clip, exactly one chosen. It is drawn right of a lane
//! label at the height of the audio lane. Passive: it draws the choice it is given and
//! answers with the channel clicked; the caller decides what the choice means.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

/// The choice to draw: the chosen channel (zero based) of `count` channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelChoice {
    pub selected: u16,
    pub count: u16,
}

/// Colours of the row.
#[derive(Debug, Clone, Copy)]
pub struct PickerStyle {
    pub fill: Color32,
    pub border: Color32,
    pub text: Color32,
    pub accent: Color32,
}

/// Width of one cell (box and number) at the given row height.
pub fn cell_width(height: f32) -> f32 {
    (height * 2.0).max(28.0)
}

/// The rectangle the row takes, from `left` at the top of `lane`.
pub fn row_rect(lane: Rect, left: f32, choice: ChannelChoice) -> Rect {
    let width = cell_width(lane.height()) * f32::from(choice.count.max(1));
    Rect::from_min_size(egui::pos2(left, lane.top()), Vec2::new(width, lane.height()))
}

/// Draws the row in `rect` and answers with the channel clicked (zero based).
pub fn show(ui: &mut egui::Ui, id: egui::Id, rect: Rect, style: PickerStyle, choice: ChannelChoice) -> Option<u16> {
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, style.fill);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, style.border), StrokeKind::Inside);
    let cell = cell_width(rect.height());
    let side = (rect.height() - 4.0).clamp(6.0, 12.0);
    let mut clicked = None;
    for channel in 0..choice.count {
        let cell_rect = Rect::from_min_size(
            egui::pos2(rect.left() + cell * f32::from(channel), rect.top()),
            Vec2::new(cell, rect.height()),
        );
        let response = ui.interact(cell_rect, id.with(channel), Sense::click());
        let center = cell_rect.left_center() + Vec2::new(side * 0.5 + 3.0, 0.0);
        let check = Rect::from_center_size(center, Vec2::splat(side));
        let chosen = channel == choice.selected;
        let painter = ui.painter();
        painter.rect_stroke(check, 0.0, Stroke::new(1.0, style.text), StrokeKind::Inside);
        if chosen {
            painter.rect_filled(check.shrink(2.0), 0.0, style.accent);
        }
        painter.text(
            egui::pos2(check.right() + 3.0, cell_rect.center().y),
            Align2::LEFT_CENTER,
            (channel + 1).to_string(),
            FontId::proportional(side + 1.0),
            style.text,
        );
        if response.clicked() {
            clicked = Some(channel);
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_row_is_as_high_as_the_lane_and_one_cell_per_channel() {
        let lane = Rect::from_min_size(egui::pos2(10.0, 50.0), Vec2::new(500.0, 15.0));
        let rect = row_rect(lane, 40.0, ChannelChoice { selected: 1, count: 4 });
        assert_eq!(rect.height(), 15.0);
        assert_eq!(rect.top(), 50.0);
        assert_eq!(rect.left(), 40.0);
        assert_eq!(rect.width(), 4.0 * cell_width(15.0));
    }
}

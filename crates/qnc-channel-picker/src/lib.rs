//! The channel picker (user rules 2026-09-30, 2026-10-14): a column of checkboxes 1..N,
//! one under the other, one per audio channel of the source clip, exactly one chosen. It
//! starts at its lane, right of the lane label, and grows over the video row (down from a
//! top lane, up from a bottom lane), never into another audio lane. Passive: it draws the
//! choice it is given and answers with the channel clicked; the caller decides what the
//! choice means. Under the channels is a "0" (user 2026-10-14): level 0, the lane keeps its
//! channel and nothing of it is heard; it is the place `count`.

use eframe::egui::{self, Align2, Color32, FontId, Rect, Sense, Stroke, StrokeKind, Vec2};

/// The choice to draw: the chosen channel (zero based) of `count` channels, or `count`
/// for level 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelChoice {
    pub selected: u16,
    pub count: u16,
    /// Taken with the keyboard: the row is shown, `selected` is the draft.
    pub open: bool,
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

/// The rectangle the column takes from `left`, `width` wide, within `room` (top, bottom):
/// from its top down (`down`, under a top lane) or from its bottom up (over a bottom lane);
/// a cell is `cell` high, lower when the channels would not fit in the room.
pub fn column_rect(left: f32, width: f32, room: (f32, f32), cell: f32, down: bool, choice: ChannelChoice) -> Rect {
    let count = f32::from(places(choice));
    let cell = cell.min((room.1 - room.0).max(1.0) / count);
    let size = Vec2::new(width, cell * count);
    if down {
        Rect::from_min_size(egui::pos2(left, room.0), size)
    } else {
        Rect::from_min_size(egui::pos2(left, room.1 - size.y), size)
    }
}

impl ChannelChoice {
    /// The lane is at level 0: nothing of its channel is heard.
    pub fn silent(&self) -> bool {
        self.selected >= self.count
    }
}

/// The places of the column: every channel and the 0 under them.
fn places(choice: ChannelChoice) -> u16 {
    choice.count + 1
}

/// Draws the row in `rect` and answers with the place clicked (a channel, zero based, or
/// `count` for level 0).
pub fn show(ui: &mut egui::Ui, id: egui::Id, rect: Rect, style: PickerStyle, choice: ChannelChoice) -> Option<u16> {
    // A transparent fill and border draw the boxes straight on what is under them.
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, style.fill);
    if style.border != Color32::TRANSPARENT {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, style.border), StrokeKind::Inside);
    }
    let cell = rect.height() / f32::from(places(choice));
    let side = (cell - 4.0).clamp(6.0, 12.0);
    let mut clicked = None;
    for channel in 0..places(choice) {
        let cell_rect = Rect::from_min_size(
            egui::pos2(rect.left(), rect.top() + cell * f32::from(channel)),
            Vec2::new(rect.width(), cell),
        );
        let response = ui.interact(cell_rect, id.with(channel), Sense::click());
        let center = cell_rect.left_center() + Vec2::new(side * 0.5 + 3.0, 0.0);
        let check = Rect::from_center_size(center, Vec2::splat(side));
        let chosen = channel == choice.selected.min(choice.count);
        let painter = ui.painter();
        painter.rect_stroke(check, 0.0, Stroke::new(1.0, style.text), StrokeKind::Inside);
        if chosen {
            painter.rect_filled(check.shrink(2.0), 0.0, style.accent);
        }
        painter.text(
            egui::pos2(check.right() + 3.0, cell_rect.center().y),
            Align2::LEFT_CENTER,
            // The channels count from 1; the place after them is level 0.
            if channel == choice.count { "0".to_string() } else { (channel + 1).to_string() },
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
    fn the_column_grows_over_the_video_and_never_into_the_other_lane() {
        let choice = ChannelChoice { selected: 1, count: 4, open: true };
        // The video row between A1 (0..18) and A2 (82..100).
        let down = column_rect(40.0, 28.0, (18.0, 82.0), 18.0, true, choice);
        assert_eq!((down.top(), down.height()), (18.0, 64.0), "under A1, lower cells to fit (4 channels and 0)");
        let up = column_rect(40.0, 28.0, (18.0, 82.0), 12.0, false, choice);
        assert_eq!((up.bottom(), up.height()), (82.0, 60.0), "above A2: 4 channels and the 0");
        assert!(ChannelChoice { selected: 4, count: 4, open: false }.silent());
    }
}

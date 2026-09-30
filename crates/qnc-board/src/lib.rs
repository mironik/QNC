//! The one QNC desktop board (user rule 2026-09-30): every application uses the same
//! layout and gives only the sizes of its places and what goes in each one.
//!
//! ```text
//! +-----------+---+--------------------+
//! | monitor   |   |                    |
//! +-----------+ d |                    |
//! | head row  | i |  right             |
//! +-----------+ v |                    |
//! | body      |   |                    |
//! +-----------+---+--------------------+
//! | dock                               |
//! +------------------------------------+
//! ```
//!
//! A place of size 1 is there but shows nothing (Project: monitor, head row and dock).
//! Passive paint only: the board keeps no state, knows no application and never reads
//! or writes the database; sizes and faces come from the application's layout contract.

use eframe::egui::{self, Color32, Rect, Sense, Vec2};
use serde::Deserialize;

/// The sizes of the places, from the layout contract (`board`).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct BoardSizes {
    /// Share of the width the left column takes.
    pub left_ratio: f32,
    pub divider_width: f32,
    pub left_min_width: f32,
    pub right_min_width: f32,
    /// Monitor height at the top of the left column.
    pub monitor_height: f32,
    /// The row under the monitor (tabs, transport).
    pub head_height: f32,
    /// The dock along the bottom; never more than [`DOCK_MAX_SHARE`] of the height.
    pub dock_height: f32,
}

/// The dock never takes more than this share of the board height.
pub const DOCK_MAX_SHARE: f32 = 0.42;

/// Faces of the board.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardFaces {
    pub bg: Color32,
    pub left: Color32,
    pub right: Color32,
    pub divider: Color32,
}

/// A place of the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    Monitor,
    Head,
    Body,
    Right,
    Dock,
}

/// The rectangles of the places inside `rect`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardRects {
    pub monitor: Rect,
    pub head: Rect,
    pub body: Rect,
    pub divider: Rect,
    pub right: Rect,
    pub dock: Rect,
}

impl BoardRects {
    pub fn place(&self, place: Place) -> Rect {
        match place {
            Place::Monitor => self.monitor,
            Place::Head => self.head,
            Place::Body => self.body,
            Place::Right => self.right,
            Place::Dock => self.dock,
        }
    }

    /// The whole left column (monitor, head row and body).
    pub fn left(&self) -> Rect {
        Rect::from_min_max(self.monitor.min, self.body.max)
    }
}

/// Splits `rect` into the places of the board.
pub fn layout(rect: Rect, sizes: &BoardSizes) -> BoardRects {
    let dock_height = sizes.dock_height.min(rect.height() * DOCK_MAX_SHARE).max(0.0);
    let dock = Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - dock_height), rect.max);
    let content = Rect::from_min_max(rect.min, egui::pos2(rect.right(), dock.top()));

    let usable = content.width().max(sizes.left_min_width + sizes.right_min_width);
    let split = (usable * sizes.left_ratio)
        .clamp(sizes.left_min_width, usable - sizes.right_min_width);
    let left = Rect::from_min_size(content.min, Vec2::new(split, content.height()));
    let divider = Rect::from_min_size(
        egui::pos2(left.right(), content.top()),
        Vec2::new(sizes.divider_width, content.height()),
    );
    let right = Rect::from_min_max(egui::pos2(divider.right(), content.top()), content.max);

    let monitor_height = sizes.monitor_height.clamp(0.0, left.height());
    let monitor = Rect::from_min_size(left.min, Vec2::new(left.width(), monitor_height));
    let head_height = sizes.head_height.clamp(0.0, left.bottom() - monitor.bottom());
    let head = Rect::from_min_size(
        egui::pos2(left.left(), monitor.bottom()),
        Vec2::new(left.width(), head_height),
    );
    let body = Rect::from_min_max(egui::pos2(left.left(), head.bottom()), left.max);
    BoardRects { monitor, head, body, divider, right, dock }
}

/// Paints the board and calls `place` for every place in the order monitor, head, body,
/// right, dock; the first answer stops the rest (one intent per frame, as the forms do).
pub fn show<R>(
    ui: &mut egui::Ui,
    sizes: &BoardSizes,
    faces: &BoardFaces,
    mut place: impl FnMut(&mut egui::Ui, Place, Rect) -> Option<R>,
) -> Option<R> {
    let rect = ui.available_rect_before_wrap();
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return None;
    }
    ui.allocate_rect(rect, Sense::hover());
    let rects = layout(rect, sizes);
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, faces.bg);
    painter.rect_filled(rects.left(), 0.0, faces.left);
    painter.rect_filled(rects.divider, 0.0, faces.divider);
    painter.rect_filled(rects.right, 0.0, faces.right);
    for which in [Place::Monitor, Place::Head, Place::Body, Place::Right, Place::Dock] {
        let area = rects.place(which);
        if area.width() < 1.0 || area.height() < 1.0 {
            continue;
        }
        let answer = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(area)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    ui.set_clip_rect(area);
                    place(ui, which, area)
                },
            )
            .inner;
        if answer.is_some() {
            return answer;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sizes() -> BoardSizes {
        BoardSizes {
            left_ratio: 0.31,
            divider_width: 5.0,
            left_min_width: 280.0,
            right_min_width: 200.0,
            monitor_height: 1.0,
            head_height: 1.0,
            dock_height: 1.0,
        }
    }

    #[test]
    fn project_places_fill_the_board_around_one_pixel_places() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1000.0, 700.0));
        let rects = layout(rect, &sizes());
        assert_eq!(rects.dock.height(), 1.0);
        assert_eq!(rects.monitor.height(), 1.0);
        assert_eq!(rects.head.height(), 1.0);
        assert_eq!(rects.body.height(), 700.0 - 3.0);
        assert_eq!(rects.left().width(), 310.0);
        assert_eq!(rects.divider.width(), 5.0);
        assert_eq!(rects.right.left(), 315.0);
        assert_eq!(rects.right.right(), 1000.0);
    }

    #[test]
    fn the_dock_never_takes_more_than_its_share_and_columns_keep_their_minimum() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(400.0, 300.0));
        let rects = layout(rect, &BoardSizes { dock_height: 500.0, ..sizes() });
        assert!((rects.dock.height() - 300.0 * DOCK_MAX_SHARE).abs() < 0.01);
        assert_eq!(rects.left().width(), 280.0);
    }

    #[test]
    fn the_monitor_never_grows_past_the_column() {
        let rect = Rect::from_min_size(egui::pos2(0.0, 0.0), Vec2::new(1000.0, 400.0));
        let rects = layout(rect, &BoardSizes { monitor_height: 900.0, head_height: 30.0, ..sizes() });
        assert_eq!(rects.monitor.bottom(), rects.left().bottom());
        assert_eq!(rects.body.height(), 0.0);
    }
}

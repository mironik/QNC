//! The one QNC desktop board (user rule 2026-09-30): every application uses the same
//! board; a layout says how the window is split and which block goes where, and forms
//! only draw blocks by name.
//!
//! A layout is a tree of splits (data, [`Layout`]): each part of a split has a size
//! (pixels, a share with minimum widths, a picture shape, or the rest) and holds either
//! a further split or one block by name. The standard layout of every application today
//! is one such tree ([`BoardSizes::standard_layout`]):
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
//! Later (user rule 2026-09-30) layouts saved by a user, dragged dividers or a layout
//! chosen per project are added here, in this block; the forms do not change, because
//! they only draw what a place asks for by name. Passive paint only: the board keeps no
//! state, knows no application and never reads or writes the database.

use eframe::egui::{self, Color32, Rect, Sense, Vec2};
use serde::{Deserialize, Serialize};

/// The measures of the standard layout, from the application's layout contract.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct BoardSizes {
    /// Share of the width the left column takes.
    pub left_ratio: f32,
    pub divider_width: f32,
    pub left_min_width: f32,
    pub right_min_width: f32,
    /// Monitor height at the top of the left column; see [`BoardSizes::monitor_aspect`].
    pub monitor_height: f32,
    /// The row under the monitor (tabs, transport).
    pub head_height: f32,
    /// The dock along the bottom; never more than [`DOCK_MAX_SHARE`] of the height.
    pub dock_height: f32,
    /// When set, the monitor keeps this picture shape in the column width instead of
    /// `monitor_height` (Ingest, Media Assist, Story).
    #[serde(default)]
    pub monitor_aspect: Option<MonitorAspect>,
}

/// A monitor that keeps its picture shape (from the preview block of the contract).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
pub struct MonitorAspect {
    /// Width over height, e.g. 16/9.
    pub ratio: f32,
    /// Height the column keeps under the monitor.
    pub reserve_below: f32,
    pub min_height: f32,
    /// Picture width is the column width less this, never below `min_width`.
    pub width_inset: f32,
    pub min_width: f32,
}

impl MonitorAspect {
    /// The monitor height in a column of `width` x `height`.
    pub fn height(&self, width: f32, height: f32) -> f32 {
        let picture_width = (width - self.width_inset).max(self.min_width);
        let available = (height - self.reserve_below).max(self.min_height);
        (picture_width / self.ratio.max(0.1)).clamp(self.min_height, available)
    }
}

/// The dock never takes more than this share of the board height.
pub const DOCK_MAX_SHARE: f32 = 0.42;

/// The faces of the standard layout, by the names it uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoardFaces {
    pub bg: Color32,
    pub left: Color32,
    pub right: Color32,
    pub divider: Color32,
}

impl BoardFaces {
    pub fn named(&self, name: &str) -> Option<Color32> {
        match name {
            "bg" => Some(self.bg),
            "left" => Some(self.left),
            "right" => Some(self.right),
            "divider" => Some(self.divider),
            _ => None,
        }
    }
}

/// The block names an application gives the five places of the standard layout; an
/// empty name leaves the place empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoardNames<'a> {
    pub monitor: &'a str,
    pub head: &'a str,
    pub body: &'a str,
    pub right: &'a str,
    pub dock: &'a str,
}

/// Which way a split lays its parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// Side by side, left to right.
    Horizontal,
    /// One under the other, top to bottom.
    Vertical,
}

/// The size of one part along its split.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Size {
    /// Fixed pixels, never more than `max_share` of the split.
    Px { px: f32, max_share: Option<f32> },
    /// A share of the split; never below `min`, and leaves `rest_min` to the rest.
    Share { ratio: f32, min: f32, rest_min: f32 },
    /// Keeps a picture shape in the width of a vertical split.
    Aspect(MonitorAspect),
    /// What the other parts leave.
    Rest,
}

/// One part of a split: its size, its face and what it holds.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Part {
    pub size: Size,
    #[serde(default)]
    pub face: Option<String>,
    /// Nothing for a plain face (a divider).
    #[serde(default)]
    pub node: Option<Node>,
}

/// A split or one block by name.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Node {
    Split { axis: Axis, parts: Vec<Part> },
    Block { name: String },
}

/// A whole layout: the face of the board and its tree.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Layout {
    #[serde(default)]
    pub face: Option<String>,
    pub root: Node,
}

impl BoardSizes {
    /// The layout every application uses today: left column (monitor, head row, body),
    /// divider and right panel over the dock.
    pub fn standard_layout(&self, names: &BoardNames<'_>) -> Layout {
        let block = |name: &str| Some(Node::Block { name: name.to_string() });
        let px = |px| Size::Px { px, max_share: None };
        let monitor = match self.monitor_aspect {
            Some(aspect) => Size::Aspect(aspect),
            None => px(self.monitor_height),
        };
        let left = Node::Split {
            axis: Axis::Vertical,
            parts: vec![
                Part { size: monitor, face: None, node: block(names.monitor) },
                Part { size: px(self.head_height), face: None, node: block(names.head) },
                Part { size: Size::Rest, face: None, node: block(names.body) },
            ],
        };
        let content = Node::Split {
            axis: Axis::Horizontal,
            parts: vec![
                Part {
                    size: Size::Share {
                        ratio: self.left_ratio,
                        min: self.left_min_width,
                        rest_min: self.right_min_width,
                    },
                    face: Some("left".into()),
                    node: Some(left),
                },
                Part { size: px(self.divider_width), face: Some("divider".into()), node: None },
                Part { size: Size::Rest, face: Some("right".into()), node: block(names.right) },
            ],
        };
        Layout {
            face: Some("bg".into()),
            root: Node::Split {
                axis: Axis::Vertical,
                parts: vec![
                    Part { size: Size::Rest, face: None, node: Some(content) },
                    Part {
                        size: Size::Px { px: self.dock_height, max_share: Some(DOCK_MAX_SHARE) },
                        face: None,
                        node: block(names.dock),
                    },
                ],
            },
        }
    }
}

/// The desktop: the surface of the active application (block `surface`) over the
/// footer (block `footer`), so the footer is a place of every board.
pub fn surface_with_footer(footer_height: f32) -> Layout {
    let block = |name: &str| Some(Node::Block { name: name.to_string() });
    Layout {
        face: None,
        root: Node::Split {
            axis: Axis::Vertical,
            parts: vec![
                Part { size: Size::Rest, face: None, node: block("surface") },
                Part { size: Size::Px { px: footer_height, max_share: None }, face: None, node: block("footer") },
            ],
        },
    }
}

/// What a layout puts in a rectangle: faces (in paint order) and blocks (in draw order).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Solved {
    pub faces: Vec<(String, Rect)>,
    pub blocks: Vec<(String, Rect)>,
}

impl Solved {
    /// The rectangle of the block `name`.
    pub fn block(&self, name: &str) -> Option<Rect> {
        self.blocks.iter().find(|(block, _)| block == name).map(|(_, rect)| *rect)
    }
}

/// Places the layout in `rect`.
pub fn solve(layout: &Layout, rect: Rect) -> Solved {
    let mut solved = Solved::default();
    if let Some(face) = &layout.face {
        solved.faces.push((face.clone(), rect));
    }
    place_node(&layout.root, rect, &mut solved);
    solved
}

fn place_node(node: &Node, rect: Rect, solved: &mut Solved) {
    match node {
        Node::Block { name } => {
            if !name.is_empty() {
                solved.blocks.push((name.clone(), rect));
            }
        }
        Node::Split { axis, parts } => {
            let (total, cross) = match axis {
                Axis::Horizontal => (rect.width(), rect.height()),
                Axis::Vertical => (rect.height(), rect.width()),
            };
            let lengths = part_lengths(parts, total, cross);
            let mut at = 0.0;
            for (part, length) in parts.iter().zip(lengths) {
                let area = match axis {
                    Axis::Horizontal => Rect::from_min_size(
                        egui::pos2(rect.left() + at, rect.top()),
                        Vec2::new(length, rect.height()),
                    ),
                    Axis::Vertical => Rect::from_min_size(
                        egui::pos2(rect.left(), rect.top() + at),
                        Vec2::new(rect.width(), length),
                    ),
                };
                at += length;
                if let Some(face) = &part.face {
                    solved.faces.push((face.clone(), area));
                }
                if let Some(node) = &part.node {
                    place_node(node, area, solved);
                }
            }
        }
    }
}

/// Lengths of the parts along a split of `total`: fixed parts in order, each at most what
/// is left, then the rest shared by the rest parts.
fn part_lengths(parts: &[Part], total: f32, cross: f32) -> Vec<f32> {
    let mut left = total;
    let mut lengths: Vec<Option<f32>> = parts
        .iter()
        .map(|part| {
            let wanted = match part.size {
                Size::Px { px, max_share } => max_share.map_or(px, |share| px.min(total * share)),
                Size::Share { ratio, min, rest_min } => {
                    let usable = total.max(min + rest_min);
                    (usable * ratio).clamp(min, usable - rest_min)
                }
                Size::Aspect(aspect) => aspect.height(cross, total),
                Size::Rest => return None,
            };
            let length = wanted.clamp(0.0, left.max(0.0));
            left -= length;
            Some(length)
        })
        .collect();
    let rest_count = lengths.iter().filter(|length| length.is_none()).count().max(1) as f32;
    let rest = left.max(0.0) / rest_count;
    lengths.iter_mut().map(|length| length.unwrap_or(rest)).collect()
}

/// Paints the standard layout with `faces` and calls `block` for every named place in
/// the order monitor, head, body, right, dock; the first answer stops the rest (one
/// intent per frame, as the forms do).
pub fn show<R>(
    ui: &mut egui::Ui,
    sizes: &BoardSizes,
    faces: &BoardFaces,
    names: &BoardNames<'_>,
    block: impl FnMut(&mut egui::Ui, &str, Rect) -> Option<R>,
) -> Option<R> {
    show_layout(ui, &sizes.standard_layout(names), |name| faces.named(name), block)
}

/// Paints any layout (faces by name through `face`) and calls `block` for every block.
pub fn show_layout<R>(
    ui: &mut egui::Ui,
    layout: &Layout,
    face: impl Fn(&str) -> Option<Color32>,
    block: impl FnMut(&mut egui::Ui, &str, Rect) -> Option<R>,
) -> Option<R> {
    let rect = ui.available_rect_before_wrap();
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return None;
    }
    ui.allocate_rect(rect, Sense::hover());
    draw(ui, &solve(layout, rect), face, block, |_| false)
}

/// Faces first, then blocks in tree order. After the first answer only the blocks
/// `always` names are still drawn (the frame's own places are never skipped).
fn draw<R>(
    ui: &mut egui::Ui,
    solved: &Solved,
    face: impl Fn(&str) -> Option<Color32>,
    mut block: impl FnMut(&mut egui::Ui, &str, Rect) -> Option<R>,
    always: impl Fn(&str) -> bool,
) -> Option<R> {
    for (name, area) in &solved.faces {
        if let Some(color) = face(name) {
            ui.painter().rect_filled(*area, 0.0, color);
        }
    }
    let mut answer = None;
    for (name, area) in &solved.blocks {
        if area.width() < 1.0 || area.height() < 1.0 || (answer.is_some() && !always(name)) {
            continue;
        }
        let this = ui
            .scope_builder(
                egui::UiBuilder::new()
                    .max_rect(*area)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                // No clip: a place may draw its own edge half a pixel outside (the dock
                // top line), as the forms did.
                |ui| block(ui, name, *area),
            )
            .inner;
        if answer.is_none() {
            answer = this;
        }
    }
    answer
}

mod frame;
pub use frame::{show_in_frame, show_surface_in_frame, Frame, FrameBlocks};

#[cfg(test)]
mod tests;

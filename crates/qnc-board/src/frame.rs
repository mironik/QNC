//! The frame around a board (user rule 2026-10-01: the layout is the frame and the guide
//! of the display, not pixels). A frame is a layout with one slot where the board goes;
//! the frame and the board are drawn as one tree, so the frame can later get more places
//! (a side bar, a second footer) without any board or form changing.

use eframe::egui::{self, Color32, Rect, Sense};

use crate::{draw, solve, surface_with_footer, Layout, Node, Part};

/// The places a frame owner draws itself (the desktop footer).
pub trait FrameBlocks {
    fn block(&mut self, ui: &mut egui::Ui, name: &str, rect: Rect);
}

/// A layout with a slot for the board, the faces of the frame and the owner of its
/// other places.
pub struct Frame<'a> {
    pub layout: Layout,
    pub slot: String,
    pub faces: Vec<(String, Color32)>,
    pub blocks: Option<&'a mut dyn FrameBlocks>,
}

impl Frame<'static> {
    /// No frame: the board fills the window (a standalone application).
    pub fn bare() -> Self {
        Frame {
            layout: Layout { face: None, root: block("board") },
            slot: "board".into(),
            faces: Vec::new(),
            blocks: None,
        }
    }
}

impl<'a> Frame<'a> {
    /// The desktop frame: the board over the footer (block `footer`).
    pub fn desktop(footer_height: f32, footer: &'a mut dyn FrameBlocks) -> Self {
        Frame {
            layout: surface_with_footer(footer_height),
            slot: "surface".into(),
            faces: Vec::new(),
            blocks: Some(footer),
        }
    }

    /// The frame's own places: every block but the slot.
    fn own_blocks(&self) -> Vec<String> {
        self.layout.block_names().into_iter().filter(|name| *name != self.slot).collect()
    }
}

impl Layout {
    /// This layout with `inner` in the place of block `slot`; when that place has no
    /// face of its own, the face of `inner` paints it.
    pub fn nest(&self, slot: &str, inner: &Layout) -> Layout {
        if is_block(&self.root, slot) {
            return Layout { face: self.face.clone().or_else(|| inner.face.clone()), root: inner.root.clone() };
        }
        Layout { face: self.face.clone(), root: nest_node(&self.root, slot, inner) }
    }

    /// The names of all blocks of this layout, in tree order.
    pub fn block_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        collect_names(&self.root, &mut names);
        names
    }
}

fn block(name: &str) -> Node {
    Node::Block { name: name.to_string() }
}

fn is_block(node: &Node, slot: &str) -> bool {
    matches!(node, Node::Block { name } if name == slot)
}

fn nest_node(node: &Node, slot: &str, inner: &Layout) -> Node {
    match node {
        Node::Block { .. } => node.clone(),
        Node::Split { axis, parts } => Node::Split {
            axis: *axis,
            parts: parts
                .iter()
                .map(|part| match &part.node {
                    Some(held) if is_block(held, slot) => Part {
                        size: part.size,
                        face: part.face.clone().or_else(|| inner.face.clone()),
                        node: Some(inner.root.clone()),
                    },
                    held => Part {
                        size: part.size,
                        face: part.face.clone(),
                        node: held.as_ref().map(|held| nest_node(held, slot, inner)),
                    },
                })
                .collect(),
        },
    }
}

fn collect_names(node: &Node, names: &mut Vec<String>) {
    match node {
        Node::Block { name } => names.push(name.clone()),
        Node::Split { parts, .. } => {
            for held in parts.iter().filter_map(|part| part.node.as_ref()) {
                collect_names(held, names);
            }
        }
    }
}

/// Draws a board layout in its frame as one tree: the board's blocks through `block`
/// (the first answer stops the rest of the board), the frame's own places through the
/// frame owner (always drawn).
pub fn show_in_frame<R>(
    ui: &mut egui::Ui,
    frame: &mut Frame<'_>,
    layout: &Layout,
    face: impl Fn(&str) -> Option<Color32>,
    mut block: impl FnMut(&mut egui::Ui, &str, Rect) -> Option<R>,
) -> Option<R> {
    let rect = ui.available_rect_before_wrap();
    if rect.width() <= 1.0 || rect.height() <= 1.0 {
        return None;
    }
    ui.allocate_rect(rect, Sense::hover());
    let whole = frame.layout.nest(&frame.slot, layout);
    let own = frame.own_blocks();
    let faces = &frame.faces;
    let mut owner = frame.blocks.as_deref_mut();
    draw(
        ui,
        &solve(&whole, rect),
        |name| face(name).or_else(|| faces.iter().find(|(face, _)| face == name).map(|(_, color)| *color)),
        |ui, name, rect| {
            if own.iter().any(|own| own == name) {
                if let Some(owner) = owner.as_deref_mut() {
                    owner.block(ui, name, rect);
                }
                None
            } else {
                block(ui, name, rect)
            }
        },
        |name| own.iter().any(|own| own == name),
    )
}

/// A board that is not a layout yet: one place `surface` drawn by `draw`, in the frame.
pub fn show_surface_in_frame(ui: &mut egui::Ui, frame: &mut Frame<'_>, draw: impl FnOnce(&mut egui::Ui)) {
    let surface = Layout { face: None, root: block("surface") };
    let mut draw = Some(draw);
    show_in_frame(ui, frame, &surface, |_| None, |ui, name, _| {
        if let (true, Some(draw)) = (name == "surface", draw.take()) {
            draw(ui);
        }
        None::<()>
    });
}

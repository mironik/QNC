//! The location browser panel (user 2026-10-20: "a block written once works everywhere"; the
//! Ingest and Project forms each drew their own copy). One row of sources (Local, LAN,
//! Internet), the Up and Disks links, the path or the disks of the computer, the folders, an
//! error or a loading line. It draws what the caller's browser state says and returns what the
//! user asked; it owns no browser session, reads no filesystem and knows no form.

use eframe::egui::{self, Color32, Label, RichText, ScrollArea, Sense, Ui, Vec2};

pub const MODULE_ID: &str = "qnc.module.dir-browser-panel";

const UP_WIDTH: f32 = 42.0;
const DISKS_WIDTH: f32 = 58.0;
const NAV_GAP: f32 = 12.0;
/// The folders line up under the path, after Up and Disks.
const TREE_OFFSET: f32 = UP_WIDTH + NAV_GAP + DISKS_WIDTH + NAV_GAP;
/// Between the steps of a path.
const CRUMB_SEPARATOR: &str = "\\";

/// The colours and sizes the form gives.
#[derive(Debug, Clone, Copy)]
pub struct BrowserStyle {
    pub text: Color32,
    pub muted: Color32,
    /// A link that cannot be used now.
    pub disabled: Color32,
    pub accent: Color32,
    pub border: Color32,
    pub danger: Color32,
    pub focus: Color32,
    pub font_ui: f32,
    pub control_height: f32,
}

/// The words of the panel (from the form's text contract).
pub struct BrowserLabels<'a> {
    pub sources: &'a str,
    pub up: &'a str,
    pub disks: &'a str,
    pub empty_folder: &'a str,
    pub loading: &'a str,
}

/// One folder or disk.
pub struct Entry<'a> {
    pub name: &'a str,
    pub serial: &'a str,
    pub volume: &'a str,
    pub uri: &'a str,
}

/// One step of the path.
pub struct Crumb<'a> {
    pub label: &'a str,
    pub uri: &'a str,
}

/// What the row after Up and Disks shows.
pub enum Nav<'a> {
    /// The disks of the computer (the entries), or this message when there are none.
    Roots { empty: &'a str },
    /// The path as steps the user can open.
    Crumbs(Vec<Crumb<'a>>),
    /// The path or a name as text.
    Path(&'a str),
}

/// What the area under it shows.
pub enum Body<'a> {
    /// The folders (the entries).
    Folders,
    /// A message instead of folders.
    Message(&'a str),
    /// Nothing (the disks are in the row above).
    Nothing,
}

pub struct BrowserInput<'a> {
    pub id_salt: &'a str,
    /// The source tabs, left to right, and which is chosen.
    pub kinds: &'a [&'a str],
    pub kind: usize,
    pub nav: Nav<'a>,
    pub body: Body<'a>,
    pub entries: Vec<Entry<'a>>,
    pub can_up: bool,
    pub can_disks: bool,
    pub busy: bool,
    pub error: Option<&'a str>,
    /// The tree is at most this high; `None`: all the room left.
    pub max_tree_height: Option<f32>,
    /// The keyboard is on this source tab or this entry (the form's focus walk says).
    pub focused_kind: Option<usize>,
    pub focused_entry: Option<usize>,
}

/// What the user asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserAction {
    Kind(usize),
    Open(String),
    Parent,
    Roots,
}

/// What the user asked of the browser with its confirm bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserAnswer {
    Browser(BrowserAction),
    Confirm,
    Cancel,
}

impl BrowserStyle {
    /// The browser in the look of the one desktop theme.
    pub fn from_theme(theme: &qnc_desktop_theme::Theme) -> Self {
        Self {
            text: theme.text,
            muted: theme.text_muted,
            disabled: theme.text_muted,
            accent: theme.accent,
            border: theme.border,
            danger: theme.danger,
            focus: theme.focus,
            font_ui: theme.font_ui,
            control_height: theme.chrome_control_height,
        }
    }
}

/// The confirm and cancel bar under the browser (its words, whether confirm can be used and
/// which button the keyboard is on: `Some(true)` confirm, `Some(false)` cancel).
pub struct ConfirmBar<'a> {
    pub confirm: &'a str,
    pub cancel: &'a str,
    pub can_confirm: bool,
    pub focused: Option<bool>,
}

/// The browser on its place with its confirm bar at the bottom, in the look of the one
/// theme; `pad` is the place's padding.
pub fn show_with_confirm(
    ui: &mut Ui,
    rect: egui::Rect,
    (theme, pad): (&qnc_desktop_theme::Theme, f32),
    input: BrowserInput<'_>,
    labels: &BrowserLabels<'_>,
    bar: &ConfirmBar<'_>,
) -> Option<BrowserAnswer> {
    // The bar holds the buttons with the pad above and below them (user 2026-10-25: Cancel and
    // Select stood too low, cut off at the bottom of the panel).
    let bar_height = theme.chrome_control_height + 2.0 * pad;
    let bar_top = (rect.bottom() - bar_height).max(rect.top());
    let bar_rect = egui::Rect::from_min_size(egui::pos2(rect.left(), bar_top), Vec2::new(rect.width(), bar_height));
    let browser_rect = egui::Rect::from_min_max(rect.left_top(), egui::pos2(rect.right(), (bar_rect.top() - 8.0).max(rect.top())));
    let style = BrowserStyle::from_theme(theme);
    let asked = ui
        .scope_builder(egui::UiBuilder::new().max_rect(browser_rect), |ui| {
            let place = qnc_ui_kit::fill_block(ui, theme.bg);
            ui.allocate_rect(place, Sense::hover());
            ui.scope_builder(egui::UiBuilder::new().max_rect(place.shrink(pad)), |ui| show(ui, input, labels, &style)).inner.map(BrowserAnswer::Browser)
        })
        .inner;
    if asked.is_some() {
        return asked;
    }
    ui.scope_builder(egui::UiBuilder::new().max_rect(bar_rect), |ui| {
        ui.allocate_rect(bar_rect, Sense::hover());
        let inner = egui::UiBuilder::new().max_rect(bar_rect.shrink(pad)).layout(egui::Layout::right_to_left(egui::Align::Center));
        ui.scope_builder(inner, |ui| {
            let response = qnc_ui_kit::show_form_action_bar(ui, &qnc_ui_kit::FormActionBarStyle::new(theme.text, theme.accent, theme.border, theme.font_ui, theme.chrome_control_height), bar.confirm, bar.can_confirm, bar.cancel);
            for (rect, which) in [(response.confirm_rect, true), (response.cancel_rect, false)] {
                qnc_ui_kit::paint_focus(ui, rect.unwrap_or(egui::Rect::NOTHING).expand(2.0), bar.focused == Some(which), theme.focus);
            }
            if response.confirm_clicked {
                Some(BrowserAnswer::Confirm)
            } else if response.cancel_clicked {
                Some(BrowserAnswer::Cancel)
            } else {
                None
            }
        })
        .inner
    })
    .inner
}

pub fn show(ui: &mut Ui, input: BrowserInput<'_>, labels: &BrowserLabels<'_>, style: &BrowserStyle) -> Option<BrowserAction> {
    let mut action = None;
    let tab_style = qnc_media_pool_head::RowStyle {
        text: style.text,
        muted: style.muted,
        accent: style.accent,
        border: style.border,
        font_ui: style.font_ui,
        control_height: style.control_height,
        tab_gap: 0.0,
    };
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), style.control_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(text(labels.sources, style.muted, style));
            ui.add_space(12.0);
            for (index, label) in input.kinds.iter().enumerate() {
                let selected = input.kind == index;
                let response = qnc_media_pool_head::tab(ui, label, selected, &tab_style);
                paint_focus(ui, response.rect, input.focused_kind == Some(index), style);
                if response.clicked() && !selected {
                    action = Some(BrowserAction::Kind(index));
                }
                ui.add_space(10.0);
            }
        },
    );
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        if fixed_link(ui, labels.up, input.can_up, UP_WIDTH, style).clicked() {
            action = Some(BrowserAction::Parent);
        }
        ui.add_space(NAV_GAP);
        if fixed_link(ui, labels.disks, input.can_disks && !input.busy, DISKS_WIDTH, style).clicked() {
            action = Some(BrowserAction::Roots);
        }
        ui.add_space(NAV_GAP);
        nav(ui, &input, style, &mut action);
    });
    if let Some(error) = input.error {
        ui.add_space(4.0);
        ui.label(text(error, style.danger, style));
    }
    ui.add_space(6.0);
    let room = ui.available_height().max(40.0);
    let height = input.max_tree_height.map_or(room, |max| room.min(max).max(40.0));
    ScrollArea::vertical()
        .id_salt((input.id_salt, "tree"))
        .max_height(height)
        .min_scrolled_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            body(ui, &input, labels, style, &mut action);
        });
    action
}

fn nav(ui: &mut Ui, input: &BrowserInput<'_>, style: &BrowserStyle, action: &mut Option<BrowserAction>) {
    match &input.nav {
        Nav::Roots { empty } if input.entries.is_empty() => {
            ui.label(text(empty, style.muted, style));
        }
        Nav::Roots { .. } => disks(ui, input, style, action),
        Nav::Path(path) => {
            ui.label(text(path, style.text, style));
        }
        Nav::Crumbs(crumbs) => {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                for (index, crumb) in crumbs.iter().enumerate() {
                    if index > 0 {
                        ui.label(text(CRUMB_SEPARATOR, style.muted, style));
                    }
                    if link(ui, crumb.label, true, style).clicked() {
                        *action = Some(BrowserAction::Open(crumb.uri.to_string()));
                    }
                }
            });
        }
    }
}

/// The disks of the computer: name, serial number and volume name in three columns.
fn disks(ui: &mut Ui, input: &BrowserInput<'_>, style: &BrowserStyle, action: &mut Option<BrowserAction>) {
    egui::Grid::new((input.id_salt, "disks"))
        .num_columns(3)
        .spacing(Vec2::new(14.0, 4.0))
        .striped(false)
        .show(ui, |ui| {
            for (index, entry) in input.entries.iter().enumerate() {
                let name = cell(ui, entry.name, style);
                paint_focus(ui, name.rect, input.focused_entry == Some(index), style);
                let clicked = name.clicked() | cell(ui, entry.serial, style).clicked() | cell(ui, entry.volume, style).clicked();
                ui.end_row();
                if clicked {
                    *action = Some(BrowserAction::Open(entry.uri.to_string()));
                }
            }
        });
}

fn body(ui: &mut Ui, input: &BrowserInput<'_>, labels: &BrowserLabels<'_>, style: &BrowserStyle, action: &mut Option<BrowserAction>) {
    if input.busy {
        ui.label(text(labels.loading, style.muted, style));
        return;
    }
    match input.body {
        Body::Nothing => {}
        Body::Message(message) => {
            ui.label(text(message, style.muted, style));
        }
        Body::Folders if input.entries.is_empty() => {
            ui.horizontal(|ui| {
                ui.add_space(TREE_OFFSET);
                ui.label(text(labels.empty_folder, style.muted, style));
            });
        }
        Body::Folders => {
            for (index, entry) in input.entries.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(TREE_OFFSET);
                    let response = link(ui, entry.name, true, style);
                    paint_focus(ui, response.rect, input.focused_entry == Some(index), style);
                    if response.clicked() {
                        *action = Some(BrowserAction::Open(entry.uri.to_string()));
                    }
                });
            }
        }
    }
}

/// A long path as its end (the start is cut and marked).
pub fn short_path(path: &str) -> String {
    if path.chars().count() <= 42 {
        return path.to_string();
    }
    let tail: Vec<char> = path.chars().rev().take(36).collect();
    format!("...{}", tail.into_iter().rev().collect::<String>())
}

fn text(words: &str, color: Color32, style: &BrowserStyle) -> RichText {
    RichText::new(words).color(color).size(style.font_ui)
}

fn link(ui: &mut Ui, words: &str, enabled: bool, style: &BrowserStyle) -> egui::Response {
    let color = if enabled { style.text } else { style.disabled };
    ui.add_enabled(enabled, Label::new(text(words, color, style)).sense(Sense::click()))
}

fn fixed_link(ui: &mut Ui, words: &str, enabled: bool, width: f32, style: &BrowserStyle) -> egui::Response {
    ui.allocate_ui_with_layout(Vec2::new(width, style.control_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        link(ui, words, enabled, style)
    })
    .inner
}

fn cell(ui: &mut Ui, words: &str, style: &BrowserStyle) -> egui::Response {
    ui.add(Label::new(text(words, style.text, style)).sense(Sense::click()).selectable(false))
}

/// The keyboard's frame around what it is on (as the focus walk draws it elsewhere).
fn paint_focus(ui: &Ui, rect: egui::Rect, on: bool, style: &BrowserStyle) {
    qnc_ui_kit::paint_focus(ui, rect.expand(3.0), on, style.focus);
}

#[cfg(test)]
mod tests {
    use super::short_path;

    #[test]
    fn a_long_path_keeps_its_end() {
        assert_eq!(short_path("C:\\short"), "C:\\short");
        let long = "C:\\Users\\someone\\Projects\\a very long folder name\\and another one";
        let shown = short_path(long);
        assert!(shown.starts_with("...") && long.ends_with(&shown[3..]) && shown.chars().count() == 39);
    }
}

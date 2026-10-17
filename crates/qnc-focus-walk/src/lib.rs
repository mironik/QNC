//! The keyboard walk over the panels of a form (user rules 2026-10-13/14: Tab walks the
//! panels in the order the form's layout contract gives, Enter confirms in a panel and with
//! nothing to confirm goes on to the next one, the keys act on the panel in focus).
//!
//! A panel is a row of buttons (arrows choose, Enter presses and goes on), the pool tabs
//! (arrows choose the tab at once), a list (the arrows and Page Up/Down move the focused
//! item, Enter opens it, the tick key ticks it; the drawn list applies each numbered step
//! once) or the source timeline (its own keys). It knows panels only by the names of the
//! layout contract and actions only by catalog action ids; it owns no player, database or
//! form and knows no application. A form without a walk keeps its keys as they were.

use std::collections::BTreeMap;

use eframe::egui;
pub use qnc_media_card::GridStep;

pub const MODULE_ID: &str = "qnc.module.focus-walk";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The panels a layout contract may name in its `tab_order`.
pub const SOURCE_KINDS: &str = "source_kinds";
pub const BROWSER: &str = "browser";
pub const BROWSER_BUTTONS: &str = "browser_buttons";
pub const POOL_TABS: &str = "pool_tabs";
pub const CARDS: &str = "pool";
pub const SOURCE_TIMELINE: &str = "source_timeline";
pub const SOURCE_BUTTONS: &str = "source_buttons";

/// The panels that are lists of items.
const LISTS: [&str; 2] = [BROWSER, CARDS];

/// The passive mark of the focus: a thin frame on top of `rect`. It paints only.
pub fn paint_focus(ui: &egui::Ui, rect: egui::Rect, focused: bool, color: egui::Color32) {
    if focused {
        ui.painter().rect_stroke(rect.shrink(1.0), 0.0, egui::Stroke::new(1.5, color), egui::StrokeKind::Inside);
    }
}

/// What became of one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Walk {
    /// The caller handles the action as it is.
    Pass,
    /// Taken here; whether the view changed.
    Done(bool),
    /// The key stands for this catalog action (a button of a row, a pool tab).
    Action(String),
}

/// What a form draws of the focus: the panel, the button chosen in a row panel, and the
/// last key on a list panel with its number.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalkView {
    pub panel: String,
    pub button: Option<String>,
    pub step: Option<(GridStep, u64)>,
}

impl WalkView {
    pub fn is(&self, panel: &str) -> bool {
        self.panel == panel
    }

    /// The step of the list panel `panel` (none when another panel has the keyboard).
    pub fn step_of(&self, panel: &str) -> Option<(GridStep, u64)> {
        self.step.filter(|_| self.is(panel))
    }

    pub fn chosen(&self, action_id: &str) -> bool {
        self.button.as_deref() == Some(action_id)
    }
}

/// The panels of a form, its row panels with their action ids left to right, and the click
/// actions of each panel (a click on a panel puts the keyboard there).
pub type FocusNames<'a> = (Vec<&'a str>, Vec<(&'a str, Vec<&'a str>)>, Vec<(&'a str, &'a str)>);

#[derive(Debug, Clone, Default)]
pub struct FocusWalk {
    order: Vec<String>,
    rows: BTreeMap<String, Vec<String>>,
    clicks: Vec<(String, String)>,
    panel: usize,
    chosen: BTreeMap<String, usize>,
    step: Option<(GridStep, u64)>,
    /// The list panel the last step was made on: another list never takes it (the folders'
    /// Enter must not open a card when the keyboard comes to the cards).
    step_on: String,
    /// Enter opened the focused card: the clip it chooses puts the keyboard on the source
    /// timeline; a click on a card keeps it on the cards.
    opening: bool,
}

impl FocusWalk {
    /// The walk of a form; it starts on the first panel.
    pub fn new((order, rows, clicks): FocusNames<'_>) -> Self {
        let rows = rows.into_iter().map(|(panel, actions)| (panel.to_string(), actions.into_iter().map(str::to_string).collect())).collect();
        let clicks = clicks.into_iter().map(|(panel, action)| (panel.to_string(), action.to_string())).collect();
        Self { order: order.into_iter().map(str::to_string).collect(), rows, clicks, ..Self::default() }
    }

    pub fn panel(&self) -> &str {
        self.order.get(self.panel).map_or("", String::as_str)
    }

    pub fn view(&self) -> WalkView {
        let button = self.row(self.panel()).and_then(|row| row.get(self.chosen_index(self.panel())).cloned());
        let step = self.step.filter(|_| self.step_on == self.panel());
        WalkView { panel: self.panel().to_string(), button, step }
    }

    /// One catalog action from a key or a click.
    pub fn route(&mut self, action_id: &str) -> Walk {
        if self.order.is_empty() {
            return Walk::Pass; // a form without a walk keeps its keys as they were
        }
        let panel = self.panel().to_string();
        let back = action_id == "step_back_frame";
        match (panel.as_str(), action_id) {
            (_, "focus_next" | "focus_prev") => Walk::Done(self.step_panel(action_id == "focus_next")),
            // The wheel over the source timeline (user 2026-10-16, the same in every form): the
            // keyboard goes there and the source timeline takes the wheel itself (piece
            // `qnc-source-timeline`: the arrows, Ctrl+wheel the level of a taken lane).
            (_, wheel) if wheel.starts_with("wheel_source") => {
                self.go(SOURCE_TIMELINE);
                Walk::Pass
            }
            // A click on a panel puts the keyboard there; the card Enter opened goes on to the
            // source timeline.
            (_, click) if self.clicks.iter().any(|(_, action)| action == click) => {
                let target = self.clicks.iter().find(|(_, action)| action == click).map(|(panel, _)| panel.clone()).unwrap_or_default();
                let opened = std::mem::take(&mut self.opening) && target == CARDS;
                self.go(if opened { SOURCE_TIMELINE } else { &target });
                Walk::Pass
            }
            // A pool tab, by click or its own key, puts the keyboard on the pool tabs.
            (_, tab) if self.row(POOL_TABS).is_some_and(|row| row.iter().any(|id| id == tab)) => {
                let index = self.row(POOL_TABS).and_then(|row| row.iter().position(|id| id == tab)).unwrap_or(0);
                self.chosen.insert(POOL_TABS.to_string(), index);
                self.go(POOL_TABS);
                Walk::Pass
            }
            (POOL_TABS, "step_back_frame" | "step_forward_frame") => {
                let tab = self.choose(POOL_TABS, back);
                tab.map_or(Walk::Done(false), Walk::Action)
            }
            (row, "step_back_frame" | "step_forward_frame") if self.row(row).is_some() => {
                self.choose(row, back);
                Walk::Done(true)
            }
            (row, "activate_focused_item") if row != POOL_TABS && self.row(row).is_some() => {
                let pressed = self.row(row).and_then(|actions| actions.get(self.chosen_index(row)).cloned());
                self.step_panel(true);
                pressed.map_or(Walk::Done(false), Walk::Action)
            }
            (list, key) if LISTS.contains(&list) && list_step(key).is_some() => {
                let step = list_step(key).unwrap_or(GridStep::Open);
                let number = self.step.map_or(1, |(_, number)| number + 1);
                self.step = Some((step, number));
                self.step_on = list.to_string();
                self.opening |= list == CARDS && step == GridStep::Open; // the clip it opens takes the keyboard
                Walk::Done(true)
            }
            (_, "activate_focused_item") => Walk::Done(self.step_panel(true)),
            // The arrows and I/O belong to the source timeline (and its buttons).
            (SOURCE_TIMELINE, _) => Walk::Pass,
            (SOURCE_BUTTONS, "mark_in" | "mark_out" | "select_mark_in" | "select_mark_out" | "navigate_prev_object" | "navigate_next_object" | "playlist_input_start") => Walk::Pass,
            (_, "step_back_frame" | "step_forward_frame" | "step_prev_part" | "step_next_part" | "mark_in" | "mark_out" | "page_prev" | "page_next" | "toggle_focused_item"
                | "select_mark_in" | "select_mark_out" | "navigate_prev_object" | "navigate_next_object" | "playlist_input_start") => Walk::Done(false),
            _ => Walk::Pass,
        }
    }

    fn row(&self, panel: &str) -> Option<&Vec<String>> {
        self.rows.get(panel).filter(|row| !row.is_empty())
    }

    fn chosen_index(&self, panel: &str) -> usize {
        self.chosen.get(panel).copied().unwrap_or(0)
    }

    /// The button to the left or right in a row, and its action.
    fn choose(&mut self, panel: &str, back: bool) -> Option<String> {
        let count = self.row(panel).map_or(0, Vec::len);
        let from = self.chosen_index(panel);
        let index = if back { from.saturating_sub(1) } else { (from + 1).min(count.saturating_sub(1)) };
        self.chosen.insert(panel.to_string(), index);
        self.row(panel).and_then(|row| row.get(index).cloned())
    }

    /// The keyboard goes to the panel `name` (a key of another panel took it there); the
    /// view after it.
    pub fn focus(&mut self, name: &str) -> WalkView {
        self.go(name);
        self.view()
    }

    fn go(&mut self, name: &str) {
        if let Some(index) = self.order.iter().position(|panel| panel == name) {
            self.panel = index;
        }
    }

    /// The next or previous panel that has something to take, round.
    fn step_panel(&mut self, forward: bool) -> bool {
        let count = self.order.len();
        for distance in 1..=count {
            let index = if forward { (self.panel + distance) % count } else { (self.panel + count - distance) % count };
            let name = self.order[index].as_str();
            let empty_row = self.rows.contains_key(name) && self.row(name).is_none();
            if !empty_row {
                self.panel = index;
                return true;
            }
        }
        false
    }
}

/// A key on a list panel: the arrows, a page up or down, Enter opens, the tick key ticks.
fn list_step(action_id: &str) -> Option<GridStep> {
    Some(match action_id {
        "step_back_frame" => GridStep::Left,
        "step_forward_frame" => GridStep::Right,
        "step_prev_part" => GridStep::Up,
        "step_next_part" => GridStep::Down,
        "page_prev" => GridStep::PageUp,
        "page_next" => GridStep::PageDown,
        "activate_focused_item" => GridStep::Open,
        "toggle_focused_item" => GridStep::Toggle,
        _ => return None,
    })
}

/// The focused item of a drawn list in focus (`step` from the walk view; each number is
/// applied once): it starts on `selected` and moves by `step` in a list of `count` items
/// in `columns` columns with `page_rows` rows on screen. Returns the focused item and the
/// step just applied (Enter, the tick key) so the list can act on it as on a click.
pub fn list_focus(
    ui: &egui::Ui,
    id: egui::Id,
    (count, columns, page_rows): (usize, usize, usize),
    selected: Option<usize>,
    step: Option<(GridStep, u64)>,
) -> (usize, Option<GridStep>) {
    let (key, applied) = (id.with("qnc-list-focus"), id.with("qnc-list-step"));
    if count == 0 {
        return (0, None);
    }
    let fresh = step.filter(|(_, number)| ui.data(|data| data.get_temp::<u64>(applied)) != Some(*number));
    let from = ui.data(|data| data.get_temp::<usize>(key)).or(selected).unwrap_or(0).min(count - 1);
    let focused = fresh.map_or(from, |(step, _)| qnc_media_card::step_focus(from, step, columns, count, page_rows));
    ui.data_mut(|data| {
        data.insert_temp(key, focused);
        if let Some((_, number)) = fresh {
            data.insert_temp(applied, number);
        }
    });
    (focused, fresh.map(|(step, _)| step))
}

/// The item of a drawn list the keyboard is on (when `panel` has the focus) and the item
/// Enter just opened, which the list opens as a click.
pub fn list_keys(ui: &egui::Ui, id: &str, count: usize, view: &WalkView, panel: &str) -> (Option<usize>, Option<usize>) {
    let (index, applied) = list_focus(ui, egui::Id::new(id), (count, 1, 12), None, view.step_of(panel));
    let opened = (applied == Some(GridStep::Open) && index < count).then_some(index);
    (view.is(panel).then_some(index), opened)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walk() -> FocusWalk {
        FocusWalk::new((
            vec![SOURCE_KINDS, BROWSER, BROWSER_BUTTONS, CARDS, SOURCE_TIMELINE, SOURCE_BUTTONS],
            vec![
                (SOURCE_KINDS, vec!["kind_local", "kind_lan"]),
                (BROWSER_BUTTONS, vec!["confirm", "cancel"]),
                (POOL_TABS, vec!["tab_all", "tab_virtual"]),
                (SOURCE_BUTTONS, vec!["clear", "import"]),
            ],
            vec![(CARDS, "preview_card"), (BROWSER, "open_folder")],
        ))
    }

    #[test]
    fn tab_and_enter_go_round_the_panels_of_the_form() {
        let mut walk = walk();
        assert_eq!(walk.panel(), SOURCE_KINDS, "it starts on the first panel");
        for expected in [BROWSER, BROWSER_BUTTONS, CARDS, SOURCE_TIMELINE, SOURCE_BUTTONS, SOURCE_KINDS] {
            walk.route("focus_next");
            assert_eq!(walk.panel(), expected);
        }
        walk.route("focus_prev");
        assert_eq!(walk.panel(), SOURCE_BUTTONS);
    }

    #[test]
    fn the_source_is_chosen_then_the_folder_then_select() {
        let mut walk = walk();
        assert_eq!(walk.route("step_forward_frame"), Walk::Done(true));
        assert_eq!(walk.view().button.as_deref(), Some("kind_lan"));
        assert_eq!(walk.route("activate_focused_item"), Walk::Action("kind_lan".into()));
        assert_eq!(walk.panel(), BROWSER, "the source pressed, on to its folders");
        walk.route("step_next_part");
        assert_eq!(walk.view().step, Some((GridStep::Down, 1)));
        walk.route("activate_focused_item");
        assert_eq!(walk.view().step, Some((GridStep::Open, 2)), "Enter opens the folder");
        assert_eq!(walk.panel(), BROWSER, "and the keyboard stays in the folders");
        walk.route("focus_next");
        assert_eq!(walk.route("activate_focused_item"), Walk::Action("confirm".into()));
        assert_eq!(walk.panel(), CARDS, "Select, on to the cards");
    }

    #[test]
    fn a_step_made_on_one_list_is_not_taken_by_another() {
        let mut walk = walk();
        walk.route("focus_next"); // the folders
        walk.route("activate_focused_item"); // Enter opens a folder
        walk.route("focus_next");
        walk.route("focus_next"); // the cards
        assert_eq!(walk.view().step, None, "the folders' Enter does not open a card");
        walk.route("step_forward_frame");
        assert_eq!(walk.view().step, Some((GridStep::Right, 2)));
    }

    #[test]
    fn keys_belong_to_the_panel_in_focus() {
        let mut walk = walk();
        walk.route("focus_next");
        walk.route("focus_next");
        walk.route("focus_next");
        assert_eq!(walk.route("mark_in"), Walk::Done(false), "I does not reach the player from the cards");
        walk.route("toggle_focused_item");
        assert_eq!(walk.view().step.map(|(step, _)| step), Some(GridStep::Toggle), "the tick key ticks the card");
        walk.route("activate_focused_item");
        walk.route("preview_card"); // the grid opens the card as a click
        assert_eq!(walk.panel(), SOURCE_TIMELINE, "Enter opened the card and the clip takes the keyboard");
        assert_eq!(walk.route("mark_in"), Walk::Pass, "I marks on the source timeline");
        walk.route("preview_card");
        assert_eq!(walk.panel(), CARDS, "a click on a card keeps the keyboard on the cards");
        walk.route("open_folder");
        assert_eq!(walk.panel(), BROWSER);
        assert_eq!(walk.route("tab_virtual"), Walk::Pass, "a tab outside the walk still switches");
    }
}

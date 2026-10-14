//! The keyboard walk over the panels of a form (user rule 2026-10-13: Tab walks the
//! panels in the order the form's layout contract gives, Enter confirms in a panel and
//! with nothing to confirm goes on to the next one, the keys act on the panel in focus).
//!
//! It knows panels only by the names of the layout contract and actions only by catalog
//! action ids. It owns no player, database or form and knows no application: a key either
//! stays with the caller (`Walk::Pass`), is taken here (`Walk::Done`) or stands for another
//! catalog action (`Walk::Action`, a pool tab or a dock button).

pub use qnc_media_card::GridStep;
use eframe::egui;

/// The passive mark of the focus: a thin frame on top of `rect`. It paints only.
pub fn paint_focus(ui: &egui::Ui, rect: egui::Rect, focused: bool, color: egui::Color32) {
    if focused {
        ui.painter().rect_stroke(rect.shrink(1.0), 0.0, egui::Stroke::new(1.5, color), egui::StrokeKind::Inside);
    }
}

pub const MODULE_ID: &str = "qnc.module.focus-walk";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The panels a layout contract may name in its `tab_order`.
pub const POOL_TABS: &str = "pool_tabs";
pub const CARDS: &str = "pool";
pub const SOURCE_TIMELINE: &str = "source_timeline";
pub const SOURCE_BUTTONS: &str = "source_buttons";
pub const BROWSER: &str = "browser";

/// What became of one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Walk {
    /// The caller handles the action as it is.
    Pass,
    /// Taken here; whether the view changed.
    Done(bool),
    /// The key stands for this catalog action (a pool tab, a dock button).
    Action(String),
}

/// What a form draws of the focus: the panel, the dock button the arrows are on and the
/// last key on the card grid with its number (the grid applies each number once).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WalkView {
    pub panel: String,
    pub button: Option<String>,
    pub card_step: Option<(GridStep, u64)>,
}

impl WalkView {
    pub fn is(&self, panel: &str) -> bool {
        self.panel == panel
    }
}

/// The panels of a form, its pool tabs, its dock buttons left to right, and the click
/// actions of each panel (a click on a panel puts the keyboard there).
pub type FocusNames<'a> = (Vec<&'a str>, Vec<&'a str>, Vec<&'a str>, Vec<(&'a str, &'a str)>);

#[derive(Debug, Clone, Default)]
pub struct FocusWalk {
    order: Vec<String>,
    tabs: Vec<String>,
    buttons: Vec<String>,
    clicks: Vec<(String, String)>,
    /// Enter opened the focused card: the clip it chooses puts the keyboard on the
    /// source timeline; a click on a card keeps it on the cards.
    opening: bool,
    panel: usize,
    tab: usize,
    button: usize,
    card_step: Option<(GridStep, u64)>,
}

impl FocusWalk {
    /// The walk of a form: its panels in order, the action ids of its pool tabs and of
    /// its dock buttons left to right; it starts on the first panel.
    pub fn new((order, tabs, buttons, clicks): FocusNames<'_>) -> Self {
        let owned = |names: Vec<&str>| names.into_iter().map(str::to_string).collect();
        let clicks = clicks.into_iter().map(|(panel, action)| (panel.to_string(), action.to_string())).collect();
        Self { order: owned(order), tabs: owned(tabs), buttons: owned(buttons), clicks, ..Self::default() }
    }

    pub fn panel(&self) -> &str {
        self.order.get(self.panel).map_or("", String::as_str)
    }

    pub fn view(&self) -> WalkView {
        let button = (self.panel() == SOURCE_BUTTONS).then(|| self.buttons.get(self.button).cloned()).flatten();
        WalkView { panel: self.panel().to_string(), button, card_step: self.card_step }
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
            // A click on a panel puts the keyboard there; the card Enter opened goes on to the
            // source timeline.
            (_, click) if self.clicks.iter().any(|(_, action)| action == click) => {
                let target = self.clicks.iter().find(|(_, action)| action == click).map(|(panel, _)| panel.clone()).unwrap_or_default();
                let opened = std::mem::take(&mut self.opening) && target == CARDS;
                self.go(if opened { SOURCE_TIMELINE } else { &target });
                Walk::Pass
            }
            // A pool tab, by click or Alt+letter, puts the keyboard on the pool tabs.
            (_, tab) if self.tabs.iter().any(|id| id == tab) => {
                self.tab = self.tabs.iter().position(|id| id == tab).unwrap_or(0);
                self.go(POOL_TABS);
                Walk::Pass
            }
            (POOL_TABS, "step_back_frame" | "step_forward_frame") => {
                self.tab = shift(self.tab, back, self.tabs.len());
                self.tabs.get(self.tab).cloned().map_or(Walk::Done(false), Walk::Action)
            }
            (SOURCE_BUTTONS, "step_back_frame" | "step_forward_frame") => {
                self.button = shift(self.button, back, self.buttons.len());
                Walk::Done(true)
            }
            (SOURCE_BUTTONS, "activate_focused_item") => {
                let pressed = self.buttons.get(self.button).cloned();
                self.step_panel(true);
                pressed.map_or(Walk::Done(false), Walk::Action)
            }
            (CARDS, key) if card_step(key).is_some() => {
                let step = card_step(key).unwrap_or(GridStep::Open);
                let number = self.card_step.map_or(1, |(_, number)| number + 1);
                self.card_step = Some((step, number));
                self.opening |= step == GridStep::Open; // the clip it opens takes the keyboard
                Walk::Done(true)
            }
            (_, "activate_focused_item") => Walk::Done(self.step_panel(true)),
            // The arrows and I/O belong to the source timeline (and its buttons).
            (SOURCE_TIMELINE, _) => Walk::Pass,
            (SOURCE_BUTTONS, "mark_in" | "mark_out") => Walk::Pass,
            (_, "step_back_frame" | "step_forward_frame" | "step_prev_part" | "step_next_part" | "mark_in" | "mark_out" | "page_prev" | "page_next") => Walk::Done(false),
            _ => Walk::Pass,
        }
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
            let empty = match self.order[index].as_str() {
                POOL_TABS => self.tabs.is_empty(),
                SOURCE_BUTTONS => self.buttons.is_empty(),
                _ => false,
            };
            if !empty {
                self.panel = index;
                return true;
            }
        }
        false
    }
}

fn shift(from: usize, back: bool, count: usize) -> usize {
    if back { from.saturating_sub(1) } else { (from + 1).min(count.saturating_sub(1)) }
}

/// A key on the card grid: the arrows, a page up or down, Enter opens, the tick key ticks.
fn card_step(action_id: &str) -> Option<GridStep> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn walk() -> FocusWalk {
        FocusWalk::new((
            vec![BROWSER, POOL_TABS, CARDS, SOURCE_TIMELINE, SOURCE_BUTTONS],
            vec!["tab_all", "tab_virtual"],
            vec!["filter", "import"],
            vec![(CARDS, "preview_card"), (BROWSER, "open_folder")],
        ))
    }

    #[test]
    fn tab_and_enter_go_round_the_panels_of_the_form() {
        let mut walk = walk();
        assert_eq!(walk.panel(), BROWSER, "it starts on the first panel");
        for expected in [POOL_TABS, CARDS, SOURCE_TIMELINE, SOURCE_BUTTONS, BROWSER] {
            walk.route("focus_next");
            assert_eq!(walk.panel(), expected);
        }
        walk.route("focus_prev");
        assert_eq!(walk.panel(), SOURCE_BUTTONS);
        walk.route("activate_focused_item");
        assert_eq!(walk.panel(), BROWSER, "Enter with nothing to confirm goes on");
        walk.route("activate_focused_item");
        assert_eq!(walk.panel(), POOL_TABS);
    }

    #[test]
    fn keys_belong_to_the_panel_in_focus() {
        let mut walk = walk();
        assert_eq!(walk.route("step_forward_frame"), Walk::Done(false), "the arrows do not reach the player from the browser");
        walk.route("tab_virtual");
        assert_eq!(walk.panel(), POOL_TABS, "a pool tab takes the keyboard to the tabs");
        assert_eq!(walk.route("step_back_frame"), Walk::Action("tab_all".into()));
        walk.route("activate_focused_item");
        assert_eq!(walk.route("step_next_part"), Walk::Done(true));
        assert_eq!(walk.view().card_step, Some((GridStep::Down, 1)));
        walk.route("toggle_focused_item");
        assert_eq!(walk.view().card_step, Some((GridStep::Toggle, 2)), "the tick key ticks the card");
        walk.route("activate_focused_item");
        assert_eq!(walk.view().card_step.map(|(step, _)| step), Some(GridStep::Open));
        walk.route("preview_card"); // the grid opens the card as a click
        assert_eq!(walk.panel(), SOURCE_TIMELINE, "Enter opened the card and the clip takes the keyboard");
        assert_eq!(walk.route("mark_in"), Walk::Pass, "I marks on the source timeline");
        walk.route("focus_next");
        assert_eq!(walk.route("step_forward_frame"), Walk::Done(true));
        assert_eq!(walk.view().button.as_deref(), Some("import"));
        assert_eq!(walk.route("activate_focused_item"), Walk::Action("import".into()));
        assert_eq!(walk.panel(), BROWSER, "the button pressed, on round the walk");
        walk.route("preview_card");
        assert_eq!(walk.panel(), CARDS, "a click on a card keeps the keyboard on the cards");
        walk.route("open_folder");
        assert_eq!(walk.panel(), BROWSER);
    }
}

//! The keys of a list panel (the media pool's cards, a browser's folders): the arrows,
//! page up/down, Enter opens, the tick key ticks. Each key is a numbered step the drawn
//! grid applies once, however often the view is read (one arrow fires two catalog actions,
//! live 2026-10-13). Enter on a list whose card chooses a clip remembers it: the clip it
//! opens takes the keyboard to its source timeline, a click keeps it on the list.
//! One block for every form (user 2026-10-18: Story and Ingest had each their own copy).

pub const MODULE_ID: &str = "qnc.module.key-list";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A key on a card grid in keyboard focus: the focused card moves, or Enter opens it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridStep {
    Left,
    Right,
    Up,
    Down,
    /// A screen of rows up or down (Page Up, Page Down).
    PageUp,
    PageDown,
    Open,
    /// The check mark of the focused card (as a click on it).
    Toggle,
}

/// The card focused after a move by `step` from `from` in a grid of `count` cards in
/// `cols` columns with `page_rows` rows on screen; it stays inside the grid.
pub fn step_focus(from: usize, step: GridStep, cols: usize, count: usize, page_rows: usize) -> usize {
    let last = count.saturating_sub(1);
    let cols = cols.max(1);
    let page = cols * page_rows.max(1);
    match step {
        GridStep::PageUp => from.checked_sub(page).unwrap_or(from % cols),
        GridStep::PageDown if from + page <= last => from + page,
        GridStep::PageDown => last - (last % cols) + (from % cols).min(last % cols),
        GridStep::Left => from.saturating_sub(1),
        GridStep::Right => (from + 1).min(last),
        GridStep::Up => from.checked_sub(cols).unwrap_or(from),
        GridStep::Down => if from + cols <= last { from + cols } else { from },
        GridStep::Open | GridStep::Toggle => from.min(last),
    }
}


/// The step a catalog key stands for on a list.
pub fn step_of(action_id: &str) -> Option<GridStep> {
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyList {
    step: Option<(GridStep, u64)>,
    opening: bool,
}

impl KeyList {
    /// One key on the list: the next numbered step. `chooses` is whether Enter on this list
    /// opens what the keyboard follows (the media pool's card), so `take_opening` tells it.
    pub fn press(&mut self, step: GridStep, chooses: bool) -> (GridStep, u64) {
        let number = self.step.map_or(1, |(_, number)| number + 1);
        self.step = Some((step, number));
        self.opening |= chooses && step == GridStep::Open;
        (step, number)
    }

    /// The last step with its number.
    pub fn last(&self) -> Option<(GridStep, u64)> {
        self.step
    }

    /// Whether Enter opened the item now chosen (once).
    pub fn take_opening(&mut self) -> bool {
        std::mem::take(&mut self.opening)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_focused_card_stays_inside_the_grid() {
        // 7 cards in 3 columns: 0 1 2 / 3 4 5 / 6
        assert_eq!(step_focus(0, GridStep::Left, 3, 7, 1), 0);
        assert_eq!(step_focus(2, GridStep::Right, 3, 7, 1), 3);
        assert_eq!(step_focus(6, GridStep::Right, 3, 7, 1), 6);
        assert_eq!(step_focus(4, GridStep::Down, 3, 7, 1), 4, "no card under 4");
        assert_eq!(step_focus(3, GridStep::Down, 3, 7, 1), 6);
        assert_eq!(step_focus(1, GridStep::Up, 3, 7, 1), 1);
        assert_eq!(step_focus(5, GridStep::Up, 3, 7, 1), 2);
        // 20 cards in 2 columns, 3 rows on screen: a page is 6 cards.
        assert_eq!(step_focus(1, GridStep::PageDown, 2, 20, 3), 7);
        assert_eq!(step_focus(17, GridStep::PageDown, 2, 20, 3), 19, "the last row, same column");
        assert_eq!(step_focus(7, GridStep::PageUp, 2, 20, 3), 1);
        assert_eq!(step_focus(3, GridStep::PageUp, 2, 20, 3), 1, "the first row, same column");
    }

    use super::*;

    #[test]
    fn every_key_is_a_new_numbered_step_and_enter_on_a_choosing_list_is_remembered_once() {
        let mut list = KeyList::default();
        assert_eq!(list.press(GridStep::Down, true), (GridStep::Down, 1));
        assert_eq!(list.press(GridStep::Open, false), (GridStep::Open, 2));
        assert!(!list.take_opening(), "Enter on a list that chooses nothing");
        list.press(GridStep::Open, true);
        assert!(list.take_opening());
        assert!(!list.take_opening(), "once");
        assert_eq!(list.last(), Some((GridStep::Open, 3)));
        assert_eq!(step_of("toggle_focused_item"), Some(GridStep::Toggle));
        assert_eq!(step_of("mark_in"), None);
    }
}

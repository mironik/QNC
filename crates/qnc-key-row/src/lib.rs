//! A row of buttons the arrows walk (pool tabs, the source dock buttons, a browser's
//! confirm buttons...): its catalog action ids left to right and the one chosen. The
//! left/right arrow chooses the neighbour (it stops at the ends), a click or the row's own
//! key chooses that button. One block for every form (user 2026-10-18: Story and Ingest
//! had each their own copy). It knows no form, panel or application.

pub const MODULE_ID: &str = "qnc.module.key-row";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyRow {
    actions: Vec<String>,
    chosen: usize,
}

impl KeyRow {
    pub fn new<S: Into<String>>(actions: impl IntoIterator<Item = S>) -> Self {
        Self { actions: actions.into_iter().map(Into::into).collect(), chosen: 0 }
    }

    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    pub fn contains(&self, action_id: &str) -> bool {
        self.actions.iter().any(|id| id == action_id)
    }

    /// The chosen button's action.
    pub fn chosen(&self) -> Option<&str> {
        self.actions.get(self.chosen).map(String::as_str)
    }

    /// The left (`back`) or right arrow: the neighbour is chosen, the ends hold; its action.
    pub fn choose(&mut self, back: bool) -> Option<&str> {
        let last = self.actions.len().saturating_sub(1);
        self.chosen = if back { self.chosen.saturating_sub(1) } else { (self.chosen + 1).min(last) };
        self.chosen()
    }

    /// A click on a button of the row, or its own key: it is chosen. False for an action the
    /// row has not.
    pub fn select(&mut self, action_id: &str) -> bool {
        match self.actions.iter().position(|id| id == action_id) {
            Some(index) => {
                self.chosen = index;
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arrows_walk_the_row_and_stop_at_its_ends() {
        let mut row = KeyRow::new(["a", "b", "c"]);
        assert_eq!(row.chosen(), Some("a"));
        assert_eq!(row.choose(true), Some("a"), "the left end holds");
        assert_eq!(row.choose(false), Some("b"));
        assert_eq!(row.choose(false), Some("c"));
        assert_eq!(row.choose(false), Some("c"), "the right end holds");
        assert!(row.select("a") && row.chosen() == Some("a"));
        assert!(!row.select("x"));
        assert_eq!(KeyRow::default().choose(false), None, "an empty row chooses nothing");
    }
}

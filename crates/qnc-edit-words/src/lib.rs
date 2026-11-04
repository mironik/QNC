//! The one meaning of a word (AGENTS 0.12, docs/97 step 6, user 2026-11-04): a word said from
//! `from` to `to` (frames of the view's axis) in the transcript, the script or the trim zoom.
//! - A click: the playhead to the word; while an edge or a cut is taken it goes to the word
//!   instead (an IN to the word's start, an OUT to its end, a cut to its nearer edge).
//! - Two words (a double click on each): the IN at the start of the earlier one and the OUT at
//!   the end of the later one, whichever comes first.
//! Owners (the source timeline, a SubSegment, a story) apply it on their own axis. Pure: the
//! pair is the only thing it keeps, the first word until the second.

use qnc_edit_edge::Edge;

pub const MODULE_ID: &str = "qnc.module.edit-words";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A word said on an axis, `[from, to)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word {
    pub from: u64,
    pub to: u64,
}

impl Word {
    pub fn new(from: u64, to: u64) -> Self {
        Word { from, to: to.max(from + 1) }
    }

    /// Where a taken thing goes when the word is clicked: an IN to its start, an OUT to its end,
    /// a cut (`None`: neither) to the edge of the word nearer to where the cut is now.
    pub fn goal(self, taken: Option<Edge>, now: u64) -> u64 {
        match taken {
            Some(Edge::In) => self.from,
            Some(Edge::Out) => self.to,
            None if now.abs_diff(self.from) <= now.abs_diff(self.to) => self.from,
            None => self.to,
        }
    }
}

/// The pair of words a cut by text makes (a double click on two words), for one place the
/// words belong to (`K`: a clip, a part of a statement).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WordPair<K> {
    first: Option<(K, Word)>,
}

/// What a word given to the pair did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paired {
    /// The first word: the IN at its start is where the cut starts.
    First(Word),
    /// The second word: the range from the start of the earlier to the end of the later.
    Range(Word),
}

impl<K: PartialEq> WordPair<K> {
    pub fn new() -> Self {
        WordPair { first: None }
    }

    /// A word double clicked in `place`: the first of a pair, or with the first one of the same
    /// place the range of both. A word of another place starts a new pair.
    pub fn give(&mut self, place: K, word: Word) -> Paired {
        match self.first.take() {
            Some((first_place, first)) if first_place == place => Paired::Range(Word { from: first.from.min(word.from), to: first.to.max(word.to) }),
            _ => {
                self.first = Some((place, word));
                Paired::First(word)
            }
        }
    }

    /// Forgets the first word (another cut was made, the place changed).
    pub fn forget(&mut self) {
        self.first = None;
    }

    pub fn waiting(&self) -> bool {
        self.first.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_brings_a_taken_edge_to_the_word() {
        let word = Word::new(100, 112);
        assert_eq!(word.goal(Some(Edge::In), 0), 100, "an IN to its start");
        assert_eq!(word.goal(Some(Edge::Out), 0), 112, "an OUT to its end");
        assert_eq!(word.goal(None, 103), 100, "a cut to the nearer edge");
        assert_eq!(word.goal(None, 110), 112);
        assert_eq!(Word::new(5, 5).to, 6, "a word is at least a frame");
    }

    #[test]
    fn two_words_are_the_range_from_the_earlier_start_to_the_later_end() {
        let mut pair = WordPair::new();
        assert_eq!(pair.give(1, Word::new(200, 210)), Paired::First(Word::new(200, 210)));
        assert!(pair.waiting());
        assert_eq!(pair.give(1, Word::new(100, 108)), Paired::Range(Word::new(100, 210)), "whichever comes first");
        assert!(!pair.waiting());
        pair.give(1, Word::new(10, 20));
        assert_eq!(pair.give(2, Word::new(30, 40)), Paired::First(Word::new(30, 40)), "another place starts a new pair");
    }
}

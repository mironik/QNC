//! Searching the transcript (user 2026-11-05, MA Script media overlay: "Potraži: and a field
//! for the term"). The term and the find that is current are the transcript's (docs/99 step 4:
//! not egui memory): the field sends what is typed and Enter, the transcript finds the words in
//! its lines, lights them and, on Enter, puts the playhead on the next find.

use crate::Line;

/// What is typed in the search field (`target`: the term).
pub const SEARCH_TERM: &str = "transcript_search_term";
/// Enter in the search field: the next find, the playhead there.
pub const SEARCH_NEXT: &str = "transcript_search_next";

/// The search as the transcript panel and its field show it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchView {
    pub term: String,
    /// The finds and which one is current (zero based).
    pub count: usize,
    pub at: usize,
    /// The words of every find, clip frames `[from, to)`.
    pub lit: Vec<(u64, u64)>,
    /// The words of the current find.
    pub current: Vec<(u64, u64)>,
    /// Grows when the current find is to be brought into view (a new term, Enter).
    pub bring: u64,
}

impl SearchView {
    /// The term typed; the first find is current.
    pub(crate) fn set_term(&mut self, term: &str, lines: &[Line]) {
        if term == self.term {
            return;
        }
        self.term = term.to_string();
        self.at = 0;
        self.found(lines);
        if !self.current.is_empty() {
            self.bring += 1;
        }
    }

    /// Enter: the next find is current; its first word for the playhead.
    pub(crate) fn next(&mut self, lines: &[Line]) -> Option<(u64, u64)> {
        self.found(lines);
        if self.count == 0 {
            return None;
        }
        self.at = (self.at + 1) % self.count;
        self.found(lines);
        self.bring += 1;
        self.current.first().copied()
    }

    /// The finds made again from `lines` (a new term, other lines), the current kept in range.
    pub(crate) fn found(&mut self, lines: &[Line]) {
        let words: Vec<(&str, u64, u64)> = lines.iter().flat_map(line_words).collect();
        let all = finds(&self.term, &words);
        self.count = all.len();
        self.at = self.at.min(all.len().saturating_sub(1));
        self.current = all.get(self.at).cloned().unwrap_or_default();
        self.lit = all.into_iter().flatten().collect();
    }
}

/// The words of a line as (text, from, to); a line without words is one word.
pub fn line_words(line: &Line) -> Vec<(&str, u64, u64)> {
    if line.words.is_empty() {
        vec![(line.text.as_str(), line.from, line.to)]
    } else {
        line.words.iter().map(|w| (w.text.as_str(), w.from, w.to.max(w.from + 1))).collect()
    }
}

/// The finds of the term in `words` (every word of the transcript in order, as (text, from,
/// to)): the words of each find, in order. A term of several words is found where they follow
/// one another; a word matches when it holds its piece of the term, whatever the case and
/// punctuation.
pub fn finds(term: &str, words: &[(&str, u64, u64)]) -> Vec<Vec<(u64, u64)>> {
    let pieces: Vec<String> = term.split_whitespace().map(plain).filter(|piece| !piece.is_empty()).collect();
    if pieces.is_empty() {
        return Vec::new();
    }
    let said: Vec<String> = words.iter().map(|(text, _, _)| plain(text)).collect();
    (0..words.len().saturating_sub(pieces.len() - 1))
        .filter(|&at| pieces.iter().enumerate().all(|(k, piece)| said[at + k].contains(piece.as_str())))
        .map(|at| words[at..at + pieces.len()].iter().map(|(_, from, to)| (*from, *to)).collect())
        .collect()
}

fn plain(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_term_of_one_or_more_words_is_found_whatever_the_case_and_punctuation() {
        let words = [("Ovaj", 0, 10), ("skandal", 10, 20), ("sa", 20, 30), ("Skandalom,", 30, 40), ("sa", 40, 50)];
        assert_eq!(finds("skandal", &words), vec![vec![(10, 20)], vec![(30, 40)]]);
        assert_eq!(finds("Skandal sa", &words), vec![vec![(10, 20), (20, 30)], vec![(30, 40), (40, 50)]]);
        assert_eq!(finds("  ", &words), Vec::<Vec<(u64, u64)>>::new());
        assert_eq!(finds("nema", &words), Vec::<Vec<(u64, u64)>>::new());
    }

    /// docs/99 step 4: the term and the current find are the transcript's; Enter goes round the
    /// finds and asks the playhead on the first word of the next.
    #[test]
    fn the_term_is_the_transcripts_and_enter_goes_round_its_finds() {
        let line = |from: u64, text: &str| Line { from, to: from + 10, text: text.into(), words: Vec::new() };
        let lines = [line(0, "skandal"), line(10, "nista"), line(20, "Skandalom")];
        let mut search = SearchView::default();
        search.set_term("skandal", &lines);
        assert_eq!((search.count, search.at, search.current.clone(), search.bring), (2, 0, vec![(0, 10)], 1));
        assert_eq!(search.next(&lines), Some((20, 30)));
        assert_eq!(search.next(&lines), Some((0, 10)), "round to the first");
        assert_eq!(search.bring, 3);
        search.set_term("nema", &lines);
        assert_eq!((search.count, search.next(&lines)), (0, None));
    }
}

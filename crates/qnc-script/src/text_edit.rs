//! A statement edited as text (user 2026-11-06: "maybe the best option is a plain classic text
//! edit: what is deleted in the text is deleted in the sound"). The words of a part are its text;
//! the user deletes words (or, where the letters are laid, letters) in it as in any editor, and
//! what the edited text no longer says is cut out of the part's picture and sound together. Text
//! typed in is not said anywhere and is not kept: only what was said can stay.

use crate::TimedWord;

/// The text of a part's words, as the editor shows it: one space between words.
pub fn text_of(words: &[TimedWord]) -> String {
    words.iter().map(|word| word.text.as_str()).collect::<Vec<_>>().join(" ")
}

/// One thing said, kept or not: a word, or a letter of a word whose letters are laid.
struct Unit {
    from: u64,
    to: u64,
    kept: bool,
}

/// What of the part `[from, to)` the edited text still says: the clip ranges kept, in order.
/// A word the text no longer has is cut from its start to its end; a word only shortened keeps
/// its remaining letters when they are laid, else stays whole. Cut at the part's start or end,
/// the part is only shorter; a pause between two kept words stays.
pub fn kept_ranges(words: &[TimedWord], edited: &str, (from, to): (u64, u64)) -> Vec<(u64, u64)> {
    let edited: Vec<&str> = edited.split_whitespace().collect();
    let said: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
    let pairs = align(&said, &edited, |a, b| a == b);
    let mut units = Vec::new();
    let mut at = (0usize, 0usize);
    let step = |units: &mut Vec<Unit>, at: (usize, usize), word_end: usize, edit_end: usize| {
        // The words between two matched ones: each keeps what an edited token in its place still
        // has of it (in order), the rest is gone.
        let gap_words = at.0..word_end;
        let gap_edits: Vec<&str> = edited[at.1..edit_end].to_vec();
        let mut edits = gap_edits.iter().peekable();
        for word in &words[gap_words] {
            let shortened = edits.peek().filter(|edit| is_within(edit, &word.text)).copied();
            if let Some(edit) = shortened {
                edits.next();
                units.extend(letters_kept(word, edit));
            } else {
                units.push(Unit { from: word.from, to: word.to.max(word.from + 1), kept: false });
            }
        }
    };
    for (word, edit) in pairs {
        step(&mut units, at, word, edit);
        units.push(Unit { from: words[word].from, to: words[word].to.max(words[word].from + 1), kept: true });
        at = (word + 1, edit + 1);
    }
    step(&mut units, at, words.len(), edited.len());
    cut(&units, (from, to))
}

/// The part's range less every run of units not kept.
fn cut(units: &[Unit], (from, to): (u64, u64)) -> Vec<(u64, u64)> {
    let mut kept: Vec<(u64, u64)> = Vec::new();
    let mut start = from;
    let mut index = 0;
    while index < units.len() {
        if units[index].kept {
            index += 1;
            continue;
        }
        let first = index;
        while index < units.len() && !units[index].kept {
            index += 1;
        }
        let cut_from = if first == 0 { from } else { units[first].from.max(start) };
        let cut_to = if index == units.len() { to } else { units[index - 1].to.min(units[index].from) };
        if cut_from > start {
            kept.push((start, cut_from.min(to)));
        }
        start = cut_to.max(start);
    }
    if start < to {
        kept.push((start, to));
    }
    kept.retain(|(from, to)| to > from);
    kept
}

/// The letters of `word` the edited token `edit` still has, in order (a letter deleted is cut);
/// a word without laid letters stays whole.
fn letters_kept(word: &TimedWord, edit: &str) -> Vec<Unit> {
    let laid = !word.letters.is_empty() && word.letters.len() == word.text.chars().count() && word.letters.iter().all(|(_, span)| span.is_some());
    if !laid || edit == word.text {
        return vec![Unit { from: word.from, to: word.to.max(word.from + 1), kept: true }];
    }
    let letters: Vec<char> = word.text.chars().collect();
    let edit: Vec<char> = edit.chars().collect();
    let pairs = align(&letters, &edit, |a, b| a == b);
    let kept: Vec<bool> = (0..letters.len()).map(|at| pairs.iter().any(|(letter, _)| *letter == at)).collect();
    word.letters
        .iter()
        .zip(kept)
        .filter_map(|((_, span), kept)| span.map(|(from, to)| Unit { from, to: to.max(from + 1), kept }))
        .collect()
}

/// Whether `edit` is what is left of `word` with some letters deleted (its letters in order).
fn is_within(edit: &str, word: &str) -> bool {
    let mut letters = word.chars();
    !edit.is_empty() && edit.chars().all(|c| letters.any(|letter| letter == c))
}

/// The longest common subsequence of `a` and `b`: the index pairs matched, in order.
fn align<T>(a: &[T], b: &[T], same: impl Fn(&T, &T) -> bool) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut table = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[i][j] = if same(&a[i], &b[j]) { table[i + 1][j + 1] + 1 } else { table[i + 1][j].max(table[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut pairs) = (0, 0, Vec::new());
    while i < n && j < m {
        if same(&a[i], &b[j]) {
            pairs.push((i, j));
            (i, j) = (i + 1, j + 1);
        } else if table[i + 1][j] >= table[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn word(text: &str, from: u64, to: u64) -> TimedWord {
        TimedWord { from, to, text: text.into(), tc: String::new(), line: false, letters: Vec::new() }
    }

    fn said() -> Vec<TimedWord> {
        vec![word("ovo", 100, 110), word("je", 112, 116), word("ono", 120, 130), word("pa", 132, 136), word("dobro", 140, 160)]
    }

    #[test]
    fn what_the_edited_text_no_longer_says_is_cut_out() {
        let words = said();
        assert_eq!(text_of(&words), "ovo je ono pa dobro");
        assert_eq!(kept_ranges(&words, "ovo je ono pa dobro", (95, 170)), vec![(95, 170)], "nothing deleted");
        assert_eq!(kept_ranges(&words, "ovo je ono dobro", (95, 170)), vec![(95, 132), (136, 170)], "a filler in the middle");
        assert_eq!(kept_ranges(&words, "ovo je dobro", (95, 170)), vec![(95, 120), (136, 170)], "two words in a row: one cut");
        assert_eq!(kept_ranges(&words, "je ono pa dobro", (95, 170)), vec![(110, 170)], "at the start: only shorter");
        assert_eq!(kept_ranges(&words, "ovo je ono pa", (95, 170)), vec![(95, 140)], "at the end: only shorter");
        assert_eq!(kept_ranges(&words, "ovo   je  ono, pa dobro novo", (95, 170)), vec![(95, 120), (130, 170)], "a changed word is gone, typed text is not kept");
        assert!(kept_ranges(&words, "", (95, 170)).is_empty(), "all of it");
    }

    #[test]
    fn letters_deleted_from_a_word_are_cut_where_they_are_laid() {
        let mut words = said();
        words[4].letters = vec![('d', Some((140, 144))), ('o', Some((144, 148))), ('b', Some((148, 152))), ('r', Some((152, 156))), ('o', Some((156, 160)))];
        assert_eq!(kept_ranges(&words, "ovo je ono pa dbro", (95, 170)), vec![(95, 144), (148, 170)], "one letter");
        // Without laid letters a shortened word stays whole.
        assert_eq!(kept_ranges(&said(), "ovo je ono pa dbro", (95, 170)), vec![(95, 170)]);
    }
}

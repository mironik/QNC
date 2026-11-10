//! The readings of the OFFs in the words of a recording that has no presses between them (user
//! 2026-11-10: a camera, a recorder, LAN or intranet): where each reading starts and ends is
//! found from the text. From left to right, a reading starts at a word that is one of the first
//! words of an OFF and is the stretch of words that reads that OFF best; a stretch never runs
//! past the place where the same OFF starts again (a reading broken off and started anew is two
//! readings, the first usually too short to count). Words that read no OFF are counted.

use qnc_read_check::{check, Verdict};

use crate::{Off, LEAST_PERCENT};

/// A reading found: of OFF `off`, heard words `first..=last`, and how much of it was read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Found {
    pub off: usize,
    pub first: usize,
    pub last: usize,
    pub percent: u32,
}

/// How many of the first written words of an OFF a reading may start with (a word dropped or
/// misheard at the start).
const OPENING_WORDS: usize = 3;

fn words_of(text: &str) -> Vec<&str> {
    text.split_whitespace().collect()
}

/// Whether two words are the same word for the engine (case, punctuation, a small slip).
fn same(written: &str, heard: &str) -> bool {
    check(written, &[heard]).percent == 100
}

/// Whether the reading of `off` could start at heard word `at`.
fn opens(off: &[&str], heard: &[&str], at: usize) -> bool {
    off.iter().take(OPENING_WORDS).any(|word| same(word, heard[at]))
}

/// Whether the first two written words of `off` are heard again at `at` (the reading starts anew).
fn starts_again(off: &[&str], heard: &[&str], at: usize) -> bool {
    match off {
        [one] => same(one, heard[at]),
        [one, two, ..] => at + 1 < heard.len() && same(one, heard[at]) && same(two, heard[at + 1]),
        [] => false,
    }
}

/// The best stretch of `heard` from `at` that reads `off`: its last word, percent and score.
fn best_from(text: &str, off: &[&str], heard: &[&str], at: usize) -> Option<(usize, u32, f64)> {
    let n = off.len();
    // A stretch never runs into the same OFF read again.
    let stop = (at + 2..heard.len()).find(|&k| starts_again(off, heard, k)).unwrap_or(heard.len());
    let shortest = (n * 6).div_ceil(10).max(1);
    let longest = (n * 14 / 10 + 2).min(stop - at);
    let mut best: Option<(usize, u32, f64)> = None;
    for length in shortest..=longest {
        let stretch = &heard[at..at + length];
        let checked = check(text, stretch);
        let used: Vec<usize> = checked.words.iter().filter(|word| word.verdict == Verdict::Read).filter_map(|word| word.heard).collect();
        // The stretch starts and ends on words of the text.
        if used.first() != Some(&0) || used.last() != Some(&(length - 1)) {
            continue;
        }
        let score = used.len() as f64 / n as f64 - 0.5 * checked.extra.len() as f64 / n as f64;
        if checked.percent >= LEAST_PERCENT && best.is_none_or(|(_, _, kept)| score > kept) {
            best = Some((at + length - 1, checked.percent, score));
        }
    }
    best
}

/// The readings of `offs` in `heard`, in the order heard.
pub fn find(offs: &[Off], heard: &[&str]) -> Vec<Found> {
    let written: Vec<Vec<&str>> = offs.iter().map(|off| words_of(&off.text)).collect();
    let mut found = Vec::new();
    let mut at = 0;
    while at < heard.len() {
        let best = offs
            .iter()
            .enumerate()
            .filter(|(index, _)| !written[*index].is_empty() && opens(&written[*index], heard, at))
            .filter_map(|(index, off)| best_from(&off.text, &written[index], heard, at).map(|(last, percent, score)| (index, last, percent, score)))
            .max_by(|a, b| a.3.total_cmp(&b.3));
        match best {
            Some((off, last, percent, _)) => {
                found.push(Found { off, first: at, last, percent });
                at = last + 1;
            }
            None => at += 1,
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn off(id: i64, text: &str) -> Off {
        Off { block_id: id, mark: format!("OFF {id}"), text: text.into() }
    }

    fn heard(text: &str) -> Vec<&str> {
        text.split_whitespace().collect()
    }

    #[test]
    fn readings_without_presses_are_found_from_the_text() {
        let offs = [off(1, "danas je u gradu otvoren novi most"), off(2, "radovi su trajali punih pet godina")];
        let words = heard("evo idemo danas je u gradu otvoren novi most pa radovi su trajali punih pet godina hvala");
        let found = find(&offs, &words);
        assert_eq!(found.iter().map(|f| (f.off, f.first, f.last)).collect::<Vec<_>>(), [(0, 2, 8), (1, 10, 15)]);
    }

    #[test]
    fn a_reading_read_again_is_a_second_reading_and_a_broken_one_does_not_eat_it() {
        let offs = [off(1, "danas je u gradu otvoren novi most")];
        let words = heard("danas je u gradu otvoren novi most danas je u ne danas je u gradu otvoren novi most");
        let found = find(&offs, &words);
        assert_eq!(found.iter().map(|f| (f.first, f.last)).collect::<Vec<_>>(), [(0, 6), (11, 17)], "{found:?}");
    }

    #[test]
    fn talk_that_reads_no_off_is_left_out() {
        let offs = [off(1, "danas je u gradu otvoren novi most")];
        assert!(find(&offs, &heard("proba proba jedan dva tri")).is_empty());
    }
}

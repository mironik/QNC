//! A written text against the words heard in a recording of it (user 2026-11-08: "while the OFF
//! is read wav2vec2 transcribes it, and that checks that it was read correctly"). Every written
//! word is read, read wrong or missed; what was said beyond the text is listed. The engine errs
//! on words read correctly, so a small difference counts as read; it writes numbers as digits, so
//! a number heard in digits covers the number words written (and the other way round). Pure:
//! reads and writes nothing.

use std::ops::Range;

pub const MODULE_ID: &str = "qnc.module.read-check";

const MISSED: u32 = 2;
const EXTRA: u32 = 2;
/// Under a missed word and an extra one together: two different words on one place are one
/// word read wrong.
const WRONG: u32 = 3;
const CLOSE: u32 = 1;
/// The most number words one number covers ("tisucu devetsto sezdeset devete").
const NUMBER_WORDS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Read,
    Wrong,
    Missed,
}

/// A written word: where it is in the text and how it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedWord {
    pub at: Range<usize>,
    pub verdict: Verdict,
    /// The heard word it was matched with (an index of what `check` was given).
    pub heard: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadCheck {
    pub words: Vec<CheckedWord>,
    /// Words heard that the text does not have.
    pub extra: Vec<String>,
    /// The whole of what was heard.
    pub heard: String,
    /// Of the written words, how many in a hundred were read (100 only when all were).
    pub percent: u32,
}

#[derive(Clone, Copy)]
enum Step {
    Start,
    Same,
    Wrong,
    Missed,
    Extra,
    /// One heard number covers this many written words.
    NumberHeard(usize),
    /// One written number covers this many heard words.
    NumberWritten(usize),
}

/// A token without its signs, in small letters.
fn plain(word: &str) -> String {
    word.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// A word without the marks of its letters.
fn folded(word: &str) -> Vec<char> {
    word.chars()
        .map(|c| match c {
            'č' | 'ć' => 'c',
            'š' => 's',
            'ž' => 'z',
            'đ' => 'd',
            other => other,
        })
        .collect()
}

fn distance(a: &[char], b: &[char]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut before = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let cost = before + usize::from(x != y);
            before = row[j + 1];
            row[j + 1] = cost.min(row[j] + 1).min(before + 1);
        }
    }
    row[b.len()]
}

/// The cost of two words on one place: 0 the same, `CLOSE` a difference the engine makes.
fn likeness(a: &str, b: &str) -> u32 {
    if a == b {
        return 0;
    }
    let (a, b) = (folded(a), folded(b));
    if a == b {
        return 0;
    }
    let allowed = match a.len().max(b.len()) {
        0..=3 => 0,
        4..=7 => 1,
        _ => 2,
    };
    if distance(&a, &b) <= allowed {
        CLOSE
    } else {
        WRONG
    }
}

fn is_digits(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_digit())
}

/// A Croatian word of a number, cardinal or ordinal; it counts only beside a number in digits,
/// so taking a word too many costs nothing.
fn is_number_word(word: &str) -> bool {
    let word: String = folded(word).into_iter().collect();
    const STEMS: [&str; 16] = ["nula", "jedan", "jedn", "dva", "dvije", "dvjest", "tri", "cetir", "cetvrt", "pet", "sest", "sedam", "sedm", "osam", "devet", "deset"];
    const OTHER: [&str; 12] = ["sto", "stotin", "tisuc", "milijun", "milijard", "prv", "drug", "trec", "osm", "stot", "cetrdeset", "pedeset"];
    STEMS.iter().chain(OTHER.iter()).any(|stem| word.starts_with(stem)) || word.ends_with("naest") || word.ends_with("naesti") || word.ends_with("deset")
}

/// `written` against the words `heard`, in order.
pub fn check(written: &str, heard: &[&str]) -> ReadCheck {
    let tokens: Vec<(Range<usize>, String)> = written
        .split_whitespace()
        .filter_map(|word| {
            let start = word.as_ptr() as usize - written.as_ptr() as usize;
            let plain = plain(word);
            (!plain.is_empty()).then(|| (start..start + word.len(), plain))
        })
        .collect();
    let spoken: Vec<(usize, &str, String)> = heard.iter().enumerate().map(|(index, word)| (index, *word, plain(word))).filter(|(_, _, plain)| !plain.is_empty()).collect();
    let (n, m) = (tokens.len(), spoken.len());
    let width = m + 1;
    let mut cost = vec![u32::MAX; (n + 1) * width];
    let mut step = vec![Step::Start; (n + 1) * width];
    cost[0] = 0;
    let at = |i: usize, j: usize| i * width + j;
    for i in 0..=n {
        for j in 0..=m {
            let here = cost[at(i, j)];
            if here == u32::MAX {
                continue;
            }
            let mut offer = |to: usize, price: u32, how: Step| {
                if here + price < cost[to] {
                    cost[to] = here + price;
                    step[to] = how;
                }
            };
            if i < n {
                offer(at(i + 1, j), MISSED, Step::Missed);
            }
            if j < m {
                offer(at(i, j + 1), EXTRA, Step::Extra);
            }
            if i < n && j < m {
                let price = likeness(&tokens[i].1, &spoken[j].2);
                offer(at(i + 1, j + 1), price, if price == WRONG { Step::Wrong } else { Step::Same });
            }
            if j < m && is_digits(&spoken[j].2) {
                for count in (1..=NUMBER_WORDS).take_while(|count| i + count <= n && is_number_word(&tokens[i + count - 1].1)) {
                    offer(at(i + count, j + 1), 0, Step::NumberHeard(count));
                }
            }
            if i < n && is_digits(&tokens[i].1) {
                for count in (1..=NUMBER_WORDS).take_while(|count| j + count <= m && is_number_word(&spoken[j + count - 1].2)) {
                    offer(at(i + 1, j + count), 0, Step::NumberWritten(count));
                }
            }
        }
    }
    let mut verdicts = vec![Verdict::Missed; n];
    let mut matched = vec![None; n];
    let mut extra = Vec::new();
    let (mut i, mut j) = (n, m);
    loop {
        match step[at(i, j)] {
            Step::Start => break,
            Step::Same => {
                (i, j) = (i - 1, j - 1);
                verdicts[i] = Verdict::Read;
                matched[i] = Some(spoken[j].0);
            }
            Step::Wrong => {
                (i, j) = (i - 1, j - 1);
                verdicts[i] = Verdict::Wrong;
                matched[i] = Some(spoken[j].0);
            }
            Step::Missed => i -= 1,
            Step::Extra => {
                j -= 1;
                extra.push(spoken[j].1.to_string());
            }
            Step::NumberHeard(count) => {
                j -= 1;
                for _ in 0..count {
                    i -= 1;
                    verdicts[i] = Verdict::Read;
                }
                matched[i] = Some(spoken[j].0);
            }
            Step::NumberWritten(count) => {
                i -= 1;
                j -= count;
                verdicts[i] = Verdict::Read;
                matched[i] = Some(spoken[j].0);
            }
        }
    }
    extra.reverse();
    let read = verdicts.iter().filter(|verdict| **verdict == Verdict::Read).count();
    let percent = if n == 0 { 0 } else { (read * 100 / n) as u32 };
    let words = tokens.into_iter().zip(verdicts).zip(matched).map(|(((at, _), verdict), heard)| CheckedWord { at, verdict, heard }).collect();
    ReadCheck { words, extra, heard: heard.join(" "), percent }
}

/// Where the words of `written` start, in bytes (a mark in the text is the number of one of them).
pub fn word_starts(written: &str) -> Vec<usize> {
    written.split_whitespace().map(|word| word.as_ptr() as usize - written.as_ptr() as usize).collect()
}

/// The second of a recording `seconds` long at which word `word` of `written` is said (user
/// 2026-11-08: M markers in the text of an OFF): the time of the heard word matched with it or the
/// next one matched; with no check, or nothing matched after it, its share of the words.
pub fn word_second(written: &str, check: Option<(&ReadCheck, &[f64])>, seconds: f64, word: usize) -> f64 {
    let starts = word_starts(written);
    let share = seconds * word as f64 / starts.len().max(1) as f64;
    let Some((&at, (check, heard))) = starts.get(word).zip(check) else { return share };
    check.words.iter().filter(|checked| checked.at.start >= at).find_map(|checked| heard.get(checked.heard?).copied()).unwrap_or(share)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verdicts(check: &ReadCheck) -> Vec<Verdict> {
        check.words.iter().map(|word| word.verdict).collect()
    }

    #[test]
    fn a_text_read_whole_is_all_read_whatever_its_signs_and_capitals() {
        let done = check("Ovo je, naravno: prva vijest.", &["ovo", "je", "naravno", "prva", "vijest"]);
        assert_eq!(done.percent, 100);
        assert!(done.extra.is_empty());
        assert_eq!(done.words.len(), 5);
    }

    #[test]
    fn a_word_skipped_is_missed_and_another_word_is_wrong() {
        let done = check("danas je lijepo vrijeme", &["danas", "lijepo", "kolega"]);
        assert_eq!(verdicts(&done), [Verdict::Read, Verdict::Missed, Verdict::Read, Verdict::Wrong]);
        assert_eq!(done.percent, 50);
    }

    #[test]
    fn what_was_said_beyond_the_text_is_listed() {
        let done = check("prva rijec", &["prva", "ovaj", "rijec"]);
        assert_eq!(done.percent, 100);
        assert_eq!(done.extra, ["ovaj"]);
    }

    #[test]
    fn a_small_difference_of_the_engine_counts_as_read_a_short_word_must_be_the_same() {
        assert_eq!(check("novinarstvu čovjek", &["novinarsvu", "covjek"]).percent, 100);
        assert_eq!(check("da", &["ne"]).percent, 0);
    }

    #[test]
    fn a_number_in_digits_covers_its_number_words_and_the_other_way_round() {
        let done = check("godine tisuću devetsto šezdeset devete u Zagrebu", &["godine", "1969.", "u", "zagrebu"]);
        assert_eq!(done.percent, 100, "{:?}", verdicts(&done));
        assert!(done.extra.is_empty());
        assert_eq!(check("ima 42 godine", &["ima", "cetrdeset", "dva", "godine"]).percent, 100);
    }

    #[test]
    fn a_word_is_found_in_the_text_by_its_bytes_whatever_the_letters_before_it() {
        let text = "čač  Šuma.";
        let done = check(text, &["cac", "suma"]);
        assert_eq!(&text[done.words[1].at.clone()], "Šuma.");
    }

    #[test]
    fn a_word_is_at_the_time_it_was_heard_else_at_the_next_heard_else_its_share() {
        let text = "prva druga treca cetvrta";
        let done = check(text, &["prva", "treca", "cetvrta"]);
        let heard = [0.5, 2.0, 3.0];
        assert_eq!(word_second(text, Some((&done, &heard)), 8.0, 2), 2.0);
        assert_eq!(word_second(text, Some((&done, &heard)), 8.0, 1), 2.0, "the word missed: the next one heard");
        assert_eq!(word_second(text, None, 8.0, 1), 2.0, "not recorded: its share");
    }

    #[test]
    fn nothing_heard_is_nothing_read() {
        let done = check("prva rijec", &[]);
        assert_eq!(done.percent, 0);
        assert_eq!(verdicts(&done), [Verdict::Missed, Verdict::Missed]);
    }
}

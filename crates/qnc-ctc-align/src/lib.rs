//! CTC forced alignment (user 2026-11-05: cutting by letters in the script): the text that is
//! known (the words of a statement) is laid on the letter probabilities a speech model heard,
//! frame by frame, and every letter gets the frames it is said in. Pure: the model gives the
//! probabilities, the owner turns frames into time.
//!
//! The words are written as the model's letters: lower case, letters the model has not (a
//! comma, a hyphen) get no frames and are left out of the path. The word gap (the model's
//! space) may be heard or not: speech often runs one word into the next.

pub const MODULE_ID: &str = "qnc.module.ctc-align";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The frames `[from, to)` a letter is said in.
pub type Span = (usize, usize);

/// The letters of one word with their frames (`None`: a sign the model has not).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WordLetters {
    pub letters: Vec<(char, Option<Span>)>,
}

impl WordLetters {
    /// The frames of the whole word: from its first heard letter to its last.
    pub fn span(&self) -> Option<Span> {
        let mut heard = self.letters.iter().filter_map(|(_, span)| *span);
        let first = heard.next()?;
        Some(heard.fold(first, |(from, to), (_, end)| (from.min(end), to.max(end))))
    }
}

/// One state of the path: the blank or a token, and whether it may be skipped.
#[derive(Clone, Copy)]
struct State {
    token: usize,
    skippable: bool,
    /// The letter (word, letter) this token is, `None` for the blank and the word gap.
    letter: Option<(usize, usize)>,
}

/// Lays `words` on `frames` (`frames[t][token]`: log probabilities). `tokens`: the letter of
/// every token id; `blank`: the CTC blank; `space`: the word gap token, if the model has one.
/// `None`: the text cannot be laid on so few frames.
pub fn align(frames: &[Vec<f32>], tokens: &[String], blank: usize, space: Option<usize>, words: &[&str]) -> Option<Vec<WordLetters>> {
    let id_of = |letter: char| -> Option<usize> {
        let lower: String = letter.to_lowercase().collect();
        tokens.iter().position(|token| *token == lower)
    };
    // The path: blank, letter, blank, letter, ..., with an optional word gap between words.
    let mut states = vec![State { token: blank, skippable: true, letter: None }];
    let mut out: Vec<WordLetters> = Vec::new();
    for (word_index, word) in words.iter().enumerate() {
        let mut letters = Vec::new();
        let mut first = true;
        for (letter_index, letter) in word.chars().enumerate() {
            letters.push((letter, None));
            let Some(token) = id_of(letter) else { continue };
            if first && word_index > 0 && !out.iter().all(|word| word.letters.iter().all(|(letter, _)| id_of(*letter).is_none())) {
                if let Some(space) = space {
                    states.push(State { token: space, skippable: true, letter: None });
                    states.push(State { token: blank, skippable: true, letter: None });
                }
            }
            first = false;
            states.push(State { token, skippable: false, letter: Some((word_index, letter_index)) });
            states.push(State { token: blank, skippable: true, letter: None });
        }
        out.push(WordLetters { letters });
    }
    let (count, time) = (states.len(), frames.len());
    if time == 0 || states.iter().filter(|state| !state.skippable).count() > time {
        return None;
    }
    // Viterbi over the states: from itself, the one before, or past a skippable one.
    let worst = f32::NEG_INFINITY;
    let mut score = vec![worst; count];
    let mut back = vec![0u8; count * time];
    let emit = |t: usize, s: usize| frames[t].get(states[s].token).copied().unwrap_or(worst);
    score[0] = emit(0, 0);
    if count > 1 {
        score[1] = emit(0, 1);
    }
    for t in 1..time {
        let mut next = vec![worst; count];
        for s in 0..count {
            let mut best = (score[s], 0u8);
            for step in 1..=4u8 {
                let Some(from) = s.checked_sub(usize::from(step)) else { break };
                // Every state between `from` and `s` is skipped: only when it may be.
                let skipped_ok = (from + 1..s).all(|between| states[between].skippable);
                // A letter right after the same letter needs a blank between them.
                let same = step >= 2 && states[s].letter.is_some() && states[from].token == states[s].token;
                if skipped_ok && !same && score[from] > best.0 {
                    best = (score[from], step);
                }
            }
            if best.0 > worst {
                next[s] = best.0 + emit(t, s);
                back[t * count + s] = best.1;
            }
        }
        score = next;
    }
    // The path ends on the last letter or the blank after it.
    let mut s = (count.saturating_sub(2)..count).max_by(|&a, &b| score[a].total_cmp(&score[b]))?;
    if score[s] == worst {
        return None;
    }
    let mut path = vec![0usize; time];
    for t in (0..time).rev() {
        path[t] = s;
        s -= usize::from(back[t * count + s]);
    }
    for (t, &s) in path.iter().enumerate() {
        if let Some((word, letter)) = states[s].letter {
            let span = &mut out[word].letters[letter].1;
            *span = Some(span.map_or((t, t + 1), |(from, _)| (from, t + 1)));
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Vec<String> {
        [" ", "[PAD]", "a", "b", "e", "j"].iter().map(|token| token.to_string()).collect()
    }

    /// Frames where the given token is certain.
    fn frames(heard: &[usize]) -> Vec<Vec<f32>> {
        heard.iter().map(|&token| (0..6).map(|id| if id == token { -0.01 } else { -8.0 }).collect()).collect()
    }

    #[test]
    fn every_letter_gets_the_frames_it_is_said_in() {
        // "ja bbe": j a _ blank b blank b e — with blanks and the word gap heard.
        let heard = [1, 5, 5, 2, 1, 0, 1, 3, 1, 3, 4, 4, 1];
        let words = align(&frames(&heard), &tokens(), 1, Some(0), &["Ja,", "bbe"]).unwrap();
        assert_eq!(words[0].letters, vec![('J', Some((1, 3))), ('a', Some((3, 4))), (',', None)]);
        assert_eq!(words[1].letters, vec![('b', Some((7, 8))), ('b', Some((9, 10))), ('e', Some((10, 12)))]);
        assert_eq!(words[1].span(), Some((7, 12)));
    }

    #[test]
    fn the_word_gap_may_be_unheard_and_too_few_frames_lay_nothing() {
        let heard = [5, 2, 3, 4];
        let words = align(&frames(&heard), &tokens(), 1, Some(0), &["ja", "be"]).unwrap();
        assert_eq!(words[1].letters[0].1, Some((2, 3)), "no word gap heard: b right after a");
        assert!(align(&frames(&[5, 2]), &tokens(), 1, Some(0), &["ja", "be"]).is_none());
    }
}

//! The edges of transcribed words moved to the sound under them (user 2026-10-26: "the words
//! must be where their sound is, so the text can be used to cut statements"). The
//! transcription engine gives word times on 80 ms steps, often several frames early or late
//! (measured on Mironik 2680: "Pa" said at 865, given 856; "dosad" at 873, given 868), so each
//! word's start goes to the nearest onset of voice (where the voice rises after quiet or after
//! a dip), in order, and its end to where its voice falls before the next word; voice no word
//! covers is given back as voice no word was heard for. Pure: reads and writes nothing.

pub const MODULE_ID: &str = "qnc.module.word-align";

/// Rises this close (frames) are one onset.
const ONSET_MERGE: u64 = 2;
/// How far a word's start may move to an onset (frames).
const SNAP_REACH: u64 = 10;
/// A rise of this much over the frame before it (out of a dip) is an onset (dB).
const ONSET_RISE_DB: f32 = 9.0;
/// The farthest from a word's last voice its voice may go on after a dip (frames).
const MAX_DIP: u64 = 5;
/// How far a last word's end may run on past its given end (frames).
const LAST_END_REACH: u64 = 10;
/// The shortest voice no word covers that is given back (frames, 80 ms at 50 fps).
const UNHEARD_MIN: u64 = 4;
/// The lowest voice: -42 dBFS in per mille of full scale.
const VOICE_FLOOR: u16 = 8;

/// A word's frames, `to` past its last frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub from: u64,
    pub to: u64,
}

/// The words moved to the sound and the voice no word covers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aligned {
    pub words: Vec<Span>,
    pub unheard: Vec<Span>,
}

/// `words` (in order) of the window `[from, to)` moved to the sound whose column peaks
/// (per mille of full scale, evenly over the window) are `peaks`.
pub fn align(from: u64, to: u64, peaks: &[u16], words: &[Span]) -> Aligned {
    let levels = frame_levels(from, to, peaks);
    if levels.is_empty() || words.is_empty() {
        return Aligned { words: words.to_vec(), unheard: Vec::new() };
    }
    let threshold = voice_threshold(&levels);
    let loud = |frame: u64| frame >= from && frame < to && levels[(frame - from) as usize] > threshold;
    let db = |frame: u64| decibels(levels[(frame - from) as usize]);
    // Onsets: voice rising out of quiet or out of a dip that goes on for a while (a syllable,
    // not a click or a breath); rises a frame or two apart are one onset, its first frame.
    let sustained = |frame: u64| (frame..frame + 4).filter(|&f| loud(f)).count() >= 3;
    let mut onsets: Vec<u64> = Vec::new();
    for frame in from..to {
        let rises = loud(frame) && (frame == from || !loud(frame - 1) || db(frame) - db(frame - 1) >= ONSET_RISE_DB);
        if rises && sustained(frame) && onsets.last().is_none_or(|&last| frame > last + ONSET_MERGE) {
            onsets.push(frame);
        }
    }
    // Starts: words onto onsets in order with the least moving in all (one onset a word, a
    // word may keep the engine's time at the cost of a full reach), so one word's nearest
    // onset is not taken from the word it belongs to.
    let given: Vec<u64> = words.iter().map(|w| w.from.clamp(from, to.saturating_sub(1))).collect();
    // A word kept where the engine put it must be on voice: one given in the quiet takes an
    // onset (user 2026-10-28: "a" and "ono" equally loud, the engine early on both).
    let keepable: Vec<bool> = given.iter().map(|&frame| loud(frame)).collect();
    let mut starts = assign(&given, &keepable, &onsets);
    // Two words said as one ("A ono" heard as "aono", Mironik 3452, user 2026-10-28) share one
    // onset: the later word took it and the earlier one was left in the quiet before it. A word
    // never starts in quiet with nothing heard until the next word: it takes that word's voice,
    // and the next word starts where the engine put it, after it.
    for index in 0..starts.len().saturating_sub(1) {
        let (start, next) = (starts[index], starts[index + 1]);
        if !loud(start) && loud(next) && (start..next).all(|frame| !loud(frame)) {
            let limit = starts.get(index + 2).copied().unwrap_or(to).max(next + 2);
            starts[index] = next;
            starts[index + 1] = given[index + 1].clamp(next + 1, limit - 1);
        }
    }
    // Ends: a word's voice runs on through dips of a few frames, never into the next word's
    // start; the last word no further than a little past its given end.
    let mut aligned = Vec::with_capacity(words.len());
    for (index, (word, &start)) in words.iter().zip(&starts).enumerate() {
        let limit = match starts.get(index + 1) {
            Some(&next) => next,
            None => (word.to.max(start + 1) + LAST_END_REACH).min(to),
        };
        let mut end = start + 1;
        let mut frame = start;
        while frame < limit && frame - end.saturating_sub(1).max(start) <= MAX_DIP {
            if loud(frame) {
                end = frame + 1;
            }
            frame += 1;
        }
        aligned.push(Span { from: start, to: end.min(limit.max(start + 1)) });
    }
    let mut unheard = Vec::new();
    let mut run: Option<u64> = None;
    for frame in from..=to {
        let voice = frame < to && loud(frame) && !aligned.iter().any(|w| w.from <= frame && frame < w.to);
        match (voice, run) {
            (true, None) => run = Some(frame),
            (false, Some(begin)) => {
                if frame - begin >= UNHEARD_MIN {
                    unheard.push(Span { from: begin, to: frame });
                }
                run = None;
            }
            _ => {}
        }
    }
    Aligned { words: aligned, unheard }
}


/// Each word's start: an onset (in order, each once, at most `SNAP_REACH` from where the engine
/// put it) or its given frame, with the least moving in all; a word kept where it was given
/// costs a full reach and more, so a near onset is taken whenever the order allows it.
fn assign(given: &[u64], keepable: &[bool], onsets: &[u64]) -> Vec<u64> {
    let (n, m) = (given.len(), onsets.len());
    let keep = SNAP_REACH + 2;
    // Kept in the quiet: only when no onset is in reach.
    let quiet = 1000;
    // best[i][j]: the least cost of words i.. with onsets j..; choice[i][j]: the onset taken.
    let mut best = vec![vec![0u64; m + 1]; n + 1];
    let mut choice = vec![vec![None; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..=m).rev() {
            let mut cost = if keepable[i] { keep } else { quiet } + best[i + 1][j];
            let mut taken = None;
            for k in j..m {
                let distance = onsets[k].abs_diff(given[i]);
                if distance <= SNAP_REACH && distance + best[i + 1][k + 1] < cost {
                    cost = distance + best[i + 1][k + 1];
                    taken = Some(k);
                }
            }
            best[i][j] = cost;
            choice[i][j] = taken;
        }
    }
    let mut starts = Vec::with_capacity(n);
    let (mut j, mut floor) = (0, 0u64);
    for i in 0..n {
        let start = match choice[i][j] {
            Some(k) => {
                j = k + 1;
                onsets[k]
            }
            None => given[i],
        }
        .max(floor);
        starts.push(start);
        floor = start + 1;
    }
    starts
}

/// The loudest column of each frame of the window.
fn frame_levels(from: u64, to: u64, peaks: &[u16]) -> Vec<u16> {
    if to <= from || peaks.is_empty() {
        return Vec::new();
    }
    let frames = (to - from) as usize;
    let columns = peaks.len();
    (0..frames)
        .map(|frame| {
            let first = frame * columns / frames;
            let last = ((frame + 1) * columns / frames).max(first + 1).min(columns);
            peaks[first.min(columns - 1)..last].iter().copied().max().unwrap_or(0)
        })
        .collect()
}

/// Voice is 6 dB over the quiet of the window (its fifth quietest level) and no more than
/// 18 dB under its loud (its tenth loudest), never under -42 dBFS: a breath or the room
/// between words is not voice.
fn voice_threshold(levels: &[u16]) -> u16 {
    let mut sorted = levels.to_vec();
    sorted.sort_unstable();
    let quiet = sorted[sorted.len() / 5];
    let loud = sorted[sorted.len() * 9 / 10];
    quiet.saturating_mul(2).max(loud / 8).max(VOICE_FLOOR)
}

fn decibels(peak: u16) -> f32 {
    20.0 * (f32::from(peak.max(1)) / 1000.0).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One column per frame: each run its level, quiet 4 elsewhere.
    fn sound(voice: &[(u64, u64, u16)], frames: u64) -> Vec<u16> {
        (0..frames).map(|f| voice.iter().find(|&&(a, b, _)| a <= f && f < b).map_or(4, |&(_, _, level)| level)).collect()
    }

    #[test]
    fn starts_given_early_or_late_go_to_the_onsets_of_their_voice() {
        // Measured on Mironik 2680: a breath before "Pa", then "Pa" 865, "dosad" 873, "njih" 881.
        let peaks = sound(&[(847, 860, 5), (865, 871, 60), (873, 876, 60), (881, 891, 25)], 1000);
        let words = [Span { from: 856, to: 860 }, Span { from: 868, to: 880 }, Span { from: 884, to: 888 }];
        let aligned = align(800, 1000, &peaks[800..].to_vec(), &words);
        assert_eq!(aligned.words, vec![Span { from: 865, to: 871 }, Span { from: 873, to: 876 }, Span { from: 881, to: 891 }]);
        assert!(aligned.unheard.is_empty());
    }

    #[test]
    fn a_rise_out_of_a_dip_is_an_onset_too() {
        let mut peaks = sound(&[(10, 40, 200)], 100);
        peaks[20] = 40;
        peaks[21] = 40;
        let aligned = align(0, 100, &peaks, &[Span { from: 11, to: 18 }, Span { from: 25, to: 30 }]);
        assert_eq!(aligned.words, vec![Span { from: 10, to: 22 }, Span { from: 22, to: 40 }]);
    }

    #[test]
    fn voice_no_word_covers_is_given_back() {
        let peaks = sound(&[(10, 20, 200), (40, 50, 200), (70, 80, 200)], 100);
        let aligned = align(0, 100, &peaks, &[Span { from: 10, to: 20 }, Span { from: 70, to: 80 }]);
        assert_eq!(aligned.unheard, vec![Span { from: 40, to: 50 }]);
    }

    #[test]
    fn many_columns_per_frame_take_the_loudest() {
        let mut peaks = vec![4u16; 400];
        peaks[41] = 300;
        assert_eq!(frame_levels(0, 100, &peaks)[10], 300);
    }
}

#[cfg(test)]
mod measured {
    use super::*;

    /// Peaks of each frame of Mironik 2680 channel 1, frames 820 on (read from the card).
    const PEAKS: [u16; 110] = [5,8,6,7,4,6,5,6,5,5,5,4,4,4,4,3,4,5,5,4,4,5,5,5,5,5,5,10,6,9,11,10,13,11,15,13,13,10,7,8,6,6,4,6,19,119,143,155,163,170,131,19,10,107,137,120,52,25,15,14,13,71,71,30,25,30,37,38,43,51,50,22,15,47,76,38,117,121,116,115,93,42,4,10,28,22,16,61,74,90,97,87,93,94,78,66,53,65,82,86,87,49,25,30,139,162,153,97,68,49];

    #[test]
    fn the_words_of_mironik_2680_go_onto_their_sound() {
        let words = [Span { from: 856, to: 860 }, Span { from: 868, to: 880 }, Span { from: 884, to: 888 }, Span { from: 892, to: 916 }, Span { from: 916, to: 924 }];
        let aligned = align(820, 930, &PEAKS, &words);
        let starts: Vec<u64> = aligned.words.iter().map(|w| w.from).collect();
        // "Pa" from its first sound (864, a frame before its loud vowel), "dosad" at the red cut.
        assert_eq!(starts[..3], [864, 873, 881], "{aligned:?}");
    }

    /// Peaks of each frame of Mironik 3452 channel 1, frames 2377 on (read from the card; the
    /// zoom of IN 2427): quiet, then "A ono" said as one ("aono") from 2418, "što" 2431, "nas"
    /// 2444 (user 2026-10-28: "where it says ono it is a, the man says aono").
    const AONO: [u16; 100] = [59,58,37,18,25,29,32,32,26,85,152,168,201,215,205,181,159,107,10,5,7,33,36,20,10,1,2,3,5,5,6,8,7,7,5,4,4,5,4,2,18,53,93,121,121,140,188,191,180,201,171,134,46,59,191,169,140,134,132,151,99,24,44,41,5,2,7,136,195,185,142,59,154,136,146,152,155,90,23,27,27,34,61,34,9,39,83,83,81,74,41,22,25,24,39,98,163,172,173,163];

    #[test]
    fn two_words_said_as_one_do_not_leave_the_first_in_the_quiet() {
        // The engine's words: Hrvatski, sabor, A, ono, što, nas, sve, zanima.
        let given = [(2357, 2382), (2386, 2413), (2413, 2421), (2421, 2441), (2441, 2449), (2449, 2457), (2457, 2473), (2474, 2498)];
        let words: Vec<Span> = given.iter().map(|&(from, to)| Span { from, to }).collect();
        let aligned = align(2377, 2477, &AONO, &words);
        let starts: Vec<u64> = aligned.words.iter().map(|w| w.from).collect();
        assert_eq!(starts[2..5], [2418, 2431, 2444], "{aligned:?}");
    }
}

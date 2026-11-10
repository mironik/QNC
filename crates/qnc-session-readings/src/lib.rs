//! The readings of a recording of the whole story, each put on the OFF it reads (user 2026-11-09:
//! "from the text the program should recognise which is the first, the second or some third off",
//! OFF 1a, OFF 1b, OFF 2a...). The presses cut the recording; every piece is heard by the Croatian
//! engine of this computer and compared with the text of every OFF; a piece that reads none of
//! them is left out and counted. Runs off the form thread. Written only through the script's
//! database and the catalog's writer; the recording itself stays.

use qnc_read_check::check;
use qnc_script_db::{HeardWord, VoiceTake};
use qnc_still_locator::StillLocator;
use std::sync::atomic::AtomicBool;

pub const MODULE_ID: &str = "qnc.module.session-readings";

/// Of the written words, how many in a hundred a piece must read to be that OFF.
pub(crate) const LEAST_PERCENT: u32 = 40;

mod found;
pub use found::{find, Found};

/// An OFF of the script a piece may read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Off {
    pub block_id: i64,
    pub mark: String,
    pub text: String,
}

/// What became of a recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sorted {
    pub placed: usize,
    pub left: usize,
}

/// The OFF that `heard` reads best (the earlier of two equally read); none below the least.
pub fn best_off(offs: &[Off], heard: &[&str]) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (index, off) in offs.iter().enumerate() {
        let percent = check(&off.text, heard).percent;
        if percent >= LEAST_PERCENT && best.is_none_or(|(_, kept)| percent > kept) {
            best = Some((index, percent));
        }
    }
    best.map(|(index, _)| index)
}

/// Hears every piece of the recording and writes it as the next reading of its OFF.
pub fn sort(locator: &StillLocator, wav_uri: &str, timecode: &str, marks: &[f64], offs: &[Off], silent: bool) -> Result<Sorted, String> {
    if let Some(reason) = qnc_take_heard::barred(locator) {
        return Err(reason);
    }
    let settings = locator.settings();
    let target = qnc_db_broker::ProjectDbTarget::for_project(locator.reader(), settings)?;
    let file = qnc_voice_take::voice_file(locator, wav_uri)?;
    let (samples, rate) = qnc_voice_record::read_take(&file)?;
    let language = settings.speech_language().map_err(|e| e.to_string())?;
    let maker = qnc_voice_take::ReadingMaker::for_project(locator)?;
    let mut voices = qnc_script_db::read_voices(&target).map_err(|e| e.to_string())?;
    let mut outcome = Sorted { placed: 0, left: 0 };
    for (from, to) in qnc_voice_take::readings(samples.len(), rate, marks) {
        let piece = &samples[from..to];
        let engine = qnc_transcribe_jobs::to_whisper_rate(piece, rate)?;
        let transcript = qnc_transcribe_run::hear_samples(&engine, &language, &AtomicBool::new(false))?;
        let words: Vec<HeardWord> = transcript.segments.iter().flat_map(|segment| &segment.words).map(|word| HeardWord { text: word.text.trim().to_string(), start: word.start, end: word.end }).filter(|word| !word.text.is_empty()).collect();
        let heard: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
        let Some(off) = best_off(offs, &heard).map(|index| &offs[index]) else {
            outcome.left += 1;
            continue;
        };
        let voice = voices.iter().find(|voice| voice.block_id == off.block_id);
        let name = voice.map_or_else(|| qnc_voice_take::record_name(&settings.project_name, &off.mark), |voice| voice.name.clone());
        let number = voice.and_then(|voice| voice.takes.last()).map_or(0, |take| take.take) + 1;
        let made = maker.make(&name, number, piece, rate, timecode, &off.mark, silent)?;
        qnc_voice_catalog::publish_record(locator.reader(), settings, &qnc_script_db::take_name(&name, number), made.record.clone())?;
        let take = VoiceTake { take: number, wav_uri: made.wav_uri.clone(), seconds: made.seconds, tc: made.timecode.clone(), text: off.text.clone(), heard: Some(words), source: None };
        qnc_script_db::add_voice_take(&target, off.block_id, name, made.clip_uri, take).map_err(|e| e.to_string())?;
        voices = qnc_script_db::read_voices(&target).map_err(|e| e.to_string())?;
        outcome.placed += 1;
    }
    Ok(outcome)
}

/// How much sound a reading found in another recording keeps before its first word and after
/// its last (a breath, the end of the last sound), never into the next reading.
const AROUND_SECONDS: f64 = 0.5;

/// Hears frames `[from, to)` of clip `clip_id` (a recording made elsewhere: no presses between
/// its readings) on the project's speech channel, finds its readings from the text and writes
/// each as the next reading of its OFF: a range of the clip, no file of its own (user
/// 2026-11-10). Nothing is copied or cut, so it is the same for a clip on a card, LAN or
/// intranet that this computer reads.
pub fn sort_clip(locator: &StillLocator, clip_id: &str, from: u64, to: u64, offs: &[Off]) -> Result<Sorted, String> {
    if let Some(reason) = qnc_take_heard::barred(locator) {
        return Err(reason);
    }
    let settings = locator.settings();
    let target = qnc_db_broker::ProjectDbTarget::for_project(locator.reader(), settings)?;
    let place = locator.locate_sound(clip_id)?;
    let channel = u16::try_from(settings.transcribe_channel().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let (_, _, _, rate) = place.channel(channel).ok_or_else(|| format!("no camera channel {}", channel + 1))?;
    let mut samples = Vec::new();
    qnc_sound_range::stream_from_start(&place, channel, from, to, &AtomicBool::new(false), &mut |piece, _, _| samples.extend_from_slice(piece))?;
    let language = settings.speech_language().map_err(|e| e.to_string())?;
    let engine = qnc_transcribe_jobs::to_whisper_rate(&samples, rate)?;
    let transcript = qnc_transcribe_run::hear_samples(&engine, &language, &AtomicBool::new(false))?;
    let words: Vec<HeardWord> = transcript.segments.iter().flat_map(|segment| &segment.words).map(|word| HeardWord { text: word.text.trim().to_string(), start: word.start, end: word.end }).filter(|word| !word.text.is_empty()).collect();
    let heard: Vec<&str> = words.iter().map(|word| word.text.as_str()).collect();
    let found = find(offs, &heard);
    let fps = f64::from(place.fps.0) / f64::from(place.fps.1.max(1));
    let tags: Vec<(String, String)> = place.media.tags.iter().map(|(key, fact)| (key.clone(), fact.value.clone())).collect();
    let timecode = qnc_source_timecode::SourceTimecode::from_tags(tags.iter().map(|(key, value)| (key.as_str(), value.as_str())), (u64::from(place.fps.0), u64::from(place.fps.1)));
    let mut voices = qnc_script_db::read_voices(&target).map_err(|e| e.to_string())?;
    let readings = reading_ranges(&found, &words, (from, to), fps);
    for (reading, (first, last)) in found.iter().zip(readings) {
        let off = &offs[reading.off];
        let voice = voices.iter().find(|voice| voice.block_id == off.block_id);
        let name = voice.map_or_else(|| qnc_voice_take::record_name(&settings.project_name, &off.mark), |voice| voice.name.clone());
        let number = voice.and_then(|voice| voice.takes.last()).map_or(0, |take| take.take) + 1;
        let start = (first - from) as f64 / fps;
        let heard_here: Vec<HeardWord> = words[reading.first..=reading.last].iter().map(|word| HeardWord { text: word.text.clone(), start: word.start - start, end: word.end - start }).collect();
        let source = qnc_script_db::TakeSource { clip_id: clip_id.to_string(), from_frame: first, to_frame: last, fps_num: place.fps.0, fps_den: place.fps.1.max(1), channel };
        let take = VoiceTake {
            take: number,
            wav_uri: place.media.media_uri.clone(),
            seconds: (last - first) as f64 / fps,
            tc: timecode.map(|timecode| timecode.label(first)).unwrap_or_default(),
            text: off.text.clone(),
            heard: Some(heard_here),
            source: Some(source),
        };
        qnc_script_db::add_voice_take(&target, off.block_id, name, place.media.media_uri.clone(), take).map_err(|e| e.to_string())?;
        voices = qnc_script_db::read_voices(&target).map_err(|e| e.to_string())?;
    }
    Ok(Sorted { placed: found.len(), left: talk_left(&found, words.len()) })
}

/// The stretches of three words or more between the readings found: talk that reads no OFF.
pub fn talk_left(found: &[Found], words: usize) -> usize {
    let mut edges = vec![0];
    for reading in found {
        edges.push(reading.first);
        edges.push(reading.last + 1);
    }
    edges.push(words);
    edges.chunks(2).filter(|gap| gap[1].saturating_sub(gap[0]) >= 3).count()
}

/// The frames of every reading found: its words with `AROUND_SECONDS` either side, never past
/// the middle of the gap to its neighbour readings, within `range`.
pub fn reading_ranges(found: &[Found], words: &[HeardWord], (from, to): (u64, u64), fps: f64) -> Vec<(u64, u64)> {
    let frame = |seconds: f64| (from + (seconds.max(0.0) * fps).round() as u64).min(to);
    found
        .iter()
        .enumerate()
        .map(|(index, reading)| {
            let (start, end) = (words[reading.first].start, words[reading.last].end);
            let before = index.checked_sub(1).map_or(0.0, |previous| (words[found[previous].last].end + start) / 2.0);
            let after = found.get(index + 1).map_or(f64::INFINITY, |next| (end + words[next.first].start) / 2.0);
            let first = frame((start - AROUND_SECONDS).max(before));
            let last = frame((end + AROUND_SECONDS).min(after)).max(first + 1);
            (first, last.min(to.max(first + 1)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn off(id: i64, text: &str) -> Off {
        Off { block_id: id, mark: format!("OFF {id}"), text: text.into() }
    }

    #[test]
    fn a_found_reading_keeps_half_a_second_around_its_words_but_not_into_the_next() {
        let word = |text: &str, start: f64, end: f64| HeardWord { text: text.into(), start, end };
        let words = [word("a", 1.0, 1.4), word("b", 1.5, 2.0), word("c", 2.2, 2.5), word("d", 2.6, 3.0)];
        let found = [Found { off: 0, first: 0, last: 1, percent: 100 }, Found { off: 1, first: 2, last: 3, percent: 100 }];
        // At 50 fps from frame 100: the first 0.5 s before its first word, the gap of 0.2 s halved.
        assert_eq!(reading_ranges(&found, &words, (100, 1000), 50.0), [(125, 205), (205, 275)]);
        assert_eq!(talk_left(&found, 4), 0);
        assert_eq!(talk_left(&found[1..], 4), 0, "two words before are not talk");
        assert_eq!(talk_left(&found[..1], 6), 1, "four words after are");
    }

    #[test]
    fn a_piece_goes_to_the_off_it_reads_and_nowhere_when_it_reads_none() {
        let offs = [off(1, "danas je u gradu otvoren novi most"), off(2, "radovi su trajali punih pet godina")];
        assert_eq!(best_off(&offs, &["radovi", "su", "trajali", "pet", "godina"]), Some(1));
        assert_eq!(best_off(&offs, &["danas", "je", "u", "gradu", "otvoren"]), Some(0));
        assert_eq!(best_off(&offs, &["sasvim", "druga", "prica"]), None);
    }
}

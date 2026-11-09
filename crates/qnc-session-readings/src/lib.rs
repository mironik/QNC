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
const LEAST_PERCENT: u32 = 40;

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
        let take = VoiceTake { take: number, wav_uri: made.wav_uri.clone(), seconds: made.seconds, tc: made.timecode.clone(), text: off.text.clone(), heard: Some(words) };
        qnc_script_db::add_voice_take(&target, off.block_id, name, made.clip_uri, take).map_err(|e| e.to_string())?;
        voices = qnc_script_db::read_voices(&target).map_err(|e| e.to_string())?;
        outcome.placed += 1;
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn off(id: i64, text: &str) -> Off {
        Off { block_id: id, mark: format!("OFF {id}"), text: text.into() }
    }

    #[test]
    fn a_piece_goes_to_the_off_it_reads_and_nowhere_when_it_reads_none() {
        let offs = [off(1, "danas je u gradu otvoren novi most"), off(2, "radovi su trajali punih pet godina")];
        assert_eq!(best_off(&offs, &["radovi", "su", "trajali", "pet", "godina"]), Some(1));
        assert_eq!(best_off(&offs, &["danas", "je", "u", "gradu", "otvoren"]), Some(0));
        assert_eq!(best_off(&offs, &["sasvim", "druga", "prica"]), None);
    }
}

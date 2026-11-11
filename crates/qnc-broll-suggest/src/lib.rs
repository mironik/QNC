//! B-roll suggested for the words of an OFF (user 2026-11-10: "the OFF, or a sentence of the OFF,
//! is the query"; "for breaking news, suggestions by themselves"). The query is the story itself:
//! the slot chosen (between two M markers: a sentence of the OFF) asks for one cover, else the OFF
//! under the Wrap playhead asks for a series, one cover a slot of it. Each slot gives the frames
//! its cover needs and the words A1 says there (a clip's transcript, or the words heard in an OFF
//! reading); the words become vectors of the same shot-content program the pictures were looked
//! at with (SigLIP 2 reads Croatian). The series gives each slot, in order, the usable part most
//! like its words that is long enough and not given to an earlier slot. It reads the project
//! database (transcripts, voice readings) off the form thread and writes nothing.

use std::sync::mpsc;

use qnc_db_broker::ProjectDbTarget;
use qnc_program_view::{SegmentKind, SegmentsView};

pub const MODULE_ID: &str = "qnc.module.broll-suggest";

/// What A1 plays in a slot: a clip and its frames `[from, to)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub clip_id: String,
    pub from: u64,
    pub to: u64,
}

/// One slot of an ask: its id, how long its cover is and what A1 plays there, or the words it
/// covers when they are written, not yet said (user 2026-11-11: the text of the script).
#[derive(Debug, Clone, PartialEq)]
pub struct SlotAsk {
    pub slot_id: String,
    pub seconds: f64,
    pub pieces: Vec<Piece>,
    pub text: String,
}

/// What the story asks for: one slot (a sentence of the OFF) or the slots of an OFF (a series).
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    /// The slot or the OFF segment asked about.
    pub key: String,
    /// The story's frames a second (an OFF reading's words are seconds of its clip).
    pub fps: f64,
    pub slots: Vec<SlotAsk>,
}

/// One slot's suggestion: its words and their vector.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotSuggestion {
    pub slot_id: String,
    pub seconds: f64,
    pub text: String,
    pub vector: Vec<f32>,
}

/// A suggestion ready: one slot, or the series of an OFF in the story's order.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub key: String,
    pub slots: Vec<SlotSuggestion>,
    /// The vectors of the parts' captions asked with it (by the same program as the words).
    pub captions: std::collections::HashMap<String, Vec<f32>>,
}

/// What the story asks for now: the chosen slot, else the empty slots of the OFF under the Wrap
/// playhead (inside that OFF); their A1 pieces from the story's own placement of every segment
/// (`wave_placements`), in its clips' frames.
pub fn ask_of(view: &SegmentsView) -> Option<Ask> {
    let (num, den) = view.timebase?;
    if num == 0 || den == 0 {
        return None;
    }
    let seconds = |frames: u64| frames as f64 * f64::from(den) / f64::from(num);
    let placements = view.wave_placements().0;
    let slot_ask = |slot_id: &str, from: u64, to: u64| SlotAsk { slot_id: slot_id.to_string(), seconds: seconds(to - from), pieces: pieces_in(&placements, from, to), text: String::new() };
    if let Some(slot) = view.selected_slot().filter(|slot| slot.end_frame > slot.start_frame) {
        return Some(Ask { key: slot.slot_id.clone(), fps: f64::from(num) / f64::from(den), slots: vec![slot_ask(&slot.slot_id, slot.start_frame, slot.end_frame)] });
    }
    let playhead = view.playhead?;
    let off = view.rows.iter().find(|row| row.kind == SegmentKind::Off && !row.lead && row.start_frame <= playhead && playhead < row.end_frame)?;
    let slots: Vec<SlotAsk> = view
        .slots
        .iter()
        .filter(|slot| !slot.has_cover && slot.start_frame < off.end_frame && off.start_frame < slot.end_frame)
        .map(|slot| (slot, slot.start_frame.max(off.start_frame), slot.end_frame.min(off.end_frame)))
        .filter(|(_, from, to)| to > from)
        .map(|(slot, from, to)| slot_ask(&slot.slot_id, from, to))
        .collect();
    (!slots.is_empty()).then(|| Ask { key: format!("off:{}", off.segment_id), fps: f64::from(num) / f64::from(den), slots })
}

/// What the text of a script asks for (user 2026-11-11: "a mark or a sentence in the text
/// editor gives the query"): its slots, each the words it covers and the seconds they last; the
/// key names what is asked (a changed text or length is asked anew). `None` without words.
pub fn ask_of_text(key: String, slots: Vec<(String, f64)>) -> Option<Ask> {
    let slots: Vec<SlotAsk> = slots
        .into_iter()
        .enumerate()
        .filter(|(_, (text, seconds))| !text.trim().is_empty() && *seconds > 0.0)
        .map(|(n, (text, seconds))| SlotAsk { slot_id: format!("{key}#{n}"), seconds, pieces: Vec::new(), text })
        .collect();
    (!slots.is_empty()).then(|| Ask { key, fps: 0.0, slots })
}

/// The A1 pieces of program frames `[from, to)`, in their clips' frames.
fn pieces_in(placements: &[qnc_program_waveform::Placement], from: u64, to: u64) -> Vec<Piece> {
    placements
        .iter()
        .filter(|placement| !placement.off && placement.program_start < to && from < placement.program_end)
        .map(|placement| {
            let (program, source) = (placement.program_end - placement.program_start, placement.source_out.saturating_sub(placement.source_in));
            let at = |frame: u64| placement.source_in + (frame - placement.program_start) * source / program.max(1);
            Piece { clip_id: placement.clip_id.clone(), from: at(from.max(placement.program_start)), to: at(to.min(placement.program_end)) }
        })
        .collect()
}

/// The words said in `pieces`, in order: a clip's transcript words whose middle is in its piece,
/// an OFF reading's heard words (seconds of its own clip at `fps`).
pub fn words_of(pieces: &[Piece], transcripts: &dyn Fn(&str) -> Vec<qnc_transcript_db::ClipTranscript>, heard: &dyn Fn(&str) -> Option<Vec<qnc_script_db::HeardWord>>, fps: f64) -> String {
    let mut said = Vec::new();
    for piece in pieces {
        let inside = |from: u64, to: u64| (from + to.max(from)) / 2 >= piece.from && (from + to.max(from)) / 2 < piece.to;
        if let Some(words) = heard(&piece.clip_id) {
            said.extend(words.into_iter().filter(|word| inside((word.start * fps).round() as u64, (word.end * fps).round() as u64)).map(|word| word.text));
        } else {
            let mut words: Vec<(u64, String)> = transcripts(&piece.clip_id)
                .into_iter()
                .flat_map(|transcript| transcript.lines.into_iter().flat_map(|line| line.words))
                .filter(|word| inside(word.from, word.to))
                .map(|word| (word.from, word.text))
                .collect();
            words.sort_by_key(|(from, _)| *from);
            words.dedup();
            said.extend(words.into_iter().map(|(_, text)| text));
        }
    }
    said.iter().map(|word| word.trim()).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ")
}

/// How like a part's vector is to a slot's words (cosine of unit vectors).
pub fn likeness(slot: &SlotSuggestion, vector: &[f32]) -> f32 {
    if vector.len() != slot.vector.len() {
        return f32::NEG_INFINITY;
    }
    vector.iter().zip(&slot.vector).map(|(a, b)| a * b).sum()
}

/// A usable part a slot may get: how long it is, the vector of its pictures and the caption a
/// model wrote of it (user 2026-11-11: Florence-2), when there is one.
#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub seconds: f64,
    pub vector: &'a [f32],
    pub caption: Option<&'a str>,
}

/// How well each candidate fits a slot's words: the likeness of its pictures to the words and,
/// where it has a caption, the likeness of the caption to the words (user 2026-11-11: the
/// caption of a shot in the suggestion), each standardised among the candidates (a picture and a
/// text are alike on another scale than two texts) and added; a candidate without a caption
/// counts its pictures only. A part of another model's width is never fit.
pub fn scores(suggestion: &Suggestion, slot: &SlotSuggestion, parts: &[Candidate<'_>]) -> Vec<f32> {
    let pictures: Vec<f32> = parts.iter().map(|part| likeness(slot, part.vector)).collect();
    let captions: Vec<Option<f32>> = parts
        .iter()
        .map(|part| part.caption.and_then(|caption| suggestion.captions.get(caption)).filter(|vector| vector.len() == slot.vector.len()).map(|vector| likeness(slot, vector)))
        .collect();
    let pictures_z = standardised(&pictures.iter().map(|value| value.is_finite().then_some(*value)).collect::<Vec<_>>());
    let captions_z = standardised(&captions);
    pictures
        .iter()
        .zip(pictures_z.iter().zip(&captions_z))
        .map(|(picture, (picture_z, caption_z))| if picture.is_finite() { picture_z.unwrap_or(0.0) + caption_z.unwrap_or(0.0) } else { f32::NEG_INFINITY })
        .collect()
}

/// Each value's distance from the values' mean in their standard deviations (0 when they are all
/// alike); `None` stays `None`.
fn standardised(values: &[Option<f32>]) -> Vec<Option<f32>> {
    let known: Vec<f32> = values.iter().flatten().copied().collect();
    if known.is_empty() {
        return values.to_vec();
    }
    let mean = known.iter().sum::<f32>() / known.len() as f32;
    let deviation = (known.iter().map(|value| (value - mean).powi(2)).sum::<f32>() / known.len() as f32).sqrt();
    values.iter().map(|value| value.map(|value| if deviation > 1e-6 { (value - mean) / deviation } else { 0.0 })).collect()
}

/// The series: for each slot in order, the part (index into `parts`) that fits its words best
/// (`scores`), long enough and not given to an earlier slot; `None` for a slot no part is long
/// enough for.
pub fn series(suggestion: &Suggestion, parts: &[Candidate<'_>]) -> Vec<Option<usize>> {
    let mut given = vec![false; parts.len()];
    suggestion
        .slots
        .iter()
        .map(|slot| {
            let scores = scores(suggestion, slot, parts);
            let best = parts
                .iter()
                .enumerate()
                .filter(|(index, part)| !given[*index] && part.seconds >= slot.seconds && scores[*index].is_finite())
                .map(|(index, _)| (index, scores[index]))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index);
            if let Some(index) = best {
                given[index] = true;
            }
            best
        })
        .collect()
}

/// One OFF of a script to cover (user 2026-11-11: "a button starts the suggestion; terms or a
/// phrase the reporter wrote are taken into account, and the script adds what is needed to cover
/// the OFF"): its block, how long it lasts, its slots (between the M markers of its text: the
/// words and seconds of each; one without markers) and the shots the reporter wants, as written.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanBlock {
    pub block_id: i64,
    pub seconds: f64,
    pub slots: Vec<(String, f64)>,
    pub wanted: Vec<String>,
}

/// A shot planned over an OFF: the part (index into the candidates), the second of the OFF it
/// starts at, how long it covers and the reporter's term it was found for (empty: the words).
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    pub part: usize,
    pub at_seconds: f64,
    pub seconds: f64,
    pub asked: String,
}

/// The shortest shot laid over an OFF without markers (a shot less than this is a flash).
pub const SHORTEST_COVER: f64 = 2.0;

/// The terms of a reporter's note: parted at commas, semicolons and new lines.
pub fn wanted_of(note: &str) -> Vec<String> {
    note.split([',', ';', '\n']).map(str::trim).filter(|term| !term.is_empty()).map(str::to_string).collect()
}

fn slot_key(block: i64, slot: usize) -> String {
    format!("b{block}s{slot}")
}

fn wanted_key(block: i64, term: usize) -> String {
    format!("b{block}w{term}")
}

/// The ask of a plan: every slot's words and every wanted term, made vectors with the words of
/// the project's language translated as for a slot of the story; `key` names this run.
pub fn plan_ask(key: String, blocks: &[PlanBlock]) -> Option<Ask> {
    let mut slots = Vec::new();
    for block in blocks {
        for (n, (text, seconds)) in block.slots.iter().enumerate() {
            slots.push(SlotAsk { slot_id: slot_key(block.block_id, n), seconds: *seconds, pieces: Vec::new(), text: text.clone() });
        }
        for (n, term) in block.wanted.iter().enumerate() {
            slots.push(SlotAsk { slot_id: wanted_key(block.block_id, n), seconds: 0.0, pieces: Vec::new(), text: term.clone() });
        }
    }
    (!slots.is_empty()).then(|| Ask { key, fps: 0.0, slots })
}

/// The cover of every OFF, in the script's order, no part given twice in the whole story:
/// - an OFF with M markers: each slot, in order, gets the part best for the reporter's term of
///   the same place (the first term the first slot) or else for its own words, long enough for
///   the slot;
/// - an OFF without them is covered from its start: first a part for each term in order, then
///   parts best for its words until it is covered; each part covers what it lasts, at most what
///   is left, and is at least `SHORTEST_COVER` (or what is left) long.
/// A slot or term without a fitting part is left out.
pub fn plan(suggestion: &Suggestion, blocks: &[PlanBlock], parts: &[Candidate<'_>]) -> Vec<(i64, Vec<Planned>)> {
    let mut given = vec![false; parts.len()];
    let asked = |id: &str| suggestion.slots.iter().find(|slot| slot.slot_id == id);
    // The best part not yet given at least `least` seconds long for the words of `slot`.
    let best = |slot: &SlotSuggestion, least: f64, given: &mut Vec<bool>| {
        let scores = scores(suggestion, slot, parts);
        let found = (0..parts.len())
            .filter(|n| !given[*n] && parts[*n].seconds >= least && scores[*n].is_finite())
            .max_by(|a, b| scores[*a].total_cmp(&scores[*b]))?;
        given[found] = true;
        Some(found)
    };
    blocks
        .iter()
        .map(|block| {
            let mut laid = Vec::new();
            if block.slots.len() > 1 {
                let mut at = 0.0;
                for (n, (_, seconds)) in block.slots.iter().enumerate() {
                    let term = block.wanted.get(n).and_then(|_| asked(&wanted_key(block.block_id, n)));
                    let query = term.or_else(|| asked(&slot_key(block.block_id, n)));
                    if let Some(part) = query.and_then(|query| best(query, *seconds, &mut given)) {
                        let asked = if term.is_some() { block.wanted[n].clone() } else { String::new() };
                        laid.push(Planned { part, at_seconds: at, seconds: *seconds, asked });
                    }
                    at += seconds;
                }
            } else {
                let mut at = 0.0;
                let lay = |part: usize, asked: String, at: &mut f64, laid: &mut Vec<Planned>| {
                    let seconds = parts[part].seconds.min(block.seconds - *at);
                    laid.push(Planned { part, at_seconds: *at, seconds, asked });
                    *at += seconds;
                };
                for (n, term) in block.wanted.iter().enumerate() {
                    let left = block.seconds - at;
                    if left <= 0.0 {
                        break;
                    }
                    if let Some(part) = asked(&wanted_key(block.block_id, n)).and_then(|query| best(query, SHORTEST_COVER.min(left), &mut given)) {
                        lay(part, term.clone(), &mut at, &mut laid);
                    }
                }
                if let Some(words) = asked(&slot_key(block.block_id, 0)) {
                    while block.seconds - at > 0.1 {
                        let Some(part) = best(words, SHORTEST_COVER.min(block.seconds - at), &mut given) else { break };
                        lay(part, String::new(), &mut at, &mut laid);
                    }
                }
            }
            (block.block_id, laid)
        })
        .collect()
}

struct Job {
    target: ProjectDbTarget,
    ask: Ask,
    captions: Vec<String>,
    language: String,
}

/// The vectors of captions the worker made, kept for the next ask (by the model they were made
/// with).
type CaptionVectors = (std::path::PathBuf, std::collections::HashMap<String, Vec<f32>>);

/// This computer's translator program (by its model) and the translations it gave.
type Translating = (std::path::PathBuf, qnc_text_translate::TranslateClient, std::collections::HashMap<String, String>);

/// The suggestion of what the story asks for, made off the form thread by one worker that keeps
/// the shot-content program started (its model loads once).
#[derive(Default)]
pub struct Suggest {
    asked: Option<Ask>,
    asked_captions: Vec<String>,
    jobs: Option<mpsc::Sender<Job>>,
    answers: Option<mpsc::Receiver<Result<Suggestion, String>>>,
    shown: Option<Suggestion>,
    working: bool,
}

impl std::fmt::Debug for Suggest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Suggest").field("asked", &self.asked).field("working", &self.working).finish()
    }
}

impl Suggest {
    /// Follows what the story asks for: a new ask is worked out, none withdraws the suggestion;
    /// `captions` are the captions of the parts it may suggest, made vectors with its words;
    /// `language` is the project's language of speech (`ai.language`): words of a language this
    /// computer's translator knows are made English first.
    /// Returns whether the suggestion shown changed.
    pub fn follow(&mut self, ask: Option<Ask>, target: Option<&ProjectDbTarget>, captions: &[&str], language: &str) -> bool {
        let mut changed = false;
        if let Some(answers) = &self.answers {
            while let Ok(answer) = answers.try_recv() {
                self.working = false;
                match answer {
                    Ok(suggestion) if self.asked.as_ref().is_some_and(|ask| ask.key == suggestion.key) => {
                        for slot in &suggestion.slots {
                            log(&format!("broll-suggest ask={} slot={} seconds={:.1} text={:?}", suggestion.key, slot.slot_id, slot.seconds, slot.text));
                        }
                        self.shown = Some(suggestion);
                        changed = true;
                    }
                    Ok(_) => {}
                    Err(error) => log(&format!("broll-suggest failed error={error}")),
                }
            }
        }
        // The parts' captions, once each; a new caption of the same ask is worked out again.
        let mut captions: Vec<String> = captions.iter().map(|caption| (*caption).to_string()).collect();
        captions.sort_unstable();
        captions.dedup();
        let same_ask = ask == self.asked;
        if same_ask && (ask.is_none() || captions == self.asked_captions) {
            return changed;
        }
        self.asked_captions.clone_from(&captions);
        if !same_ask {
            self.asked = ask.clone();
            // The suggestion shown stays until the new one comes (a text being typed asks at every
            // key: the cards do not jump back meanwhile); none asked withdraws it.
            if ask.is_none() && self.shown.take().is_some() {
                changed = true;
            }
        }
        let (Some(ask), Some(target)) = (ask, target) else { return changed };
        if self.jobs.is_none() {
            self.start();
        }
        if let Some(jobs) = &self.jobs {
            self.working = jobs.send(Job { target: target.clone(), ask, captions, language: language.to_string() }).is_ok();
        }
        changed
    }

    /// The suggestion of what the story asks for, once worked out.
    pub fn shown(&self) -> Option<&Suggestion> {
        self.shown.as_ref()
    }

    /// A suggestion is being worked out.
    pub fn working(&self) -> bool {
        self.working
    }

    fn start(&mut self) {
        let (jobs, take) = mpsc::channel::<Job>();
        let (send, answers) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("qnc-broll-suggest".into()).spawn(move || {
            let mut client: Option<(std::path::PathBuf, qnc_shot_content::ContentClient)> = None;
            let mut made: CaptionVectors = (std::path::PathBuf::new(), std::collections::HashMap::new());
            let mut translator: Option<Translating> = None;
            while let Ok(mut job) = take.recv() {
                // Only what was asked last counts.
                while let Ok(newer) = take.try_recv() {
                    job = newer;
                }
                if send.send(work(&job, &mut client, &mut made, &mut translator)).is_err() {
                    return;
                }
            }
        });
        if spawned.is_ok() {
            (self.jobs, self.answers) = (Some(jobs), Some(answers));
        }
    }
}

/// The words of an ask's slots and their vectors (a slot without words is left out).
fn work(job: &Job, client: &mut Option<(std::path::PathBuf, qnc_shot_content::ContentClient)>, made: &mut CaptionVectors, translator: &mut Option<Translating>) -> Result<Suggestion, String> {
    let voices = qnc_script_db::read_voices(&job.target).unwrap_or_default();
    let heard = |clip: &str| {
        voices
            .iter()
            .flat_map(|voice| voice.takes.iter().map(move |take| (voice, take)))
            .find(|(voice, take)| qnc_voice_take::clip_id(&qnc_script_db::take_name(&voice.name, take.take)) == clip)
            .map(|(_, take)| take.heard.clone().unwrap_or_default())
    };
    // Each clip's transcripts read once for every slot.
    let mut read = std::collections::HashMap::new();
    for piece in job.ask.slots.iter().flat_map(|slot| &slot.pieces) {
        read.entry(piece.clip_id.clone()).or_insert_with(|| qnc_transcript_db::read_clip(&job.target, &piece.clip_id).unwrap_or_default());
    }
    let transcripts = |clip: &str| read.get(clip).cloned().unwrap_or_default();
    let said: Vec<(&SlotAsk, String)> = job
        .ask
        .slots
        .iter()
        .map(|slot| (slot, if slot.text.trim().is_empty() { words_of(&slot.pieces, &transcripts, &heard, job.ask.fps) } else { slot.text.clone() }))
        .filter(|(_, text)| !text.is_empty())
        .collect();
    if said.is_empty() {
        return Err(format!("{} has no words on A1", job.ask.key));
    }
    let engine = qnc_ai_catalog::installed_shot_content()?;
    if client.as_ref().is_none_or(|(model, _)| *model != engine.model) {
        *client = Some((engine.model.clone(), qnc_shot_content::ContentClient::new(&qnc_plugin_process::beside_application(&engine.program)?)));
    }
    let (_, content) = client.as_ref().ok_or("no shot-content program")?;
    // The words in English where this computer's translator knows their language (user
    // 2026-11-11: measured, SigLIP 2 finds the pictures and the captions of an English sentence
    // better than of the same sentence in Croatian); a translation that fails leaves the words
    // as they are said.
    let asked: Vec<String> = match english(&said.iter().map(|(_, text)| text.clone()).collect::<Vec<_>>(), &job.language, translator) {
        Ok(texts) => texts,
        Err(error) => {
            log(&format!("broll-suggest translation failed error={error}"));
            said.iter().map(|(_, text)| text.clone()).collect()
        }
    };
    let mut vectors = Vec::new();
    for texts in asked.chunks(qnc_shot_content::MOST_AT_ONCE) {
        let request = qnc_shot_content::TextsRequest { model: engine.model.clone(), texts: texts.to_vec() };
        vectors.extend(content.texts(&request)?.vectors);
    }
    let slots = said.into_iter().zip(vectors).map(|((slot, text), vector)| SlotSuggestion { slot_id: slot.slot_id.clone(), seconds: slot.seconds, text, vector }).collect();
    // The parts' captions, each made a vector once a model (a caption the program cannot read
    // leaves the suggestion to the pictures, never stops it).
    if made.0 != engine.model {
        *made = (engine.model.clone(), std::collections::HashMap::new());
    }
    let missing: Vec<String> = job.captions.iter().filter(|caption| !made.1.contains_key(*caption)).cloned().collect();
    for texts in missing.chunks(qnc_shot_content::MOST_AT_ONCE) {
        match content.texts(&qnc_shot_content::TextsRequest { model: engine.model.clone(), texts: texts.to_vec() }) {
            Ok(answer) => made.1.extend(texts.iter().cloned().zip(answer.vectors)),
            Err(error) => {
                log(&format!("broll-suggest captions failed error={error}"));
                break;
            }
        }
    }
    let captions = job.captions.iter().filter_map(|caption| Some((caption.clone(), made.1.get(caption)?.clone()))).collect();
    Ok(Suggestion { key: job.ask.key.clone(), slots, captions })
}

/// `texts` (said in `language`, an `ai.language` code) in English: as they are when English or
/// when this computer has no translator of their language, else translated by its program, kept
/// started for the next ask.
fn english(texts: &[String], language: &str, translator: &mut Option<Translating>) -> Result<Vec<String>, String> {
    if language == "en" {
        return Ok(texts.to_vec());
    }
    let Ok(engine) = qnc_ai_catalog::installed_translator() else { return Ok(texts.to_vec()) };
    if !engine.translates(language) {
        return Ok(texts.to_vec());
    }
    if translator.as_ref().is_none_or(|(model, _, _)| *model != engine.model) {
        *translator = Some((engine.model.clone(), qnc_text_translate::TranslateClient::new(&qnc_plugin_process::beside_application(&engine.program)?), Default::default()));
    }
    let (_, client, given) = translator.as_mut().ok_or("no translator")?;
    // A text asked again (the same OFF, new captions) is not translated again.
    let missing: Vec<String> = texts.iter().filter(|text| !given.contains_key(*text)).cloned().collect();
    if !missing.is_empty() {
        let english = client.translate(&qnc_text_translate::TranslateRequest { model: engine.model.clone(), texts: missing.clone() })?;
        for (said, english) in missing.into_iter().zip(english) {
            log(&format!("broll-suggest translated {said:?} -> {english:?}"));
            given.insert(said, english);
        }
    }
    Ok(texts.iter().map(|text| given.get(text).cloned().unwrap_or_else(|| text.clone())).collect())
}

fn log(line: &str) {
    if qnc_dev_diagnostics::player_diagnostics_enabled() {
        qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, line.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_transcript_db::{ClipTranscript, Line, Word};

    #[test]
    fn the_words_of_a_slot_are_its_pieces_words_in_order() {
        let word = |from, to, text: &str| Word { from, to, text: text.into() };
        let transcript = ClipTranscript {
            clip_id: "c".into(),
            from_frame: 0,
            to_frame: 1000,
            language: "hr".into(),
            engine: "e".into(),
            lines: vec![Line { from: 0, to: 300, text: String::new(), words: vec![word(0, 40, "prije"), word(100, 140, "luka"), word(150, 190, "i"), word(200, 260, "brodovi"), word(400, 420, "poslije")] }],
        };
        let heard = vec![qnc_script_db::HeardWord { text: "more".into(), start: 1.0, end: 1.4 }, qnc_script_db::HeardWord { text: "kasnije".into(), start: 9.0, end: 9.5 }];
        let pieces = [Piece { clip_id: "voice-off1".into(), from: 0, to: 100 }, Piece { clip_id: "c".into(), from: 90, to: 300 }];
        let text = words_of(&pieces, &|clip| if clip == "c" { vec![transcript.clone()] } else { Vec::new() }, &|clip| (clip == "voice-off1").then(|| heard.clone()), 50.0);
        assert_eq!(text, "more luka i brodovi");
    }

    #[test]
    fn a_series_gives_each_slot_its_best_part_long_enough_and_none_twice() {
        let slot = |id: &str, seconds, vector: Vec<f32>| SlotSuggestion { slot_id: id.into(), seconds, text: String::new(), vector };
        let suggestion = Suggestion { key: "off:x".into(), slots: vec![slot("s1", 3.0, vec![1.0, 0.0]), slot("s2", 3.0, vec![1.0, 0.0]), slot("s3", 9.0, vec![0.0, 1.0])], captions: Default::default() };
        const SEA: [f32; 2] = [1.0, 0.0];
        const SEA2: [f32; 2] = [0.8, 0.6];
        const BOOK: [f32; 2] = [0.0, 1.0];
        let part = |seconds, vector: &'static [f32]| Candidate { seconds, vector, caption: None };
        let parts = [part(4.0, &SEA), part(5.0, &SEA2), part(2.0, &BOOK)];
        // s1 takes the sea, s2 the next like it, s3 has none long enough.
        assert_eq!(series(&suggestion, &parts), [Some(0), Some(1), None]);
    }

    #[test]
    fn a_caption_like_the_words_lifts_a_part_its_pictures_alone_would_not() {
        // The words are of a harbour; two parts' pictures are alike to them, the second a little
        // less, but its caption says boats on the water.
        let words = vec![1.0, 0.0, 0.0];
        let slot = SlotSuggestion { slot_id: "s".into(), seconds: 2.0, text: "luka i brodovi".into(), vector: words };
        let mut captions = std::collections::HashMap::new();
        captions.insert("a person standing on the floor".to_string(), vec![0.0, 1.0, 0.0]);
        captions.insert("some boats on the water".to_string(), vec![0.9, 0.0, 0.436]);
        let suggestion = Suggestion { key: "slot:s".into(), slots: vec![slot.clone()], captions };
        let (a, b, c) = ([0.30, 0.95, 0.0], [0.28, 0.96, 0.0], [0.0, 1.0, 0.0]);
        let parts = [
            Candidate { seconds: 3.0, vector: &a, caption: Some("a person standing on the floor") },
            Candidate { seconds: 3.0, vector: &b, caption: Some("some boats on the water") },
            Candidate { seconds: 3.0, vector: &c, caption: None },
        ];
        let scores = scores(&suggestion, &slot, &parts);
        assert!(scores[1] > scores[0], "the caption of the boats lifts it over a picture a little more alike: {scores:?}");
        assert_eq!(series(&suggestion, &parts), [Some(1)]);
        let other = [0.0f32; 2];
        assert_eq!(super::scores(&suggestion, &slot, &[Candidate { seconds: 3.0, vector: &other, caption: None }]), [f32::NEG_INFINITY], "another model's width never fits");
    }

    #[test]
    fn the_text_of_a_script_asks_with_its_own_words_and_lengths() {
        let ask = ask_of_text("script:7".into(), vec![("Ribari su isplovili iz luke.".into(), 2.4), ("  ".into(), 1.0), ("Brodovi su se vratili.".into(), 0.0), ("Navečer.".into(), 0.8)]).unwrap();
        let slots: Vec<(&str, &str, f64)> = ask.slots.iter().map(|slot| (slot.slot_id.as_str(), slot.text.as_str(), slot.seconds)).collect();
        assert_eq!(slots, [("script:7#0", "Ribari su isplovili iz luke.", 2.4), ("script:7#3", "Navečer.", 0.8)], "a slot without words or length asks nothing");
        assert!(ask_of_text("script:7".into(), vec![("".into(), 2.0)]).is_none());
    }

    #[test]
    fn a_plan_covers_an_off_with_the_shots_wanted_first_then_by_its_words_none_twice() {
        let slot = |id: &str, vector: Vec<f32>| SlotSuggestion { slot_id: id.into(), seconds: 0.0, text: String::new(), vector };
        // Words of the first OFF are of the sea; the reporter wants a book first. The second OFF
        // has two marked slots: the first wants a book, the second has only its words (sea).
        let suggestion = Suggestion {
            key: "plan:1".into(),
            slots: vec![slot("b1s0", vec![1.0, 0.0]), slot("b1w0", vec![0.0, 1.0]), slot("b2s0", vec![1.0, 0.0]), slot("b2s1", vec![1.0, 0.0]), slot("b2w0", vec![0.0, 1.0])],
            captions: Default::default(),
        };
        const SEA: [f32; 2] = [1.0, 0.0];
        const SEA2: [f32; 2] = [0.9, 0.436];
        const BOOK: [f32; 2] = [0.0, 1.0];
        const BOOK2: [f32; 2] = [0.2, 0.98];
        let part = |seconds, vector: &'static [f32]| Candidate { seconds, vector, caption: None };
        let parts = [part(3.0, &SEA), part(10.0, &SEA2), part(2.5, &BOOK), part(4.0, &BOOK2)];
        let blocks = [
            PlanBlock { block_id: 1, seconds: 6.0, slots: vec![("more".into(), 6.0)], wanted: vec!["knjiga".into()] },
            PlanBlock { block_id: 2, seconds: 5.0, slots: vec![("a".into(), 3.5), ("b".into(), 1.5)], wanted: vec!["knjiga".into()] },
        ];
        let planned = plan(&suggestion, &blocks, &parts);
        let laid = |n: usize| planned[n].1.iter().map(|p| (p.part, p.at_seconds, p.seconds, p.asked.as_str())).collect::<Vec<_>>();
        // OFF 1: the book (2.5 s) first, then the sea for the 3.5 s left.
        assert_eq!(laid(0), [(2, 0.0, 2.5, "knjiga"), (0, 2.5, 3.0, ""), (1, 5.5, 0.5, "")]);
        // OFF 2: the first slot wants a book: the one left long enough (4 s); the second slot
        // has no sea left (both given) and nothing else long enough: left out.
        assert_eq!(laid(1), [(3, 0.0, 3.5, "knjiga")]);
        assert_eq!(wanted_of("luka, brodovi;\nribari , "), ["luka", "brodovi", "ribari"]);
    }
}

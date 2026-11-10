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

/// One slot of an ask: its id, how long its cover is and what A1 plays there.
#[derive(Debug, Clone, PartialEq)]
pub struct SlotAsk {
    pub slot_id: String,
    pub seconds: f64,
    pub pieces: Vec<Piece>,
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
    let slot_ask = |slot_id: &str, from: u64, to: u64| SlotAsk { slot_id: slot_id.to_string(), seconds: seconds(to - from), pieces: pieces_in(&placements, from, to) };
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

/// The series: for each slot in order, the part (index into `parts`: its seconds and vector)
/// most like its words that is long enough and not given to an earlier slot; `None` for a slot
/// no part is long enough for.
pub fn series(suggestion: &Suggestion, parts: &[(f64, &[f32])]) -> Vec<Option<usize>> {
    let mut given = vec![false; parts.len()];
    suggestion
        .slots
        .iter()
        .map(|slot| {
            let best = parts
                .iter()
                .enumerate()
                .filter(|(index, (seconds, _))| !given[*index] && *seconds >= slot.seconds)
                .map(|(index, (_, vector))| (index, likeness(slot, vector)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(index, _)| index);
            if let Some(index) = best {
                given[index] = true;
            }
            best
        })
        .collect()
}

struct Job {
    target: ProjectDbTarget,
    ask: Ask,
}

/// The suggestion of what the story asks for, made off the form thread by one worker that keeps
/// the shot-content program started (its model loads once).
#[derive(Default)]
pub struct Suggest {
    asked: Option<Ask>,
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
    /// Follows what the story asks for: a new ask is worked out, none withdraws the suggestion.
    /// Returns whether the suggestion shown changed.
    pub fn follow(&mut self, ask: Option<Ask>, target: Option<&ProjectDbTarget>) -> bool {
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
        if ask == self.asked {
            return changed;
        }
        self.asked = ask.clone();
        if self.shown.take().is_some() {
            changed = true;
        }
        let (Some(ask), Some(target)) = (ask, target) else { return changed };
        if self.jobs.is_none() {
            self.start();
        }
        if let Some(jobs) = &self.jobs {
            self.working = jobs.send(Job { target: target.clone(), ask }).is_ok();
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
            while let Ok(mut job) = take.recv() {
                // Only what was asked last counts.
                while let Ok(newer) = take.try_recv() {
                    job = newer;
                }
                if send.send(work(&job, &mut client)).is_err() {
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
fn work(job: &Job, client: &mut Option<(std::path::PathBuf, qnc_shot_content::ContentClient)>) -> Result<Suggestion, String> {
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
        .map(|slot| (slot, words_of(&slot.pieces, &transcripts, &heard, job.ask.fps)))
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
    let mut vectors = Vec::new();
    for texts in said.chunks(qnc_shot_content::MOST_AT_ONCE) {
        let request = qnc_shot_content::TextsRequest { model: engine.model.clone(), texts: texts.iter().map(|(_, text)| text.clone()).collect() };
        vectors.extend(content.texts(&request)?.vectors);
    }
    let slots = said.into_iter().zip(vectors).map(|((slot, text), vector)| SlotSuggestion { slot_id: slot.slot_id.clone(), seconds: slot.seconds, text, vector }).collect();
    Ok(Suggestion { key: job.ask.key.clone(), slots })
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
        let suggestion = Suggestion { key: "off:x".into(), slots: vec![slot("s1", 3.0, vec![1.0, 0.0]), slot("s2", 3.0, vec![1.0, 0.0]), slot("s3", 9.0, vec![0.0, 1.0])] };
        let (sea, sea2, short_book) = ([1.0, 0.0], [0.8, 0.6], [0.0, 1.0]);
        let parts: Vec<(f64, &[f32])> = vec![(4.0, &sea), (5.0, &sea2), (2.0, &short_book)];
        // s1 takes the sea, s2 the next like it, s3 has none long enough.
        assert_eq!(series(&suggestion, &parts), [Some(0), Some(1), None]);
    }
}

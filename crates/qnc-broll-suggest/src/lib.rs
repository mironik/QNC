//! B-roll suggested for the words of an OFF (user 2026-11-10: "the text gives the length, the
//! shots long enough are ranked by how like the text they are"). The slot chosen in the story
//! (between two M markers: a part of the OFF the reporter marked) gives the frames a cover needs
//! and the words A1 says there: a clip's transcript, or the words heard in an OFF reading. Those
//! words become a vector of the same shot-content program the pictures were looked at with (the
//! model's text side reads Croatian, SigLIP 2 is multilingual), and the usable parts long enough
//! are ranked by how like it they are. It reads the project database (transcripts, voice
//! readings) off the form thread and writes nothing.

use std::sync::mpsc;

use qnc_db_broker::ProjectDbTarget;
use qnc_program_view::SegmentsView;

pub const MODULE_ID: &str = "qnc.module.broll-suggest";

/// What A1 plays in the slot: a clip and its frames `[from, to)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub clip_id: String,
    pub from: u64,
    pub to: u64,
}

/// The cover a slot asks for: its id, how long it is and what A1 plays there.
#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    pub slot_id: String,
    pub seconds: f64,
    /// The story's frames a second (an OFF reading's words are seconds of its clip).
    pub fps: f64,
    pub pieces: Vec<Piece>,
}

/// A suggestion ready: the slot, its words and their vector.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub slot_id: String,
    pub seconds: f64,
    pub text: String,
    pub vector: Vec<f32>,
}

/// The cover the chosen slot asks for (none chosen: none); its A1 pieces from the story's own
/// placement of every segment (`wave_placements`), in its clips' frames.
pub fn ask_of(view: &SegmentsView) -> Option<Ask> {
    let slot = view.selected_slot()?;
    let (num, den) = view.timebase?;
    if num == 0 || den == 0 || slot.end_frame <= slot.start_frame {
        return None;
    }
    let seconds = (slot.end_frame - slot.start_frame) as f64 * f64::from(den) / f64::from(num);
    let pieces = view
        .wave_placements()
        .0
        .into_iter()
        .filter(|placement| !placement.off && placement.program_start < slot.end_frame && slot.start_frame < placement.program_end)
        .map(|placement| {
            // The program frames of the slot in the piece, as frames of its clip.
            let (program, source) = (placement.program_end - placement.program_start, placement.source_out.saturating_sub(placement.source_in));
            let at = |frame: u64| placement.source_in + (frame - placement.program_start) * source / program.max(1);
            Piece { clip_id: placement.clip_id.clone(), from: at(slot.start_frame.max(placement.program_start)), to: at(slot.end_frame.min(placement.program_end)) }
        })
        .collect();
    Some(Ask { slot_id: slot.slot_id.clone(), seconds, fps: f64::from(num) / f64::from(den), pieces })
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

/// How like a part's vector is to the suggestion's text (cosine of unit vectors).
pub fn likeness(suggestion: &Suggestion, vector: &[f32]) -> f32 {
    if vector.len() != suggestion.vector.len() {
        return f32::NEG_INFINITY;
    }
    vector.iter().zip(&suggestion.vector).map(|(a, b)| a * b).sum()
}

struct Job {
    target: ProjectDbTarget,
    ask: Ask,
}

/// The suggestion for the slot chosen, made off the form thread by one worker that keeps the
/// shot-content program started (its model loads once).
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
    /// Follows the chosen slot: a new ask is worked out, none withdraws the suggestion. Returns
    /// whether the suggestion shown changed.
    pub fn follow(&mut self, ask: Option<Ask>, target: Option<&ProjectDbTarget>) -> bool {
        let mut changed = false;
        if let Some(answers) = &self.answers {
            while let Ok(answer) = answers.try_recv() {
                self.working = false;
                match answer {
                    Ok(suggestion) if self.asked.as_ref().is_some_and(|ask| ask.slot_id == suggestion.slot_id) => {
                        log(&format!("broll-suggest slot={} seconds={:.1} text={:?}", suggestion.slot_id, suggestion.seconds, suggestion.text));
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

    /// The suggestion of the slot chosen, once worked out.
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
                // Only the slot chosen last counts.
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

/// The words of a job's slot and their vector.
fn work(job: &Job, client: &mut Option<(std::path::PathBuf, qnc_shot_content::ContentClient)>) -> Result<Suggestion, String> {
    let voices = qnc_script_db::read_voices(&job.target).unwrap_or_default();
    let heard = |clip: &str| {
        voices
            .iter()
            .flat_map(|voice| voice.takes.iter().map(move |take| (voice, take)))
            .find(|(voice, take)| qnc_voice_take::clip_id(&qnc_script_db::take_name(&voice.name, take.take)) == clip)
            .map(|(_, take)| take.heard.clone().unwrap_or_default())
    };
    let transcripts = |clip: &str| qnc_transcript_db::read_clip(&job.target, clip).unwrap_or_default();
    let text = words_of(&job.ask.pieces, &transcripts, &heard, job.ask.fps);
    if text.is_empty() {
        return Err(format!("slot {} has no words on A1", job.ask.slot_id));
    }
    let engine = qnc_ai_catalog::installed_shot_content()?;
    if client.as_ref().is_none_or(|(model, _)| *model != engine.model) {
        *client = Some((engine.model.clone(), qnc_shot_content::ContentClient::new(&qnc_plugin_process::beside_application(&engine.program)?)));
    }
    let (_, content) = client.as_ref().ok_or("no shot-content program")?;
    let texts = content.texts(&qnc_shot_content::TextsRequest { model: engine.model.clone(), texts: vec![text.clone()] })?;
    let vector = texts.vectors.into_iter().next().ok_or("no vector of the text")?;
    Ok(Suggestion { slot_id: job.ask.slot_id.clone(), seconds: job.ask.seconds, text, vector })
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
    fn a_part_like_the_text_is_liked_more() {
        let suggestion = Suggestion { slot_id: "s".into(), seconds: 3.0, text: "luka".into(), vector: vec![1.0, 0.0] };
        assert!(likeness(&suggestion, &[0.9, 0.436]) > likeness(&suggestion, &[0.1, 0.995]));
        assert_eq!(likeness(&suggestion, &[1.0]), f32::NEG_INFINITY, "a vector of another model");
    }
}

//! The AI B-roll of a project (user 2026-11-10: "a tab of its own for the AI B-roll", sorted by
//! what the pictures show, in Story, S-Story and MA Video). Every usable part of every clip whose
//! content the background worker looked at, grouped by the label its pictures fit best (in the
//! order of the newsroom's list of labels), the longest first within a label; a click opens the
//! part's clip with its IN/OUT on the source timeline. It reads the content and the label vectors
//! the worker kept (outside the UI thread, again now and then) and the list of labels; it loads
//! no model and writes nothing.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use qnc_clip_row::{ClipRow, TabCard};
use qnc_db_broker::ProjectDbTarget;
use qnc_source_preview::SourcePreview;

pub const MODULE_ID: &str = "qnc.module.ai-broll";

/// A click on a part's card opens its clip with its IN/OUT (target: `clip|from|to`).
pub const OPEN: &str = "ai_broll_open";

const READ_EVERY: Duration = Duration::from_secs(3);

/// A part whose A1 is spoken for at least this share of its frames (words of its transcript) is
/// a statement (user 2026-11-10: "A1 as a help, where it exists").
pub const SPOKEN: f32 = 0.3;

/// The share of clip frames `[from, to)` the words of `transcripts` cover; `None` when no
/// transcript covers the part (nothing is known of its speech then).
pub fn spoken_share(transcripts: &[qnc_transcript_db::ClipTranscript], from: u64, to: u64) -> Option<f32> {
    let covered = transcripts.iter().any(|transcript| transcript.from_frame <= from && to <= transcript.to_frame.max(transcript.from_frame));
    if !covered || to <= from {
        return None;
    }
    let mut spans: Vec<(u64, u64)> = transcripts
        .iter()
        .flat_map(|transcript| transcript.lines.iter().flat_map(|line| line.words.iter()))
        .map(|word| (word.from.max(from), word.to.min(to)))
        .filter(|(a, b)| a < b)
        .collect();
    spans.sort_unstable();
    let (mut said, mut end) = (0u64, from);
    for (a, b) in spans {
        let a = a.max(end);
        if a < b {
            said += b - a;
            end = b;
        }
    }
    Some(said as f32 / (to - from) as f32)
}

/// One usable part as the tab shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct AiPart {
    pub clip_id: String,
    /// Clip frames `[from, to)`.
    pub from: u64,
    pub to: u64,
    /// The label it fits best, its dictionary key and its share among all labels.
    pub label: String,
    pub text: String,
    pub share: f32,
    /// Its A1 is spoken (a transcript of the clip has words for most of it).
    pub spoken: bool,
}

/// What the tab shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiBrollView {
    /// In the order shown.
    pub parts: Vec<AiPart>,
    pub message: String,
}

impl AiBrollView {
    /// The parts of the clips of the pool as cards: "Label · clip", the part's duration, the
    /// clip's poster. A part of a clip the pool has not is left out.
    pub fn cards<'a>(&self, clips: &'a [ClipRow]) -> Vec<TabCard<'a>> {
        self.parts
            .iter()
            .filter_map(|part| {
                let clip = clips.iter().find(|clip| clip.clip_id == part.clip_id)?;
                let label = qnc_ui_text::host().ok().and_then(|texts| texts.get(&part.text).ok()).unwrap_or(&part.label);
                let per_frame = if clip.duration_frames > 0 { clip.duration_seconds / clip.duration_frames as f64 } else { 0.0 };
                Some(TabCard {
                    id: format!("{}|{}|{}", part.clip_id, part.from, part.to),
                    title: qnc_ui_text::fill(if part.spoken { "ai_broll.card_spoken" } else { "ai_broll.card" }, &[("label", label), ("clip", &clip.name)]).into(),
                    seconds: (part.to - part.from) as f64 * per_frame,
                    clip: Some(clip),
                    icon: qnc_clip_row::PoolIcon::None,
                })
            })
            .collect()
    }

    pub fn count(&self) -> usize {
        self.parts.len()
    }
}

/// The parts in the tab's order from the kept content and label vectors and the list of labels:
/// each part under the label it fits best (softmax of the model's logits), the labels in the
/// order of the list, the longest part first within a label.
pub fn sort_parts(
    content: &[qnc_shot_content_db::ClipContent],
    set: &qnc_shot_content_db::LabelSet,
    list: &qnc_shot_labels::Labels,
    speech: &dyn Fn(&str, u64, u64) -> Option<f32>,
) -> Vec<AiPart> {
    // The kept vectors in the order of the list (a label the kept set has not is never best).
    let vectors: Vec<Vec<f32>> = list
        .labels
        .iter()
        .map(|label| set.labels.iter().find(|kept| kept.code == label.code).map_or_else(Vec::new, |kept| kept.vector.clone()))
        .collect();
    let texts = qnc_shot_content::Texts { vectors, scale: set.scale, bias: set.bias };
    let order = |code: &str| list.labels.iter().position(|label| label.code == code).unwrap_or(usize::MAX);
    let mut parts: Vec<(usize, AiPart)> = content
        .iter()
        .flat_map(|clip| clip.parts.iter().map(move |part| (clip, part)))
        .filter_map(|(clip, part)| {
            let logits: Vec<f32> = (0..list.labels.len()).map(|n| if texts.vectors[n].is_empty() { f32::NEG_INFINITY } else { texts.logit(&part.vector, n) }).collect();
            // A statement is not B-roll (user 2026-11-10: "a picture that looks like a statement
            // is not a statement; it must have A1 too"): its pictures look like one and its A1 is
            // spoken. Without a transcript of the clip A1 is not known, the pictures decide.
            let spoken = speech(&clip.clip_id, part.from, part.to).map(|share| share >= SPOKEN);
            if list.is_statement(&logits) && spoken != Some(false) {
                return None;
            }
            let (label, share) = list.shares(&logits).into_iter().next()?;
            // Several people and A1 may be both (user 2026-11-10): it stays, marked spoken.
            let spoken = spoken == Some(true);
            Some((
                order(&label),
                AiPart {
                    clip_id: clip.clip_id.clone(),
                    from: part.from,
                    to: part.to,
                    text: list.text_of(&label).unwrap_or(&label).to_string(),
                    label,
                    share,
                    spoken,
                },
            ))
        })
        .collect();
    parts.sort_by(|(a, x), (b, y)| a.cmp(b).then((y.to - y.from).cmp(&(x.to - x.from))).then(x.clip_id.cmp(&y.clip_id)).then(x.from.cmp(&y.from)));
    parts.into_iter().map(|(_, part)| part).collect()
}

type Read = Result<Vec<AiPart>, String>;

/// The AI B-roll of the project on the preview.
#[derive(Debug, Default)]
pub struct AiBroll {
    view: AiBrollView,
    project: Option<String>,
    reading: Option<mpsc::Receiver<Read>>,
    read_at: Option<Instant>,
    source_request: Option<(String, u64, u64)>,
}

impl AiBroll {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &AiBrollView {
        &self.view
    }

    pub fn busy(&self) -> bool {
        self.reading.is_some()
    }

    /// Takes a finished read, reads again now and then. Returns whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview) -> bool {
        let before = self.view.clone();
        let project = preview.project_target().cloned();
        let key = project.as_ref().map(|target| target.uri().to_string());
        if key != self.project {
            *self = Self { project: key, ..Self::default() };
        }
        if let Some(reading) = &self.reading {
            if let Ok(result) = reading.try_recv() {
                match result {
                    Ok(parts) => {
                        self.view.parts = parts;
                        self.view.message.clear();
                    }
                    Err(error) => self.view.message = error,
                }
                self.reading = None;
            }
        }
        if let Some(project) = project.filter(|_| self.reading.is_none() && self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY)) {
            self.read_at = Some(Instant::now());
            let (send, read) = mpsc::channel();
            if std::thread::Builder::new().name("qnc-ai-broll-read".into()).spawn(move || _ = send.send(read_parts(&project))).is_ok() {
                self.reading = Some(read);
            }
        }
        self.view != before
    }

    /// Its own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>) -> Option<bool> {
        if action_id != OPEN {
            return None;
        }
        let mut fields = target?.rsplitn(3, '|');
        let (to, from, clip) = (fields.next()?.parse::<u64>().ok()?, fields.next()?.parse::<u64>().ok()?, fields.next()?);
        let known = self.view.parts.iter().any(|part| part.clip_id == clip && part.from == from && part.to == to);
        if known {
            self.source_request = Some((clip.to_string(), from, to));
        }
        Some(known)
    }

    /// A clip and IN/OUT a chosen part opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }
}

/// The parts of the project behind `target` for the model of this host.
fn read_parts(target: &ProjectDbTarget) -> Read {
    let engine = qnc_ai_catalog::installed_shot_content()?;
    let model = engine.model_name();
    let list = qnc_shot_labels::Labels::read(&qnc_shot_labels::installed_path()?)?;
    let Some(set) = qnc_shot_content_db::read_labels(target, &model)? else {
        return Ok(Vec::new());
    };
    let content = qnc_shot_content_db::read_all(target, &model)?;
    // The transcripts of the clips with parts (A1), where they exist.
    let transcripts: std::collections::HashMap<&str, Vec<qnc_transcript_db::ClipTranscript>> = content
        .iter()
        .filter(|clip| !clip.parts.is_empty())
        .filter_map(|clip| Some((clip.clip_id.as_str(), qnc_transcript_db::read_clip(target, &clip.clip_id).ok()?)))
        .collect();
    let speech = |clip: &str, from: u64, to: u64| spoken_share(transcripts.get(clip)?, from, to);
    Ok(sort_parts(&content, &set, &list, &speech))
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_shot_content_db::{ClipContent, LabelSet, LabelVector, PartContent};

    fn list() -> qnc_shot_labels::Labels {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("labels.json");
        std::fs::write(&path, r#"{"version":1,"least":0.1,"labels":[
            {"code":"sea","text":"shot.label.sea","prompt":"a photo of the sea"},
            {"code":"person","text":"shot.label.person","prompt":"a photo of a person"}]}"#)
        .unwrap();
        qnc_shot_labels::Labels::read(&path).unwrap()
    }

    fn part(from: u64, to: u64, vector: Vec<f32>) -> PartContent {
        PartContent { from, to, frames: vec![from], vector }
    }

    #[test]
    fn parts_are_grouped_by_the_label_they_fit_best_in_the_lists_order_the_longest_first() {
        let set = LabelSet {
            model: "m".into(),
            scale: 10.0,
            bias: 0.0,
            labels: vec![LabelVector { code: "person".into(), prompt: "p".into(), vector: vec![0.0, 1.0] }, LabelVector { code: "sea".into(), prompt: "s".into(), vector: vec![1.0, 0.0] }],
        };
        let content = vec![
            ClipContent { clip_id: "a".into(), model: "m".into(), parts: vec![part(0, 100, vec![0.0, 1.0]), part(200, 500, vec![1.0, 0.0])], error: String::new(), took_ms: 0 },
            ClipContent { clip_id: "b".into(), model: "m".into(), parts: vec![part(0, 50, vec![0.9, 0.1])], error: String::new(), took_ms: 0 },
        ];
        let parts = sort_parts(&content, &set, &list(), &|_, _, _| None);
        let shown: Vec<(&str, u64, &str)> = parts.iter().map(|p| (p.clip_id.as_str(), p.from, p.label.as_str())).collect();
        assert_eq!(shown, [("a", 200, "sea"), ("b", 0, "sea"), ("a", 0, "person")], "sea comes first in the list; the longer sea part first");
        assert!(parts[0].share > 0.99);
        assert_eq!(parts[0].text, "shot.label.sea");
    }

    #[test]
    fn a_part_spoken_on_a1_is_a_statement_and_one_without_a_transcript_is_judged_by_its_pictures() {
        use qnc_transcript_db::{ClipTranscript, Line, Word};
        let word = |from, to| Word { from, to, text: "w".into() };
        let transcript = ClipTranscript {
            clip_id: "a".into(),
            from_frame: 0,
            to_frame: 1000,
            language: "hr".into(),
            engine: "e".into(),
            lines: vec![Line { from: 0, to: 300, text: "w w".into(), words: vec![word(0, 100), word(50, 200)] }],
        };
        assert_eq!(spoken_share(&[transcript.clone()], 0, 400), Some(0.5), "words overlapping are counted once");
        assert_eq!(spoken_share(&[transcript.clone()], 500, 900), Some(0.0));
        assert_eq!(spoken_share(&[transcript], 900, 1200), None, "the transcript does not cover the part");
        let set = LabelSet { model: "m".into(), scale: 10.0, bias: 0.0, labels: vec![LabelVector { code: "sea".into(), prompt: "s".into(), vector: vec![1.0, 0.0] }] };
        let content = vec![ClipContent { clip_id: "a".into(), model: "m".into(), parts: vec![part(0, 400, vec![1.0, 0.0]), part(500, 900, vec![1.0, 0.0])], error: String::new(), took_ms: 0 }];
        let speech = |_: &str, from: u64, to: u64| Some(if from == 0 { 0.5 } else { 0.0 }).filter(|_| to > from);
        let parts = sort_parts(&content, &set, &list(), &speech);
        // The pictures are no statement: spoken or not both stay, the spoken one marked (people
        // and A1 may be both).
        assert_eq!(parts.iter().map(|p| (p.from, p.spoken)).collect::<Vec<_>>(), [(0, true), (500, false)]);
        let talk = qnc_shot_labels::Labels::read(&{
            let dir = tempfile::tempdir().unwrap().keep();
            let path = dir.join("labels.json");
            std::fs::write(&path, r#"{"version":1,"least":0.1,"statement_least":0.05,"labels":[
                {"code":"talk","text":"t","prompt":"talk","statement":true},
                {"code":"sea","text":"shot.label.sea","prompt":"s"}]}"#)
            .unwrap();
            path
        })
        .unwrap();
        let set = LabelSet {
            labels: vec![LabelVector { code: "talk".into(), prompt: "talk".into(), vector: vec![1.0, 0.0] }, LabelVector { code: "sea".into(), prompt: "s".into(), vector: vec![1.0, 0.0] }],
            ..set
        };
        let parts = sort_parts(&content, &set, &talk, &speech);
        assert_eq!(parts.iter().map(|p| p.from).collect::<Vec<_>>(), [500], "looks like a statement and spoken: a statement; not spoken: B-roll");
        let parts = sort_parts(&content, &set, &talk, &|_, _, _| None);
        assert!(parts.is_empty(), "without a transcript the pictures decide");
    }

    #[test]
    fn a_click_opens_only_a_part_the_tab_shows() {
        let mut broll = AiBroll::new();
        broll.view.parts = vec![AiPart { clip_id: "clip|x".into(), from: 10, to: 90, label: "sea".into(), text: "shot.label.sea".into(), share: 1.0, spoken: false }];
        assert_eq!(broll.act(OPEN, Some("clip|x|10|90")), Some(true));
        assert_eq!(broll.take_source_request(), Some(("clip|x".to_string(), 10, 90)));
        assert_eq!(broll.act(OPEN, Some("clip|x|10|91")), Some(false));
        assert_eq!(broll.act("other", Some("clip|x|10|90")), None);
    }
}

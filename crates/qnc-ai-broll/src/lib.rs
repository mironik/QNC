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

/// What a statement is (user 2026-11-10: "A1 as a help, where it exists"): the one rule of the
/// transcripts.
pub use qnc_transcript_db::{spoken_share, SPOKEN};


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
    /// How the shot looks (steady, pan, moving, soft, still, shot) and its poster (a project URI).
    pub kind: String,
    pub poster: String,
    /// Its vector (kept with a correction) and how the examples of this computer gave it its
    /// group (`None`: the labels' texts did).
    pub vector: Vec<f32>,
    pub taught: Option<Taught>,
    /// The terms of the vocabulary it fits best (dictionary keys), best first (user 2026-11-10:
    /// what the model saw, not only the group).
    pub terms: Vec<String>,
    /// The caption a model wrote of it (user 2026-11-11: Florence-2), when there is one; with
    /// its pictures it fits the words of an OFF.
    pub caption: Option<String>,
}

/// The chosen card's button asks for another group (target: `clip|from|to`); an answer gives
/// it (target: the group's code, `NOT_BROLL`: no B-roll) or withdraws the question.
pub use qnc_pool_tabs::REGROUP_AI_BROLL as REGROUP;
pub const GROUP: &str = "ai_broll_group";
pub const REGROUP_CANCEL: &str = "ai_broll_regroup_cancel";
pub use qnc_shot_examples::{Taught, NOT_BROLL};
pub use qnc_broll_suggest::{ask_of, ask_of_text, Ask};

/// What the tab shows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AiBrollView {
    /// In the order shown.
    pub parts: Vec<AiPart>,
    /// The shots' posters read so far, by URI.
    pub posters: std::collections::HashMap<String, std::sync::Arc<qnc_image_assets::RgbaImage>>,
    pub message: String,
    /// The suggestion for the slot chosen in the story: its length and the words A1 says there.
    pub suggestion: Option<qnc_broll_suggest::Suggestion>,
}

/// The words of a shot's kind.
fn kind_text(kind: &str) -> &'static str {
    match kind {
        "steady" => qnc_ui_text::text("shots.camera.steady"),
        "pan" => qnc_ui_text::text("shots.camera.pan"),
        "moving" => qnc_ui_text::text("shots.look.moving"),
        "soft" => qnc_ui_text::text("shots.look.soft"),
        "still" => qnc_ui_text::text("shots.look.still"),
        _ => "",
    }
}

impl AiBrollView {
    /// The parts of the clips of the pool as cards: "Label · clip", the part's duration, the
    /// clip's poster. A part of a clip the pool has not is left out.
    /// With a suggestion for the chosen slot (user 2026-11-10: B-roll for the words of an OFF) the
    /// parts long enough for the slot come first, the most like its words first, numbered; the
    /// shorter ones after them, in the same order.
    pub fn cards<'a>(&'a self, clips: &'a [ClipRow]) -> Vec<TabCard<'a>> {
        let cards: Vec<(qnc_broll_suggest::Candidate<'a>, TabCard<'a>)> = self
            .parts
            .iter()
            .filter_map(|part| {
                let clip = clips.iter().find(|clip| clip.clip_id == part.clip_id)?;
                let label = qnc_ui_text::host().ok().and_then(|texts| texts.get(&part.text).ok()).unwrap_or(&part.label);
                let per_frame = if clip.duration_frames > 0 { clip.duration_seconds / clip.duration_frames as f64 } else { 0.0 };
                let kind = kind_text(&part.kind);
                // What the model saw (the vocabulary's best terms) says more than the group.
                let words = |key: &str| qnc_ui_text::host().ok().and_then(|texts| texts.get(key).ok()).unwrap_or(key).to_string();
                let label = if !part.terms.is_empty() { part.terms.iter().take(2).map(|term| words(term)).collect::<Vec<_>>().join(", ") } else if kind.is_empty() { label.to_string() } else { format!("{label} · {kind}") };
                let seconds = (part.to - part.from) as f64 * per_frame;
                Some((
                    qnc_broll_suggest::Candidate { seconds, vector: part.vector.as_slice(), caption: part.caption.as_deref() },
                    TabCard {
                        id: format!("{}|{}|{}", part.clip_id, part.from, part.to),
                        title: qnc_ui_text::fill(if part.spoken { "ai_broll.card_spoken" } else { "ai_broll.card" }, &[("label", &label), ("clip", &clip.name)]).into(),
                        seconds,
                        clip: Some(clip),
                        icon: qnc_clip_row::PoolIcon::None,
                        poster: self.posters.get(&part.poster).map(|image| (part.poster.as_str(), image.as_ref())),
                    },
                ))
            })
            .collect();
        let Some(suggestion) = &self.suggestion else { return cards.into_iter().map(|(_, card)| card).collect() };
        let parts: Vec<qnc_broll_suggest::Candidate<'_>> = cards.iter().map(|(part, _)| *part).collect();
        match suggestion.slots.as_slice() {
            // One sentence of the OFF: the parts long enough first, those fitting its words best
            // first (their pictures and their captions).
            [slot] => {
                let scores = qnc_broll_suggest::scores(suggestion, slot, &parts);
                let mut cards: Vec<(f32, qnc_broll_suggest::Candidate<'_>, TabCard<'a>)> = scores.into_iter().zip(cards).map(|(score, (part, card))| (score, part, card)).collect();
                cards.sort_by(|(a_score, a, _), (b_score, b, _)| (b.seconds >= slot.seconds).cmp(&(a.seconds >= slot.seconds)).then(b_score.total_cmp(a_score)));
                let long = cards.iter().filter(|(_, part, _)| part.seconds >= slot.seconds).count();
                for (n, (_, _, card)) in cards.iter_mut().take(long).enumerate() {
                    card.title = format!("{}. {}", n + 1, card.title);
                }
                cards.into_iter().map(|(_, _, card)| card).collect()
            }
            // The whole OFF: the series, a cover a slot in the story's order, then the rest.
            slots => {
                let picks = qnc_broll_suggest::series(suggestion, &parts);
                let mut cards: Vec<Option<TabCard<'a>>> = cards.into_iter().map(|(_, card)| Some(card)).collect();
                let mut shown = Vec::with_capacity(cards.len());
                for (n, pick) in picks.iter().enumerate() {
                    if let Some(mut card) = pick.and_then(|index| cards[index].take()) {
                        card.title = format!("{}/{} · {}", n + 1, slots.len(), card.title);
                        shown.push(card);
                    }
                }
                shown.extend(cards.into_iter().flatten());
                shown
            }
        }
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
    (project, examples, described): (&str, &[qnc_shot_examples::Example], &[qnc_shot_examples::Example]),
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
            // A statement kept whole by the look (`statement`) is never B-roll.
            if part.kind == "statement" || (list.is_statement(&logits) && spoken != Some(false)) {
                return None;
            }
            let (mut label, mut share) = list.shares(&logits).into_iter().next()?;
            // The corrections of this computer come before the labels' texts (user 2026-11-10:
            // the agent learns from what the newsroom corrects): this shot's own, else the
            // group of a corrected shot it looks like; a shot that is no B-roll is left out.
            let key = qnc_shot_examples::key(project, &clip.clip_id, part.from, part.to);
            let taught = match qnc_shot_examples::group_with(examples, described, &key, &part.vector) {
                Some((NOT_BROLL, _)) => return None,
                Some((code, taught)) if list.text_of(code).is_some() => {
                    (label, share) = (code.to_string(), 1.0);
                    Some(taught)
                }
                _ => None,
            };
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
                    kind: part.kind.clone(),
                    poster: part.poster.clone(),
                    vector: part.vector.clone(),
                    taught,
                    terms: Vec::new(),
                    caption: None,
                },
            ))
        })
        .collect();
    parts.sort_by(|(a, x), (b, y)| a.cmp(b).then((y.to - y.from).cmp(&(x.to - x.from))).then(x.clip_id.cmp(&y.clip_id)).then(x.from.cmp(&y.from)));
    parts.into_iter().map(|(_, part)| part).collect()
}

/// The parts in order, the posters newly read and the model of the parts' vectors.
type Read = Result<(Vec<AiPart>, Vec<(String, std::sync::Arc<qnc_image_assets::RgbaImage>)>, String, String), String>;

/// The AI B-roll of the project on the preview.
#[derive(Debug, Default)]
pub struct AiBroll {
    view: AiBrollView,
    project: Option<String>,
    reading: Option<mpsc::Receiver<Read>>,
    read_at: Option<Instant>,
    source_request: Option<(String, u64, u64)>,
    /// The model of the parts read, the card a group is being chosen for and a correction
    /// being kept.
    model: String,
    /// The project's language of speech (`ai.language`), for the words of an OFF.
    language: String,
    regroup: Option<String>,
    keeping: Option<mpsc::Receiver<Result<(), String>>>,
    /// B-roll suggested for the words of the slot chosen in the story.
    suggest: qnc_broll_suggest::Suggest,
}

impl AiBroll {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn view(&self) -> &AiBrollView {
        &self.view
    }

    /// Follows the slot chosen in the story (`qnc_broll_suggest::ask_of`): its suggestion orders
    /// the cards once worked out. Returns whether the view changed.
    pub fn follow_slot(&mut self, ask: Option<qnc_broll_suggest::Ask>, preview: &SourcePreview) -> bool {
        // The parts' captions fit the words with their pictures (user 2026-11-11).
        let captions: Vec<&str> = self.view.parts.iter().filter_map(|part| part.caption.as_deref()).collect();
        let changed = self.suggest.follow(ask, preview.project_target(), &captions, &self.language);
        if changed {
            self.view.suggestion = self.suggest.shown().cloned();
        }
        changed
    }

    pub fn busy(&self) -> bool {
        self.reading.is_some() || self.suggest.working()
    }

    /// Takes a finished read, reads again now and then. Returns whether the view changed.
    pub fn poll(&mut self, preview: &SourcePreview) -> bool {
        let before = self.view.clone();
        let project = preview.project_target().cloned();
        let key = project.as_ref().map(|target| target.uri().to_string());
        if key != self.project {
            if self.regroup.is_some() {
                qnc_notice::withdraw();
            }
            *self = Self { project: key, ..Self::default() };
        }
        if let Some(reading) = &self.reading {
            if let Ok(result) = reading.try_recv() {
                match result {
                    Ok((parts, posters, model, language)) => {
                        self.language = language;
                        self.view.posters.extend(posters);
                        self.view.parts = parts;
                        self.view.message.clear();
                        self.model = model;
                    }
                    Err(error) => self.view.message = error,
                }
                self.reading = None;
            }
        }
        // A correction kept: the parts are read again (shots like it follow it).
        if let Some(Ok(kept)) = self.keeping.as_ref().map(mpsc::Receiver::try_recv) {
            self.keeping = None;
            self.read_at = None;
            if let Err(error) = kept {
                qnc_notice::raise(error);
            }
        }
        if let Some(project) = project.filter(|_| self.reading.is_none() && self.keeping.is_none() && self.read_at.is_none_or(|at| at.elapsed() >= READ_EVERY)) {
            self.read_at = Some(Instant::now());
            let (send, read) = mpsc::channel();
            let (settings, known) = (preview.project_settings(), self.view.posters.keys().cloned().collect::<std::collections::HashSet<_>>());
            if std::thread::Builder::new().name("qnc-ai-broll-read".into()).spawn(move || _ = send.send(read_parts(&project, settings, &known))).is_ok() {
                self.reading = Some(read);
            }
        }
        self.view != before
    }

    /// Its own actions; `None` for any other.
    pub fn act(&mut self, action_id: &str, target: Option<&str>) -> Option<bool> {
        match action_id {
            OPEN => {
                let Some(part) = target.and_then(|id| self.part_of(id)).cloned() else { return Some(false) };
                self.source_request = Some((part.clip_id, part.from, part.to));
                Some(true)
            }
            REGROUP => {
                let id = target?.to_string();
                let Some(part) = self.part_of(&id) else { return Some(false) };
                qnc_notice::ask(regroup_choice(part, &self.groups_of()));
                self.regroup = Some(id);
                Some(true)
            }
            GROUP => Some(self.keep(target?)),
            REGROUP_CANCEL => {
                self.regroup = None;
                qnc_notice::withdraw();
                Some(true)
            }
            _ => None,
        }
    }

    /// The part of a card's id (`clip|from|to`) the tab shows.
    fn part_of(&self, id: &str) -> Option<&AiPart> {
        let mut fields = id.rsplitn(3, '|');
        let (to, from, clip) = (fields.next()?.parse::<u64>().ok()?, fields.next()?.parse::<u64>().ok()?, fields.next()?);
        self.view.parts.iter().find(|part| part.clip_id == clip && part.from == from && part.to == to)
    }

    /// The groups a shot can be given: the list's labels that are no statement, then no B-roll.
    fn groups_of(&self) -> Vec<(String, String)> {
        let list = qnc_shot_labels::installed_path().and_then(|path| qnc_shot_labels::Labels::read(&path));
        let mut groups: Vec<(String, String)> = list.map_or_else(|_| Vec::new(), |list| list.labels.into_iter().filter(|label| !label.statement).map(|label| (label.code, label.text)).collect());
        groups.push((NOT_BROLL.into(), "ai_broll.not_broll".into()));
        groups
    }

    /// Gives the shot asked about the group `code`: shown at once, kept in the examples of this
    /// computer in the background, then the parts are read again.
    fn keep(&mut self, code: &str) -> bool {
        qnc_notice::withdraw();
        let Some(id) = self.regroup.take() else { return false };
        let (Some(part), Some(project)) = (self.part_of(&id).cloned(), self.project.clone()) else { return false };
        let Some(root) = qnc_dev_diagnostics::resolve_qnc_root() else { return false };
        let example = qnc_shot_examples::Example {
            model: self.model.clone(),
            key: qnc_shot_examples::key(&project, &part.clip_id, part.from, part.to),
            label: code.to_string(),
            vector: part.vector.clone(),
        };
        if code == NOT_BROLL {
            self.view.parts.retain(|shown| !(shown.clip_id == part.clip_id && shown.from == part.from && shown.to == part.to));
        } else if let Some((text, shown)) = self.groups_of().into_iter().find(|(group, _)| group == code).map(|(_, text)| text).zip(self.view.parts.iter_mut().find(|shown| shown.clip_id == part.clip_id && shown.from == part.from && shown.to == part.to)) {
            (shown.label, shown.text, shown.share, shown.taught) = (code.to_string(), text, 1.0, Some(Taught::Corrected));
        }
        let (send, kept) = mpsc::channel();
        let spawned = std::thread::Builder::new().name("qnc-ai-broll-keep".into()).spawn(move || _ = send.send(qnc_shot_examples::save(&root, &example))).is_ok();
        if spawned {
            self.keeping = Some(kept);
        }
        spawned
    }

    /// A clip and IN/OUT a chosen part opens on the source timeline, once.
    pub fn take_source_request(&mut self) -> Option<(String, u64, u64)> {
        self.source_request.take()
    }
}

/// The question of another group for `part`: every group (its own one proposed) and Cancel.
fn regroup_choice(part: &AiPart, groups: &[(String, String)]) -> qnc_notice::Choice {
    let words = |text: &str| qnc_ui_text::host().ok().and_then(|texts| texts.get(text).ok()).unwrap_or(text).to_string();
    let mut options: Vec<qnc_notice::ChoiceOption> =
        groups.iter().map(|(code, text)| qnc_notice::ChoiceOption::new(GROUP, words(text), *code == part.label).with_target(code.clone())).collect();
    options.push(qnc_notice::ChoiceOption::new(REGROUP_CANCEL, qnc_ui_text::text("common.cancel"), false));
    qnc_notice::Choice {
        title: qnc_ui_text::text("ai_broll.regroup_title").into(),
        message: qnc_ui_text::fill("ai_broll.regroup_message", &[("group", &words(&part.text))]).into(),
        options,
        ..qnc_notice::Choice::default()
    }
}

/// The parts of the project behind `target` for the model of this host, and the posters of its
/// shots not in `known`, read from the project folder.
fn read_parts(target: &ProjectDbTarget, settings: Option<(qnc_work_settings::SettingsReader, qnc_work_settings::WorkSettings)>, known: &std::collections::HashSet<String>) -> Read {
    let (mut parts, model) = read_sorted(target)?;
    // A caption (English) fits the words of an OFF only when they are English: measured
    // 2026-11-11, SigLIP 2 finds the English captions of the shots for English sentences (5 of
    // 7) but not for the same sentences in Croatian; translated into English by this computer's
    // translator they find them as well (5 of 7). So the captions count in the suggestion for a
    // project whose speech is English or translated into English.
    let language = settings.as_ref().and_then(|(_, settings)| settings.speech_language().ok()).unwrap_or_default();
    let english = language == "en" || qnc_ai_catalog::installed_translator().is_ok_and(|translator| translator.translates(&language));
    if !english {
        parts.iter_mut().for_each(|part| part.caption = None);
    }
    let folder = settings.and_then(|(reader, settings)| Some((reader.local_workspace_dir(&settings).ok()??, settings.output_root_uri)));
    let posters = folder.map_or_else(Vec::new, |(dir, root)| {
        parts
            .iter()
            .filter(|part| !part.poster.is_empty() && !known.contains(&part.poster))
            .filter_map(|part| {
                let relative = part.poster.strip_prefix(root.trim_end_matches('/'))?.strip_prefix('/')?;
                let file = dir.join(relative);
                // A poster inside the project only.
                (file.starts_with(&dir) && !relative.contains("..")).then_some(())?;
                let image = qnc_image_assets::decode_thumbnail(&std::fs::read(file).ok()?).ok()?;
                Some((part.poster.clone(), std::sync::Arc::new(image)))
            })
            .collect()
    });
    Ok((parts, posters, model, language))
}

/// The captions this computer's captioner wrote of the looked-at parts of the project, by clip
/// and frames; none while this host has no captioner.
fn captions_of(root: &std::path::Path, project: &str, content: &[qnc_shot_content_db::ClipContent]) -> std::collections::HashMap<(String, u64, u64), String> {
    let Ok(captioner) = qnc_ai_catalog::installed_shot_captioner() else { return Default::default() };
    let captioner = captioner.model_name();
    let mut captions = std::collections::HashMap::new();
    for clip in content.iter().filter(|clip| !clip.parts.is_empty()) {
        let Ok(read) = qnc_shot_captions::read_clip(root, &captioner, project, &clip.clip_id) else { continue };
        captions.extend(read.into_iter().map(|caption| ((clip.clip_id.clone(), caption.from, caption.to), caption.text)));
    }
    captions
}

/// Examples of the groups the captions give the looked-at shots: a shot whose caption a rule of
/// the list names gets that group with its vector (`model`: of the vectors).
fn described_examples(captions: &std::collections::HashMap<(String, u64, u64), String>, project: &str, model: &str, content: &[qnc_shot_content_db::ClipContent], list: &qnc_shot_labels::Labels) -> Vec<qnc_shot_examples::Example> {
    content
        .iter()
        .flat_map(|clip| clip.parts.iter().map(move |part| (clip, part)))
        .filter_map(|(clip, part)| {
            let group = list.group_of_caption(captions.get(&(clip.clip_id.clone(), part.from, part.to))?)?;
            Some(qnc_shot_examples::Example { model: model.into(), key: qnc_shot_examples::key(project, &clip.clip_id, part.from, part.to), label: group.into(), vector: part.vector.clone() })
        })
        .collect()
}

/// The parts of the project behind `target` for the model of this host, in the tab's order.
/// With the model of their vectors.
fn read_sorted(target: &ProjectDbTarget) -> Result<(Vec<AiPart>, String), String> {
    let engine = qnc_ai_catalog::installed_shot_content()?;
    let model = engine.model_name();
    let list = qnc_shot_labels::Labels::read(&qnc_shot_labels::installed_path()?)?;
    let Some(set) = qnc_shot_content_db::read_labels(target, &model)? else {
        return Ok((Vec::new(), model));
    };
    let content = qnc_shot_content_db::read_all(target, &model)?;
    // The transcripts of the clips with parts (A1), where they exist.
    let transcripts: std::collections::HashMap<&str, Vec<qnc_transcript_db::ClipTranscript>> = content
        .iter()
        .filter(|clip| !clip.parts.is_empty())
        .filter_map(|clip| Some((clip.clip_id.as_str(), qnc_transcript_db::read_clip(target, &clip.clip_id).ok()?)))
        .collect();
    let speech = |clip: &str, from: u64, to: u64| spoken_share(transcripts.get(clip)?, from, to);
    let root = qnc_dev_diagnostics::resolve_qnc_root();
    // The corrections of this computer, outside every project (user 2026-11-10).
    let examples = root.as_deref().map_or_else(|| Ok(Vec::new()), |root| qnc_shot_examples::read(root, &model))?;
    // The captions of this computer's captioner (user 2026-11-11: Florence-2) and the groups
    // they give the shots, the corrector and trainer of SigLIP: made here from the captions and
    // the list's rules, never kept, so a changed rule counts at once.
    let captions = root.as_deref().map_or_else(Default::default, |root| captions_of(root, target.uri(), &content));
    let described = described_examples(&captions, target.uri(), &model, &content, &list);
    let mut parts = sort_parts(&content, &set, &list, &speech, (target.uri(), &examples, &described));
    for part in &mut parts {
        part.caption = captions.get(&(part.clip_id.clone(), part.from, part.to)).cloned();
    }
    // The terms of the vocabulary of this computer, where its vectors are kept for this model.
    let vocabulary = root.as_deref().and_then(|root| qnc_shot_vocabulary::read(root, &model).ok().flatten()).zip(qnc_shot_vocabulary::installed().ok());
    if let Some((set, list)) = vocabulary {
        for part in &mut parts {
            part.terms = qnc_shot_vocabulary::best_terms(&set, &list, &part.vector, 2).into_iter().filter_map(|(code, _)| list.text_of(&code).map(str::to_string)).collect();
        }
    }
    Ok((parts, model))
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
        PartContent { from, to, kind: "steady".into(), frames: vec![from], vector, poster: String::new(), fine: false }
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
        let parts = sort_parts(&content, &set, &list(), &|_, _, _| None, ("p", &[], &[]));
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
        let parts = sort_parts(&content, &set, &list(), &speech, ("p", &[], &[]));
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
        let parts = sort_parts(&content, &set, &talk, &speech, ("p", &[], &[]));
        assert_eq!(parts.iter().map(|p| p.from).collect::<Vec<_>>(), [500], "looks like a statement and spoken: a statement; not spoken: B-roll");
        let parts = sort_parts(&content, &set, &talk, &|_, _, _| None, ("p", &[], &[]));
        assert!(parts.is_empty(), "without a transcript the pictures decide");
    }

    #[test]
    fn a_click_opens_only_a_part_the_tab_shows() {
        let mut broll = AiBroll::new();
        broll.view.parts = vec![AiPart { clip_id: "clip|x".into(), from: 10, to: 90, label: "sea".into(), text: "shot.label.sea".into(), share: 1.0, spoken: false, kind: "steady".into(), poster: String::new(), vector: vec![], taught: None, terms: Vec::new(), caption: None }];
        assert_eq!(broll.act(OPEN, Some("clip|x|10|90")), Some(true));
        assert_eq!(broll.take_source_request(), Some(("clip|x".to_string(), 10, 90)));
        assert_eq!(broll.act(OPEN, Some("clip|x|10|91")), Some(false));
        assert_eq!(broll.act("other", Some("clip|x|10|90")), None);
    }

    #[test]
    fn a_slots_suggestion_puts_the_parts_long_enough_and_most_like_its_words_first() {
        let clip = ClipRow { clip_id: "a".into(), name: "A".into(), duration_seconds: 10.0, duration_frames: 500, ..ClipRow::default() };
        let shot = |from, to, vector: Vec<f32>| AiPart { clip_id: "a".into(), from, to, label: "sea".into(), text: "sea".into(), share: 1.0, spoken: false, kind: String::new(), poster: String::new(), vector, taught: None, terms: Vec::new(), caption: None };
        let mut view = AiBrollView { parts: vec![shot(0, 100, vec![1.0, 0.0]), shot(100, 300, vec![0.0, 1.0]), shot(300, 500, vec![0.8, 0.6])], ..AiBrollView::default() };
        let ids = |view: &AiBrollView| view.cards(std::slice::from_ref(&clip)).iter().map(|card| card.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&view), ["a|0|100", "a|100|300", "a|300|500"]);
        let slot = |id: &str, seconds, vector: Vec<f32>| qnc_broll_suggest::SlotSuggestion { slot_id: id.into(), seconds, text: "more".into(), vector };
        view.suggestion = Some(qnc_broll_suggest::Suggestion { key: "s".into(), slots: vec![slot("s", 3.0, vec![1.0, 0.0])], captions: Default::default() });
        // 2 s is shorter than the slot: last; of the 4 s parts the one most like the words first.
        assert_eq!(ids(&view), ["a|300|500", "a|100|300", "a|0|100"]);
        let cards = view.cards(std::slice::from_ref(&clip));
        assert!(cards[0].title.starts_with("1. ") && cards[1].title.starts_with("2. ") && !cards[2].title.starts_with("3. "));
        // The whole OFF: a cover a slot in order, none twice, then the rest.
        view.suggestion = Some(qnc_broll_suggest::Suggestion { key: "off:x".into(), slots: vec![slot("s1", 3.0, vec![0.0, 1.0]), slot("s2", 3.0, vec![0.0, 1.0])], captions: Default::default() });
        let cards = view.cards(std::slice::from_ref(&clip));
        assert_eq!(cards.iter().map(|card| card.id.as_str()).collect::<Vec<_>>(), ["a|100|300", "a|300|500", "a|0|100"]);
        assert!(cards[0].title.starts_with("1/2 · ") && cards[1].title.starts_with("2/2 · "));
    }

    #[test]
    fn a_corrected_shot_and_shots_like_it_take_the_group_the_computer_keeps() {
        let set = LabelSet {
            model: "m".into(),
            scale: 10.0,
            bias: 0.0,
            labels: vec![LabelVector { code: "person".into(), prompt: "p".into(), vector: vec![0.0, 1.0] }, LabelVector { code: "sea".into(), prompt: "s".into(), vector: vec![1.0, 0.0] }],
        };
        let content = vec![ClipContent {
            clip_id: "a".into(),
            model: "m".into(),
            parts: vec![part(0, 100, vec![1.0, 0.0]), part(200, 300, vec![0.99, 0.141]), part(400, 500, vec![0.0, 1.0]), part(600, 700, vec![0.6, 0.8])],
            error: String::new(),
            took_ms: 0,
        }];
        let example = |key: &str, label: &str, vector: Vec<f32>| qnc_shot_examples::Example { model: "m".into(), key: key.into(), label: label.into(), vector };
        // The pictures say sea; the user said person, and that a shot of the sea far from it is
        // no B-roll.
        let examples = [example(&qnc_shot_examples::key("p", "a", 0, 100), "person", vec![1.0, 0.0]), example("other project|z|0|9", NOT_BROLL, vec![0.6, 0.8])];
        let parts = sort_parts(&content, &set, &list(), &|_, _, _| None, ("p", &examples, &[]));
        let shown: Vec<(u64, &str, Option<Taught>)> = parts.iter().map(|p| (p.from, p.label.as_str(), p.taught)).collect();
        assert_eq!(
            shown,
            [(0, "person", Some(Taught::Corrected)), (200, "person", Some(Taught::LikeCorrected)), (400, "person", None)],
            "its own correction, one like it follows it, one like a shot that is no B-roll is left out, the rest as the labels say"
        );
        // A caption names the first shot sea and the
        // third sea; the user's correction still comes first for the first and its like.
        let described = [example(&qnc_shot_examples::key("p", "a", 0, 100), "sea", vec![1.0, 0.0]), example(&qnc_shot_examples::key("p", "a", 400, 500), "sea", vec![0.0, 1.0])];
        let parts = sort_parts(&content, &set, &list(), &|_, _, _| None, ("p", &examples, &described));
        let shown: Vec<(u64, &str, Option<Taught>)> = parts.iter().map(|p| (p.from, p.label.as_str(), p.taught)).collect();
        assert_eq!(shown, [(400, "sea", Some(Taught::Described)), (0, "person", Some(Taught::Corrected)), (200, "person", Some(Taught::LikeCorrected))], "the groups in the order of the list");
    }
}

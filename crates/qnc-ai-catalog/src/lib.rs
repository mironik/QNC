//! The AI catalog of this host (`catalogs/ai/catalog.json`): which program, model and backend
//! do each AI job here (user 2026-10-26). It is host configuration like the decoder catalog,
//! never a project setting: a laptop with an Intel card and one with an NVIDIA card run the
//! same project with their own engines. One engine per job is selected; one that cannot run
//! (its model missing, an unknown backend) is an error, nothing falls back to another.

use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const MODULE_ID: &str = "qnc.module.ai-catalog";
/// The catalog beside the application, below its folder or one of its parents.
pub const CATALOG: &str = "catalogs/ai/catalog.json";
/// The backends an engine may name.
pub const BACKENDS: [&str; 2] = ["vulkan", "cpu"];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: String,
    transcribe: Job,
    /// The fine times of the letters of a known text (user 2026-11-05: cutting by letters in
    /// the script); a host without it has none.
    #[serde(default)]
    letters: Option<LettersJob>,
    /// The in-between pictures of a smooth cut (user 2026-11-08); a host without it has none.
    #[serde(default)]
    smooth: Option<LettersJob>,
    /// What the pictures of a clip show (user 2026-11-10: sorting the B-roll by content); a host
    /// without it has none.
    #[serde(default)]
    shot_content: Option<ContentJob>,
    /// A sentence of what a shot shows (user 2026-11-10: Florence-2 as the second AI engine); a
    /// host without it has none.
    #[serde(default)]
    shot_caption: Option<LettersJob>,
    /// A translation of the words of a story into English (user 2026-11-11: "the OFF into English",
    /// so that SigLIP 2 finds the pictures and the captions of its words); a host without it has
    /// none.
    #[serde(default)]
    translate: Option<TranslateJob>,
    /// The store of the captions and the corrections an AI server keeps for every QNC that uses it
    /// (docs/102 step 7b); a host that is no AI server needs none.
    #[serde(default)]
    #[allow(dead_code)]
    store: Option<StoreJob>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentJob {
    program: String,
    model: String,
    /// Pictures looked at per usable part of a clip: 1, 2 or 3, after the strength of this host.
    pictures: u8,
}

/// The engine that sees what the pictures of a clip show, on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShotContentEngine {
    /// The program's name beside the application.
    pub program: String,
    /// The model's folder.
    pub model: PathBuf,
    /// Pictures per usable part (1-3).
    pub pictures: u8,
}

impl ShotContentEngine {
    /// The model's name the kept vectors carry: the last folder of its path (vectors of
    /// different models are not compared).
    pub fn model_name(&self) -> String {
        self.model.file_name().map_or_else(|| self.model.display().to_string(), |name| name.to_string_lossy().into_owned())
    }
}

/// The shot content engine of this host's catalog; its model must be there.
pub fn installed_shot_content() -> Result<ShotContentEngine, String> {
    shot_content(&installed_path()?)
}

/// The shot content engine of the catalog at `path`.
pub fn shot_content(path: &Path) -> Result<ShotContentEngine, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let job = file.shot_content.ok_or("the AI catalog of this host has no shot content engine")?;
    if !(1..=3).contains(&job.pictures) {
        return Err("shot content: pictures per part is 1, 2 or 3".into());
    }
    let model = path.parent().ok_or("the AI catalog has no folder")?.join(&job.model);
    if !model_here(&model) {
        return Err(format!("shot content: the model {} is not on this host", model.display()));
    }
    Ok(ShotContentEngine { program: job.program, model, pictures: job.pictures })
}

/// The engine that writes a sentence of what a shot shows, on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShotCaptioner {
    /// The program's name beside the application.
    pub program: String,
    /// The model's folder.
    pub model: PathBuf,
}

impl ShotCaptioner {
    /// The model's name the kept captions carry: the last folder of its path.
    pub fn model_name(&self) -> String {
        self.model.file_name().map_or_else(|| self.model.display().to_string(), |name| name.to_string_lossy().into_owned())
    }
}

/// The shot captioner of this host's catalog; its model must be there.
pub fn installed_shot_captioner() -> Result<ShotCaptioner, String> {
    shot_captioner(&installed_path()?)
}

/// The shot captioner of the catalog at `path`.
pub fn shot_captioner(path: &Path) -> Result<ShotCaptioner, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let job = file.shot_caption.ok_or("the AI catalog of this host has no shot captioner")?;
    let model = path.parent().ok_or("the AI catalog has no folder")?.join(&job.model);
    if !model_here(&model) {
        return Err(format!("shot captioner: the model {} is not on this host", model.display()));
    }
    Ok(ShotCaptioner { program: job.program, model })
}

/// The engine that makes the in-between pictures of a smooth cut on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameInterpolator {
    /// The program's name beside the application.
    pub program: String,
    /// The model's folder.
    pub model: PathBuf,
}

/// The frame interpolator of this host's catalog; its model must be there.
pub fn installed_frame_interpolator() -> Result<FrameInterpolator, String> {
    frame_interpolator(&installed_path()?)
}

/// The frame interpolator of the catalog at `path`.
pub fn frame_interpolator(path: &Path) -> Result<FrameInterpolator, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let job = file.smooth.ok_or("the AI catalog of this host has no frame interpolator")?;
    let model = path.parent().ok_or("the AI catalog has no folder")?.join(&job.model);
    if !model.exists() {
        return Err(format!("frame interpolator: the model {} is not on this host", model.display()));
    }
    Ok(FrameInterpolator { program: job.program, model })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LettersJob {
    program: String,
    model: String,
}

/// The engine that lays a known text on its speech, letter by letter, on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LetterAligner {
    /// The program's name beside the application.
    pub program: String,
    /// The model's folder.
    pub model: PathBuf,
}

/// The letter aligner of this host's catalog; its model must be there.
pub fn installed_letter_aligner() -> Result<LetterAligner, String> {
    letter_aligner(&installed_path()?)
}

/// The letter aligner of the catalog at `path`.
pub fn letter_aligner(path: &Path) -> Result<LetterAligner, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let job = file.letters.ok_or("the AI catalog of this host has no letter aligner")?;
    let model = path.parent().ok_or("the AI catalog has no folder")?.join(&job.model);
    if !model_here(&model) {
        return Err(format!("letter aligner: the model {} is not on this host", model.display()));
    }
    Ok(LetterAligner { program: job.program, model })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    /// The engine of the best text (user 2026-10-26: "Quality" beside Transcribe).
    selected: String,
    /// The fast engine on the processor (user 2026-10-26: the trim zoom's words and "Fast"
    /// beside Transcribe; it never takes the graphics card from the player).
    #[serde(default)]
    fast: Option<String>,
    engines: Vec<Engine>,
}

/// Which engine transcribes: each its own, so both can run at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quality {
    /// The best text (names, letters), as fast as the host allows.
    Best,
    /// Fast on the processor: the trim zoom, a quick pass over many clips.
    Fast,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Engine {
    id: String,
    program: String,
    backend: String,
    /// The model, relative to the catalog's folder: a file, or the folder of a model made
    /// of several files.
    model: String,
    #[serde(default)]
    vad_model: Option<String>,
    threads: u32,
}

/// The engine that transcribes speech on this host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transcriber {
    pub id: String,
    /// The program's name beside the application.
    pub program: String,
    /// `vulkan` or `cpu`.
    pub backend: String,
    pub model: PathBuf,
    pub vad_model: Option<PathBuf>,
    pub threads: u32,
}

/// The catalog of this host (`QNC_AI_CATALOG`, else beside the program) and its folder.
pub fn installed_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("QNC_AI_CATALOG") {
        if path.is_empty() {
            return Err("empty AI catalog override".into());
        }
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    exe.parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .map(|p| p.join(CATALOG))
        .find(|p| p.is_file())
        .ok_or_else(|| format!("the AI catalog ({CATALOG}) is not beside the application"))
}

/// The best transcription engine this host's catalog selects.
pub fn installed_transcriber() -> Result<Transcriber, String> {
    transcriber(&installed_path()?)
}

/// The transcription engine this host's catalog selects for `quality`.
pub fn installed_transcriber_for(quality: Quality) -> Result<Transcriber, String> {
    transcriber_for(&installed_path()?, quality)
}

/// The best transcription engine the catalog at `path` selects.
pub fn transcriber(path: &Path) -> Result<Transcriber, String> {
    transcriber_for(path, Quality::Best)
}

/// The transcription engine the catalog at `path` selects for `quality`; its model must be
/// there. A catalog without a fast engine gives none (nothing falls back).
pub fn transcriber_for(path: &Path, quality: Quality) -> Result<Transcriber, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.version != "1" {
        return Err(format!("AI catalog version {} is not known", file.version));
    }
    let folder = path.parent().ok_or("the AI catalog has no folder")?;
    let selected = match quality {
        Quality::Best => file.transcribe.selected.clone(),
        Quality::Fast => file.transcribe.fast.clone().ok_or("the AI catalog of this host has no fast transcription engine")?,
    };
    engine_in(&file, folder, &selected)
}

/// The transcription engine `id` of this host's catalog, whatever is selected (plug-in contracts
/// version 2, docs/102 step 5: the application names the engine, the program finds its model,
/// backend and threads on the computer it runs on).
pub fn installed_engine(id: &str) -> Result<Transcriber, String> {
    let path = installed_path()?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    engine_in(&file, path.parent().ok_or("the AI catalog has no folder")?, id)
}

fn engine_in(file: &File, folder: &Path, selected: &str) -> Result<Transcriber, String> {
    let engine = file
        .transcribe
        .engines
        .iter()
        .find(|engine| engine.id == selected)
        .ok_or_else(|| format!("the transcription engine {selected} is not in the AI catalog of this host"))?;
    if !BACKENDS.contains(&engine.backend.as_str()) {
        return Err(format!("transcription engine {}: unknown backend {}", engine.id, engine.backend));
    }
    if engine.threads == 0 {
        return Err(format!("transcription engine {}: threads must be at least 1", engine.id));
    }
    let present = |relative: &str| -> Result<PathBuf, String> {
        let file = folder.join(relative);
        if model_here(&file) {
            Ok(file)
        } else {
            Err(format!("transcription engine {}: the model {} is not on this host", engine.id, file.display()))
        }
    };
    Ok(Transcriber {
        id: engine.id.clone(),
        program: engine.program.clone(),
        backend: engine.backend.clone(),
        model: present(&engine.model)?,
        vad_model: engine.vad_model.as_deref().map(present).transpose()?,
        threads: engine.threads,
    })
}

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoreJob {
    #[allow(dead_code)]
    program: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TranslateJob {
    program: String,
    model: String,
    /// The languages of speech (the project's `ai.language` codes) the model translates from.
    from: Vec<String>,
}

/// The engine that translates the words of a story into English on this host (user 2026-11-11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Translator {
    /// The program's name beside the application.
    pub program: String,
    /// The model's folder.
    pub model: PathBuf,
    /// The languages it translates from.
    pub from: Vec<String>,
}

impl Translator {
    /// Whether it translates speech of `language` (an `ai.language` code).
    pub fn translates(&self, language: &str) -> bool {
        self.from.iter().any(|code| code == language)
    }
}

/// The translator of this host's catalog; its model must be there.
pub fn installed_translator() -> Result<Translator, String> {
    translator(&installed_path()?)
}

/// The translator of the catalog at `path`.
pub fn translator(path: &Path) -> Result<Translator, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let job = file.translate.ok_or("the AI catalog of this host has no translator")?;
    let model = path.parent().ok_or("the AI catalog has no folder")?.join(&job.model);
    if !model_here(&model) {
        return Err(format!("translator: the model {} is not on this host", model.display()));
    }
    if job.from.is_empty() {
        return Err("translator: it translates from no language".into());
    }
    Ok(Translator { program: job.program, model, from: job.from })
}

/// The name a model is asked by in a plug-in request (contracts version 2): its file or folder
/// name, the same on every computer that has it.
pub fn model_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// The model named `name` (`model_name`) that a job of this host's catalog uses; it must be on
/// this host (plug-in contracts version 2: the program finds the model it is asked by name).
pub fn installed_model(name: &str) -> Result<PathBuf, String> {
    let path = installed_path()?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let catalog: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let folder = path.parent().ok_or("the AI catalog has no folder")?;
    let mut models = Vec::new();
    collect(&catalog, &["model", "vad_model"], &mut models);
    models
        .into_iter()
        .map(|relative| folder.join(relative))
        .find(|model| model_name(model) == name)
        .filter(|model| model.exists())
        .ok_or_else(|| format!("the model {name} is not in the AI catalog of this host or not on it"))
}

/// The plug-in programs this host's catalog names (an AI server runs only these): every job but
/// the frame interpolator of the smooth cut, which is a tool of this computer, not a plug-in.
pub fn installed_programs() -> Result<Vec<String>, String> {
    let path = installed_path()?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut catalog: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if let Some(jobs) = catalog.as_object_mut() {
        jobs.remove("smooth");
    }
    let mut programs = Vec::new();
    collect(&catalog, &["program"], &mut programs);
    programs.sort_unstable();
    programs.dedup();
    Ok(programs)
}

/// Every string of the catalog under one of `keys`, however deep.
fn collect(value: &serde_json::Value, keys: &[&str], into: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                match value {
                    serde_json::Value::String(text) if keys.contains(&key.as_str()) => into.push(text.clone()),
                    other => collect(other, keys, into),
                }
            }
        }
        serde_json::Value::Array(items) => items.iter().for_each(|item| collect(item, keys, into)),
        _ => {}
    }
}

/// Whether a model the application names is where it runs: on this computer, or on the AI
/// server of this computer (docs/102 step 6: a laptop asking a server needs no models; the
/// program there finds them in its own catalog, strictly).
fn model_here(model: &Path) -> bool {
    model.exists() || qnc_ai_place::server().ok().flatten().is_some()
}

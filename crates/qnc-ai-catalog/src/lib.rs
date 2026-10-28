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
    let engine = file
        .transcribe
        .engines
        .iter()
        .find(|engine| engine.id == selected)
        .ok_or_else(|| format!("the selected transcription engine {selected} is not in the AI catalog"))?;
    if !BACKENDS.contains(&engine.backend.as_str()) {
        return Err(format!("transcription engine {}: unknown backend {}", engine.id, engine.backend));
    }
    if engine.threads == 0 {
        return Err(format!("transcription engine {}: threads must be at least 1", engine.id));
    }
    let present = |relative: &str| -> Result<PathBuf, String> {
        let file = folder.join(relative);
        if file.exists() {
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

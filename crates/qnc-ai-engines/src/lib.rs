//! Whether the engine of an AI job runs on this computer (docs/102 step 4, user 2026-11-11): an
//! AI feature names the engine it uses by a key of this block, and the project's AI section says
//! next to its switch where the engine runs or why it is not installed. It reads the AI catalog
//! of this host and looks for the program beside the application; it starts nothing.

pub const MODULE_ID: &str = "qnc.module.ai-engines";

/// The engines a feature may name: the jobs of the AI catalog of this host.
pub const KEYS: [&str; 7] = ["transcribe.fast", "transcribe.best", "letters", "shot_content", "shot_caption", "translate", "smooth"];

/// Where the engine of `key` runs: on this computer (its program and model), or why it is not
/// installed.
pub fn state(key: &str) -> Result<String, String> {
    use qnc_ai_catalog::Quality;
    let (program, model) = match key {
        "transcribe.fast" => qnc_ai_catalog::installed_transcriber_for(Quality::Fast).map(|engine| (engine.program, engine.model))?,
        "transcribe.best" => qnc_ai_catalog::installed_transcriber_for(Quality::Best).map(|engine| (engine.program, engine.model))?,
        "letters" => qnc_ai_catalog::installed_letter_aligner().map(|engine| (engine.program, engine.model))?,
        "shot_content" => qnc_ai_catalog::installed_shot_content().map(|engine| (engine.program, engine.model))?,
        "shot_caption" => qnc_ai_catalog::installed_shot_captioner().map(|engine| (engine.program, engine.model))?,
        "translate" => qnc_ai_catalog::installed_translator().map(|engine| (engine.program, engine.model))?,
        "smooth" => qnc_ai_catalog::installed_frame_interpolator().map(|engine| (engine.program, engine.model))?,
        other => return Err(format!("no AI engine '{other}'")),
    };
    qnc_plugin_process::beside_application(&program)?;
    let model = model.file_name().map_or_else(|| model.display().to_string(), |name| name.to_string_lossy().into_owned());
    Ok(format!("{program}, {model}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_unknown_engine_is_said_not_guessed() {
        assert!(super::state("whisper").unwrap_err().contains("whisper"));
    }
}

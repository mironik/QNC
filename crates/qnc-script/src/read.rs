//! Reading the script of a project: its blocks, the profile it is written with (a project
//! without one gets the profile chosen on this computer), the target length of its story sheet
//! and the voice records of its blocks with whether their files are there.

use qnc_db_broker::ProjectDbTarget;
use qnc_source_preview::StillLocator;

use crate::{Block, ScriptProfile, Voice};

pub(crate) struct Script {
    pub blocks: Vec<Block>,
    pub profile: Option<ScriptProfile>,
    pub target: Option<f64>,
    /// The story's working title (the sheet's slug).
    pub title: Option<String>,
    /// Every voice record, and whether its clip and the take used are in the project folder.
    pub voices: Vec<(Voice, bool)>,
    /// The fine times of the letters of the words of the statements (user 2026-11-05: cutting
    /// by letters). The words themselves come from the one reader of transcripts
    /// (`qnc-clip-transcripts`), not from here.
    pub letters: std::collections::HashMap<String, Vec<qnc_letter_db::WordLetters>>,
}

pub(crate) type Read = Result<Script, String>;

/// The script, the profile it is written with and the target length of the story sheet (its
/// first length field's value). A project without a profile gets the one chosen on this
/// computer.
pub(crate) fn read_script(target: &ProjectDbTarget, locator: Option<&StillLocator>) -> Read {
    let blocks = qnc_script_db::read(target)?;
    let profile = match qnc_script_db::read_profile(target)? {
        Some(profile) => Some(profile),
        None => {
            let root = qnc_dev_diagnostics::resolve_qnc_root().ok_or_else(|| qnc_ui_text::text("script.no_profile").to_string())?;
            let (profiles, errors) = qnc_script_profile::Profiles::load(&root);
            for error in errors {
                qnc_dev_diagnostics::log_line(qnc_dev_diagnostics::DiagnosticsStream::Player, format!("script-profile refused {error}"));
            }
            let chosen = profiles.current().cloned().ok_or_else(|| qnc_ui_text::text("script.no_profile").to_string())?;
            qnc_script_db::set_profile(target, chosen)?;
            qnc_script_db::read_profile(target)?
        }
    };
    let sheet = qnc_newsroom_sheet_db::read(target).ok();
    // The story's working title is the sheet's slug (user 2026-11-01, MA Script header).
    let title = sheet.as_ref().and_then(|sheet| sheet.values.get("slug")?.as_str().map(str::trim).filter(|title| !title.is_empty()).map(str::to_string));
    let length = sheet.and_then(|sheet| {
        let field = sheet.sheet.get("fields")?.as_array()?.iter().find(|field| field.get("kind").and_then(serde_json::Value::as_str) == Some("duration"))?;
        let code = field.get("id")?.as_str()?;
        sheet.values.get(code)?.as_f64().filter(|seconds| *seconds > 0.0)
    });
    let there = |uri: &str| locator.and_then(|locator| qnc_voice_take::voice_file(locator, uri).ok()).is_some_and(|file| file.is_file());
    let voices = qnc_script_db::read_voices(target)?
        .into_iter()
        .map(|voice| {
            // A reading found in a recording made elsewhere is there when its clip is.
            let present = match voice.chosen_take().and_then(|take| take.source.as_ref()) {
                Some(source) => locator.is_some_and(|locator| locator.locate(&source.clip_id).is_ok()),
                None => there(&voice.clip_uri) && voice.chosen_take().is_some_and(|take| there(&take.wav_uri)),
            };
            (voice, present)
        })
        .collect();
    let mut letters = std::collections::HashMap::new();
    for block in &blocks {
        for clip in crate::view::block_pieces(block).into_iter().map(|piece| piece.clip_id).filter(|clip| !clip.is_empty()) {
            if !letters.contains_key(&clip) {
                letters.insert(clip.clone(), qnc_letter_db::read_clip(target, &clip).unwrap_or_default());
            }
        }
    }
    Ok(Script { blocks, profile, target: length, title, voices, letters })
}

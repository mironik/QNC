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
    /// Every voice record, and whether its clip and the take used are in the project folder.
    pub voices: Vec<(Voice, bool)>,
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
    let length = sheet.and_then(|sheet| {
        let field = sheet.sheet.get("fields")?.as_array()?.iter().find(|field| field.get("kind").and_then(serde_json::Value::as_str) == Some("duration"))?;
        let code = field.get("id")?.as_str()?;
        sheet.values.get(code)?.as_f64().filter(|seconds| *seconds > 0.0)
    });
    let there = |uri: &str| locator.and_then(|locator| qnc_voice_take::voice_file(locator, uri).ok()).is_some_and(|file| file.is_file());
    let voices = qnc_script_db::read_voices(target)?
        .into_iter()
        .map(|voice| {
            let present = there(&voice.clip_uri) && voice.chosen_take().is_some_and(|take| there(&take.wav_uri));
            (voice, present)
        })
        .collect();
    Ok(Script { blocks, profile, target: length, voices })
}

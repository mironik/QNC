//! Reading the script of a project: its blocks, the profile it is written with (a project
//! without one gets the profile chosen on this computer) and the target length of its story sheet.

use qnc_db_broker::ProjectDbTarget;

use crate::{Block, ScriptProfile};

pub(crate) type Read = Result<(Vec<Block>, Option<ScriptProfile>, Option<f64>), String>;

/// The script, the profile it is written with and the target length of the story sheet (its
/// first length field's value). A project without a profile gets the one chosen on this
/// computer.
pub(crate) fn read_script(target: &ProjectDbTarget) -> Read {
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
    Ok((blocks, profile, length))
}

//! Where the AI programs of this computer run (docs/102 step 6, user 2026-11-11: an AI server on
//! the LAN, the intranet, or a laptop). A setting of this computer, never of a project:
//! `ai-place.json` beside the other settings of this computer. Without it every AI program runs
//! beside the application; with `{ "server": "host:port", "token": "..." }` every AI program runs
//! on that server (`qnc-ai-server`), which speaks the same plug-in contracts.

use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const MODULE_ID: &str = "qnc.module.ai-place";
/// The setting of this computer, beside its other settings.
pub const FILE: &str = "ai-place.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    server: String,
    token: String,
}

/// The AI server of this computer from the settings of the QNC `root`; none when AI runs here.
/// A setting that cannot be read is an error, never a quiet fall back to this computer.
pub fn server_of(root: &Path) -> Result<Option<qnc_plugin_process::Remote>, String> {
    let path = qnc_dev_diagnostics::config_path(root).with_file_name(FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("{}: {error}", path.display())),
    };
    let file: File = serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    if file.server.trim().is_empty() || file.token.trim().is_empty() {
        return Err(format!("{}: an AI server needs its address and its token", path.display()));
    }
    Ok(Some(qnc_plugin_process::Remote { address: file.server, token: file.token }))
}

/// The AI server of this computer (the QNC root it runs in); none when AI runs here.
pub fn server() -> Result<Option<qnc_plugin_process::Remote>, String> {
    let root = qnc_dev_diagnostics::resolve_qnc_root().ok_or("no QNC root")?;
    server_of(&root)
}

/// The plug-in spec of AI program `program`: beside the application, or on the AI server of this
/// computer (there the program is named, the server starts its own).
pub fn spec(program: &str, contract: &str, version: u32, timeout: std::time::Duration) -> Result<qnc_plugin_process::PluginSpec, String> {
    let remote = server()?;
    let executable = match &remote {
        Some(_) => PathBuf::from(program),
        None => qnc_plugin_process::beside_application(program)?,
    };
    Ok(qnc_plugin_process::PluginSpec { executable, contract: contract.into(), version, timeout, remote })
}

#[cfg(test)]
mod tests {
    #[test]
    fn without_the_setting_ai_runs_here_and_a_half_setting_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(super::server_of(root.path()).unwrap(), None);
        let file = qnc_dev_diagnostics::config_path(root.path()).with_file_name(super::FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, r#"{"server": "127.0.0.1:7843", "token": ""}"#).unwrap();
        assert!(super::server_of(root.path()).is_err());
        std::fs::write(&file, r#"{"server": "127.0.0.1:7843", "token": "t"}"#).unwrap();
        assert_eq!(super::server_of(root.path()).unwrap().unwrap().address, "127.0.0.1:7843");
    }
}

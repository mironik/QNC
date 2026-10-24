//! What is installed and what works with what (user 2026-10-23, plug-ins step 3): every plug-in
//! program the catalogs name, asked who it is (its id, version and the contract it speaks) and
//! whether the application takes it; every decoder of the decoder catalog with its executable
//! and that executable's own version; and the programs beside the application. It only reads
//! the catalogs and asks the programs; it changes nothing and chooses nothing.

use qnc_record_format_plugin::{FormatCatalog, FormatKind};
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

pub const MODULE_ID: &str = "qnc.module.installed-versions";

/// The whole report.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// The version of the program that made the report (the application build it belongs to).
    pub application_version: String,
    pub plugins: Vec<PluginLine>,
    pub decoders: Vec<DecoderLine>,
    pub programs: Vec<ProgramLine>,
}

/// One plug-in a catalog names.
#[derive(Debug, Clone, Serialize)]
pub struct PluginLine {
    /// What it is for, e.g. `record format camera.sony.index.read`.
    pub role: String,
    /// Its program name, or `built in`.
    pub program: String,
    /// The contract and version the application speaks with it.
    pub contract: String,
    pub contract_version: u32,
    /// What the plug-in said about itself, when it answered.
    pub plugin_id: Option<String>,
    pub plugin_version: Option<String>,
    /// `None` when the application takes it, else why not.
    pub problem: Option<String>,
}

/// One decoder of the decoder catalog.
#[derive(Debug, Clone, Serialize)]
pub struct DecoderLine {
    pub id: String,
    /// The version of the registration (the installed adapter package).
    pub version: String,
    pub protocol: String,
    /// `selected`, `preferred` or empty.
    pub use_: String,
    pub executable: Option<String>,
    /// The first line the executable says about its own version, when it says one.
    pub executable_version: Option<String>,
    /// The program of its jobs besides playback.
    pub jobs: Option<String>,
    pub problem: Option<String>,
}

/// One program beside the application.
#[derive(Debug, Clone, Serialize)]
pub struct ProgramLine {
    pub name: String,
    pub bytes: u64,
    /// Seconds since 1970 of its last change.
    pub modified: u64,
}

/// The report for the QNC root `root` (its catalog of record formats) and the decoder catalog of
/// this host; `application_version` is the version of the calling program.
pub fn report(root: &Path, application_version: &str) -> Report {
    Report {
        application_version: application_version.into(),
        plugins: plugins(root),
        decoders: decoders(),
        programs: programs(),
    }
}

fn plugins(root: &Path) -> Vec<PluginLine> {
    let mut lines = Vec::new();
    match FormatCatalog::read(root) {
        Ok(catalog) => {
            for entry in catalog.formats {
                let role = format!("record format {}", entry.reader_id);
                lines.push(match entry.kind {
                    FormatKind::Builtin { name } => PluginLine {
                        role,
                        program: format!("built in ({name})"),
                        contract: qnc_record_format_plugin::CONTRACT.into(),
                        contract_version: qnc_record_format_plugin::CONTRACT_VERSION,
                        plugin_id: None,
                        plugin_version: None,
                        problem: None,
                    },
                    FormatKind::Plugin { program, contract_version, timeout_ms } => {
                        let mut line = ask(role, &program, qnc_record_format_plugin::CONTRACT, contract_version, Duration::from_millis(timeout_ms));
                        if line.problem.is_none() && contract_version != qnc_record_format_plugin::CONTRACT_VERSION {
                            line.problem = Some(format!(
                                "the catalog asks version {contract_version}, this application speaks {}",
                                qnc_record_format_plugin::CONTRACT_VERSION
                            ));
                        }
                        line
                    }
                });
            }
        }
        Err(error) => lines.push(PluginLine {
            role: "record formats".into(),
            program: String::new(),
            contract: qnc_record_format_plugin::CONTRACT.into(),
            contract_version: qnc_record_format_plugin::CONTRACT_VERSION,
            plugin_id: None,
            plugin_version: None,
            problem: Some(error),
        }),
    }
    if let Ok((catalog, _)) = qnc_decoder_catalog::installed_catalog() {
        for registration in &catalog.adapters {
            if let Some(jobs) = &registration.jobs {
                let role = format!("decoder jobs of {}", registration.id);
                lines.push(ask(role, &jobs.program, qnc_decoder_jobs::CONTRACT, qnc_decoder_jobs::CONTRACT_VERSION, Duration::from_secs(10)));
            }
        }
    }
    lines
}

/// Starts `program` beside the application and asks who it is.
fn ask(role: String, program: &str, contract: &str, version: u32, timeout: Duration) -> PluginLine {
    let mut line = PluginLine {
        role,
        program: program.into(),
        contract: contract.into(),
        contract_version: version,
        plugin_id: None,
        plugin_version: None,
        problem: None,
    };
    let executable = match qnc_plugin_process::beside_application(program) {
        Ok(executable) => executable,
        Err(error) => {
            line.problem = Some(error);
            return line;
        }
    };
    let spec = qnc_plugin_process::PluginSpec { executable, contract: contract.into(), version, timeout };
    match qnc_plugin_process::Plugin::new(spec).ready() {
        Ok(ready) => {
            line.plugin_id = Some(ready.plugin_id);
            line.plugin_version = Some(ready.plugin_version);
        }
        Err(error) => line.problem = Some(error),
    }
    line
}

fn decoders() -> Vec<DecoderLine> {
    let (catalog, directory) = match qnc_decoder_catalog::installed_catalog() {
        Ok(found) => found,
        Err(error) => {
            return vec![DecoderLine {
                id: String::new(),
                version: String::new(),
                protocol: String::new(),
                use_: String::new(),
                executable: None,
                executable_version: None,
                jobs: None,
                problem: Some(error.to_string()),
            }]
        }
    };
    catalog
        .adapters
        .iter()
        .map(|registration| {
            let protocol = serde_json::to_value(&registration.driver)
                .ok()
                .and_then(|driver| driver.get("protocol").and_then(|p| p.as_str()).map(String::from))
                .unwrap_or_default();
            let use_ = if registration.id == catalog.selected {
                "selected"
            } else if catalog.prefer.contains(&registration.id) {
                "preferred"
            } else {
                ""
            };
            let (executable, executable_version, problem) = match registration.executable_path(&directory) {
                Ok(path) => {
                    let version = if protocol.starts_with("ffmpeg") || protocol == "libav_v1" { first_version_line(&path) } else { None };
                    (Some(path.display().to_string()), version, None)
                }
                Err(error) => (None, None, Some(error.to_string())),
            };
            DecoderLine {
                id: registration.id.clone(),
                version: registration.version.clone(),
                protocol,
                use_: use_.into(),
                executable,
                executable_version,
                jobs: registration.jobs.as_ref().map(|jobs| jobs.program.clone()),
                problem,
            }
        })
        .collect()
}

/// The first line of `<executable> -version` (the ffmpeg family says its build there).
fn first_version_line(executable: &Path) -> Option<String> {
    let mut command = std::process::Command::new(executable);
    command.arg("-version").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let output = command.output().ok()?;
    String::from_utf8_lossy(&output.stdout).lines().next().map(|line| line.trim().to_string()).filter(|line| !line.is_empty())
}

/// The QNC programs in the folder of the running program.
fn programs() -> Vec<ProgramLine> {
    let Some(folder) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(PathBuf::from)) else {
        return Vec::new();
    };
    let mut lines: Vec<ProgramLine> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let program = name.strip_suffix(std::env::consts::EXE_SUFFIX).filter(|_| !std::env::consts::EXE_SUFFIX.is_empty()).unwrap_or(&name);
            if !program.starts_with("qnc-") || program.contains('.') {
                return None;
            }
            let meta = entry.metadata().ok().filter(|meta| meta.is_file())?;
            let modified = meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
            Some(ProgramLine { name: program.into(), bytes: meta.len(), modified })
        })
        .collect();
    lines.sort_by(|a, b| a.name.cmp(&b.name));
    lines
}

/// `modified` as a UTC date and time `YYYY-MM-DD HH:MM`.
pub fn utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    // Days since 1970-01-01 to a civil date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rest / 3600, rest % 3600 / 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_date_is_written_as_utc() {
        assert_eq!(utc(0), "1970-01-01 00:00");
        assert_eq!(utc(1_761_264_000), "2025-10-24 00:00");
        assert_eq!(utc(951_782_400 + 3_661), "2000-02-29 01:01");
    }

    #[test]
    fn a_missing_plug_in_program_is_reported_not_hidden() {
        let line = ask("decoder jobs of x".into(), "qnc-no-such-program", qnc_decoder_jobs::CONTRACT, 1, Duration::from_secs(1));
        assert!(line.problem.is_some() && line.plugin_id.is_none());
    }
}

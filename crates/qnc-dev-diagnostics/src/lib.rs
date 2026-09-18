//! Shared development diagnostics switches.
//!
//! This module owns only development diagnostics flags. It does not own QNC
//! workflow, media, project state, shell routing or application state.

use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const MODULE_ID: &str = "qnc.module.dev-diagnostics";
pub const VERSION: &str = "0.1.0";
pub const CONFIG_VERSION: &str = "1";
pub const PLAYER_ENV: &str = "QNC_PLAYER_DIAGNOSTICS";
pub const FILMSTRIP_ENV: &str = "QNC_FILMSTRIP_DIAGNOSTICS";
pub const WAVE_ENV: &str = "QNC_WAVE_DIAGNOSTICS";
const CACHE_TTL: Duration = Duration::from_millis(250);
const MAX_CONFIG_BYTES: u64 = 16 * 1024;
const MAX_LOG_READ_BYTES: u64 = 256 * 1024;
const MAX_TEST_OUTPUT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DiagnosticsSettings {
    pub version: String,
    pub player_diagnostics: bool,
    pub filmstrip_diagnostics: bool,
    pub wave_diagnostics: bool,
    pub updated_unix_ms: u64,
}

impl Default for DiagnosticsSettings {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION.into(),
            player_diagnostics: false,
            filmstrip_diagnostics: false,
            wave_diagnostics: false,
            updated_unix_ms: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsState {
    pub qnc_root: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub saved: DiagnosticsSettings,
    pub effective_player_diagnostics: bool,
    pub effective_filmstrip_diagnostics: bool,
    pub effective_wave_diagnostics: bool,
    pub player_env_override: Option<bool>,
    pub filmstrip_env_override: Option<bool>,
    pub wave_env_override: Option<bool>,
    pub read_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsStream {
    Player,
    Filmstrip,
    Wave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticCheckId {
    IngestCore,
    ArtifactTimeline,
    PlayerStack,
    ComponentBoundaries,
    Conformance,
    DiagnosticsTool,
    AppBuild,
}

impl DiagnosticCheckId {
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticCheckId::IngestCore => "ingest_core",
            DiagnosticCheckId::ArtifactTimeline => "artifact_timeline",
            DiagnosticCheckId::PlayerStack => "player_stack",
            DiagnosticCheckId::ComponentBoundaries => "component_boundaries",
            DiagnosticCheckId::Conformance => "conformance",
            DiagnosticCheckId::DiagnosticsTool => "diagnostics_tool",
            DiagnosticCheckId::AppBuild => "app_build",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        DIAGNOSTIC_CHECKS
            .iter()
            .find(|check| check.id.as_str() == value)
            .map(|check| check.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticCheck {
    pub id: DiagnosticCheckId,
    pub title: &'static str,
    pub description: &'static str,
    pub args: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticTestResult {
    pub id: DiagnosticCheckId,
    pub title: String,
    pub command_line: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: u128,
    pub output: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticTestReport {
    pub started_unix_ms: u64,
    pub duration_ms: u128,
    pub results: Vec<DiagnosticTestResult>,
}

impl DiagnosticTestReport {
    pub fn success(&self) -> bool {
        !self.results.is_empty() && self.results.iter().all(|result| result.success)
    }
}

const INGEST_CORE_ARGS: &[&str] = &[
    "test",
    "-p",
    "qnc-ingest-application",
    "-p",
    "qnc-ingest-select",
    "-p",
    "qnc-media-thumbnail",
    "--quiet",
];
const ARTIFACT_TIMELINE_ARGS: &[&str] = &[
    "test",
    "-p",
    "qnc-filmstrip",
    "-p",
    "qnc-filmstrip-worker",
    "-p",
    "qnc-wave",
    "-p",
    "qnc-wave-worker",
    "-p",
    "qnc-wave-view",
    "-p",
    "qnc-timeline",
    "-p",
    "qnc-timeline-assets",
    "--quiet",
];
const PLAYER_STACK_ARGS: &[&str] = &[
    "test",
    "-p",
    "qnc-player-input",
    "-p",
    "qnc-player-client",
    "-p",
    "qnc-player-launcher",
    "-p",
    "qnc-player-timeline",
    "-p",
    "qnc-player-frame-transport",
    "-p",
    "qnc-broadcast-player",
    "-p",
    "qnc-broadcast-engine",
    "--quiet",
];
const CONFORMANCE_ARGS: &[&str] = &["run", "-p", "qnc-conformance", "--quiet"];
const DIAGNOSTICS_TOOL_ARGS: &[&str] = &[
    "test",
    "-p",
    "qnc-dev-diagnostics",
    "-p",
    "qnc-dev-diagnostics-app",
    "--quiet",
];
const APP_BUILD_ARGS: &[&str] = &[
    "build",
    "-p",
    "qnc-app",
    "-p",
    "qnc-ingest",
    "-p",
    "qnc-player-runner",
    "--quiet",
];

const DIAGNOSTIC_CHECKS: &[DiagnosticCheck] = &[
    DiagnosticCheck {
        id: DiagnosticCheckId::IngestCore,
        title: "Ingest core",
        description: "Application, Select session i thumbnails.",
        args: INGEST_CORE_ARGS,
    },
    DiagnosticCheck {
        id: DiagnosticCheckId::ArtifactTimeline,
        title: "Artifacts + Timeline",
        description: "Filmstrip, Wave, Timeline i timeline-assets.",
        args: ARTIFACT_TIMELINE_ARGS,
    },
    DiagnosticCheck {
        id: DiagnosticCheckId::PlayerStack,
        title: "Player stack",
        description: "Player ulaz, klijent, launcher, frame transport i engine.",
        args: PLAYER_STACK_ARGS,
    },
    DiagnosticCheck {
        id: DiagnosticCheckId::ComponentBoundaries,
        title: "Component boundaries",
        description: "Javni worker/player-input moduli bez runtime Ingest ovisnosti.",
        args: &[],
    },
    DiagnosticCheck {
        id: DiagnosticCheckId::Conformance,
        title: "Conformance",
        description: "Projektni zakon i boundary provjere.",
        args: CONFORMANCE_ARGS,
    },
    DiagnosticCheck {
        id: DiagnosticCheckId::DiagnosticsTool,
        title: "Diagnostics tool",
        description: "Diagnostics modul i forma.",
        args: DIAGNOSTICS_TOOL_ARGS,
    },
    DiagnosticCheck {
        id: DiagnosticCheckId::AppBuild,
        title: "App build",
        description: "Glavne QNC aplikacije i pomocni player binary.",
        args: APP_BUILD_ARGS,
    },
];

#[derive(Debug, Clone)]
struct CachedSettings {
    loaded_at: Instant,
    path: Option<PathBuf>,
    settings: DiagnosticsSettings,
}

static CACHE: OnceLock<Mutex<Option<CachedSettings>>> = OnceLock::new();

pub fn config_path(root: &Path) -> PathBuf {
    root.join("data").join("dev-diagnostics.json")
}

pub fn log_dir(root: &Path) -> PathBuf {
    root.join("data").join("diagnostics")
}

pub fn log_path(root: &Path, stream: DiagnosticsStream) -> PathBuf {
    let file_name = match stream {
        DiagnosticsStream::Player => "player.log",
        DiagnosticsStream::Filmstrip => "filmstrip.log",
        DiagnosticsStream::Wave => "wave.log",
    };
    log_dir(root).join(file_name)
}

pub fn diagnostic_checks() -> &'static [DiagnosticCheck] {
    DIAGNOSTIC_CHECKS
}

pub fn run_diagnostic_checks(root: &Path, ids: &[DiagnosticCheckId]) -> DiagnosticTestReport {
    let started = Instant::now();
    let started_unix_ms = unix_ms();
    let mut results = Vec::new();
    if !is_qnc_root(root) {
        results.push(DiagnosticTestResult {
            id: DiagnosticCheckId::Conformance,
            title: "QNC root".into(),
            command_line: root.display().to_string(),
            success: false,
            exit_code: None,
            duration_ms: 0,
            output: "QNC root nije valjan.".into(),
        });
        return DiagnosticTestReport {
            started_unix_ms,
            duration_ms: started.elapsed().as_millis(),
            results,
        };
    }
    for id in ids {
        let Some(check) = diagnostic_check(*id) else {
            results.push(DiagnosticTestResult {
                id: *id,
                title: id.as_str().into(),
                command_line: String::new(),
                success: false,
                exit_code: None,
                duration_ms: 0,
                output: "Nepoznata diagnostics provjera.".into(),
            });
            continue;
        };
        results.push(run_one_check(root, check));
    }
    DiagnosticTestReport {
        started_unix_ms,
        duration_ms: started.elapsed().as_millis(),
        results,
    }
}

fn diagnostic_check(id: DiagnosticCheckId) -> Option<&'static DiagnosticCheck> {
    DIAGNOSTIC_CHECKS.iter().find(|check| check.id == id)
}

fn run_one_check(root: &Path, check: &DiagnosticCheck) -> DiagnosticTestResult {
    if check.id == DiagnosticCheckId::ComponentBoundaries {
        return run_component_boundary_check(root, check);
    }
    let cargo = cargo_program();
    let command_line = command_line(&cargo, check.args);
    let started = Instant::now();
    let output = Command::new(&cargo)
        .current_dir(root)
        .env("QNC_ROOT", root)
        .args(check.args)
        .output();
    let duration_ms = started.elapsed().as_millis();
    match output {
        Ok(output) => DiagnosticTestResult {
            id: check.id,
            title: check.title.into(),
            command_line,
            success: output.status.success(),
            exit_code: output.status.code(),
            duration_ms,
            output: process_output(&output.stdout, &output.stderr),
        },
        Err(error) => DiagnosticTestResult {
            id: check.id,
            title: check.title.into(),
            command_line,
            success: false,
            exit_code: None,
            duration_ms,
            output: format!("Pokretanje provjere nije uspjelo: {error}"),
        },
    }
}

fn run_component_boundary_check(root: &Path, check: &DiagnosticCheck) -> DiagnosticTestResult {
    let cargo = cargo_program();
    let packages = [
        "qnc-filmstrip-worker",
        "qnc-wave-worker",
        "qnc-player-input",
    ];
    let started = Instant::now();
    let mut output = String::new();
    let mut success = true;
    let mut exit_code = Some(0);
    for package in packages {
        let args = [
            "tree", "-p", package, "--edges", "normal", "--prefix", "none",
        ];
        output.push_str(&format!("COMMAND {}\n", command_line(&cargo, &args)));
        match Command::new(&cargo)
            .current_dir(root)
            .env("QNC_ROOT", root)
            .args(args)
            .output()
        {
            Ok(result) => {
                exit_code = result.status.code();
                let text = process_output(&result.stdout, &result.stderr);
                output.push_str(&text);
                output.push('\n');
                if !result.status.success() {
                    success = false;
                    continue;
                }
                if text.contains("qnc-ingest-") {
                    success = false;
                    output.push_str(&format!(
                        "BOUNDARY FAIL: {package} has runtime qnc-ingest-* dependency.\n"
                    ));
                } else {
                    output.push_str(&format!(
                        "BOUNDARY OK: {package} has no runtime qnc-ingest-* dependency.\n"
                    ));
                }
            }
            Err(error) => {
                success = false;
                exit_code = None;
                output.push_str(&format!("Pokretanje cargo tree nije uspjelo: {error}\n"));
            }
        }
    }
    DiagnosticTestResult {
        id: check.id,
        title: check.title.into(),
        command_line: "cargo tree boundary scan".into(),
        success,
        exit_code,
        duration_ms: started.elapsed().as_millis(),
        output: truncate_test_output(output),
    }
}

fn cargo_program() -> String {
    env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}

fn command_line(program: &str, args: &[&str]) -> String {
    std::iter::once(program.to_string())
        .chain(args.iter().map(|arg| shell_display_arg(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_display_arg(arg: &str) -> String {
    if arg.chars().any(char::is_whitespace) {
        format!("\"{}\"", arg.replace('"', "\\\""))
    } else {
        arg.into()
    }
}

fn process_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut text = String::new();
    if !stdout.is_empty() {
        text.push_str("STDOUT\n");
        text.push_str(&String::from_utf8_lossy(stdout));
    }
    if !stderr.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str("STDERR\n");
        text.push_str(&String::from_utf8_lossy(stderr));
    }
    if text.trim().is_empty() {
        text.push_str("Nema izlaza.");
    }
    truncate_test_output(text)
}

fn truncate_test_output(text: String) -> String {
    if text.len() <= MAX_TEST_OUTPUT_BYTES {
        return text;
    }
    let start = text
        .char_indices()
        .rev()
        .find(|(index, _)| text.len() - *index <= MAX_TEST_OUTPUT_BYTES)
        .map(|(index, _)| index)
        .unwrap_or(0);
    format!(
        "[izlaz skracen; prikazan je zadnji dio]\n{}",
        &text[start..]
    )
}

pub fn load_from_root(root: &Path) -> Result<DiagnosticsSettings, String> {
    load_from_path(&config_path(root))
}

pub fn load_from_path(path: &Path) -> Result<DiagnosticsSettings, String> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DiagnosticsSettings::default());
        }
        Err(error) => return Err(format!("Diagnostics config metadata: {error}")),
    };
    if !metadata.is_file() {
        return Err("Diagnostics config path is not a file.".into());
    }
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err("Diagnostics config exceeds size limit.".into());
    }
    let bytes = fs::read(path).map_err(|error| format!("Diagnostics config read: {error}"))?;
    let settings = serde_json::from_slice::<DiagnosticsSettings>(&bytes)
        .map_err(|error| format!("Diagnostics config JSON: {error}"))?;
    validate(&settings)?;
    Ok(settings)
}

pub fn save_to_root(root: &Path, mut settings: DiagnosticsSettings) -> Result<(), String> {
    settings.version = CONFIG_VERSION.into();
    settings.updated_unix_ms = unix_ms();
    validate(&settings)?;
    let path = config_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Diagnostics config dir: {error}"))?;
    }
    let bytes = serde_json::to_vec_pretty(&settings)
        .map_err(|error| format!("Diagnostics config encode: {error}"))?;
    fs::write(&path, bytes).map_err(|error| format!("Diagnostics config write: {error}"))?;
    clear_cache();
    Ok(())
}

pub fn diagnostics_state() -> DiagnosticsState {
    let root = locate_qnc_root();
    let path = root.as_ref().map(|root| config_path(root));
    let (saved, read_error) = match path.as_ref() {
        Some(path) => match load_from_path(path) {
            Ok(settings) => (settings, None),
            Err(error) => (DiagnosticsSettings::default(), Some(error)),
        },
        None => (
            DiagnosticsSettings::default(),
            Some("QNC root nije pronadjen.".into()),
        ),
    };
    let player_env_override = env_flag(PLAYER_ENV);
    let filmstrip_env_override = env_flag(FILMSTRIP_ENV);
    let wave_env_override = env_flag(WAVE_ENV);
    DiagnosticsState {
        qnc_root: root,
        config_path: path,
        effective_player_diagnostics: player_env_override.unwrap_or(saved.player_diagnostics),
        effective_filmstrip_diagnostics: filmstrip_env_override
            .unwrap_or(saved.filmstrip_diagnostics),
        effective_wave_diagnostics: wave_env_override.unwrap_or(saved.wave_diagnostics),
        saved,
        player_env_override,
        filmstrip_env_override,
        wave_env_override,
        read_error,
    }
}

pub fn player_diagnostics_enabled() -> bool {
    env_flag(PLAYER_ENV).unwrap_or_else(|| cached_settings().player_diagnostics)
}

pub fn filmstrip_diagnostics_enabled() -> bool {
    env_flag(FILMSTRIP_ENV).unwrap_or_else(|| cached_settings().filmstrip_diagnostics)
}

pub fn wave_diagnostics_enabled() -> bool {
    env_flag(WAVE_ENV).unwrap_or_else(|| cached_settings().wave_diagnostics)
}

pub fn log_line(stream: DiagnosticsStream, message: impl AsRef<str>) {
    let Some(root) = locate_qnc_root() else {
        return;
    };
    let path = log_path(&root, stream);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut file = match fs::OpenOptions::new().create(true).append(true).open(path) {
        Ok(file) => file,
        Err(_) => return,
    };
    // One write per line: `writeln!` on a bare `File` issues several writes, so
    // lines from concurrent processes interleave in the shared log.
    let line = format!("{} {}\n", unix_ms(), message.as_ref());
    let _ = file.write_all(line.as_bytes());
}

pub fn recent_lines_from_root(
    root: &Path,
    stream: DiagnosticsStream,
    max_lines: usize,
) -> Result<Vec<String>, String> {
    let path = log_path(root, stream);
    let mut file = match fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("Diagnostics log open: {error}")),
    };
    let len = file
        .metadata()
        .map_err(|error| format!("Diagnostics log metadata: {error}"))?
        .len();
    let start = len.saturating_sub(MAX_LOG_READ_BYTES);
    file.seek(SeekFrom::Start(start))
        .map_err(|error| format!("Diagnostics log seek: {error}"))?;
    let mut bytes = Vec::new();
    file.take(MAX_LOG_READ_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Diagnostics log read: {error}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    if lines.len() > max_lines {
        lines.drain(0..lines.len() - max_lines);
    }
    Ok(lines)
}

pub fn clear_log(root: &Path, stream: DiagnosticsStream) -> Result<(), String> {
    let path = log_path(root, stream);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("Diagnostics log dir: {error}"))?;
    }
    fs::write(path, "").map_err(|error| format!("Diagnostics log clear: {error}"))
}

pub fn clear_cache() {
    if let Some(cache) = CACHE.get() {
        *cache.lock().unwrap() = None;
    }
}

pub fn locate_qnc_root() -> Option<PathBuf> {
    if let Ok(root) = env::var("QNC_ROOT") {
        let root = PathBuf::from(root);
        if is_qnc_root(&root) {
            return Some(root);
        }
    }

    let mut starts = Vec::new();
    if let Ok(exe) = env::current_exe() {
        starts.push(exe);
    }
    if let Ok(cwd) = env::current_dir() {
        starts.push(cwd);
    }

    for start in starts {
        let base = if start.is_file() {
            start.parent().map(PathBuf::from)
        } else {
            Some(start)
        };
        if let Some(base) = base {
            for candidate in base.ancestors() {
                if is_qnc_root(candidate) {
                    return Some(candidate.to_path_buf());
                }
            }
        }
    }
    None
}

fn cached_settings() -> DiagnosticsSettings {
    let root = locate_qnc_root();
    let path = root.as_ref().map(|root| config_path(root));
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    let mut cache = cache.lock().unwrap();
    if let Some(cached) = cache.as_ref() {
        if cached.path == path && cached.loaded_at.elapsed() < CACHE_TTL {
            return cached.settings.clone();
        }
    }
    let settings = path
        .as_ref()
        .and_then(|path| load_from_path(path).ok())
        .unwrap_or_default();
    *cache = Some(CachedSettings {
        loaded_at: Instant::now(),
        path,
        settings: settings.clone(),
    });
    settings
}

fn validate(settings: &DiagnosticsSettings) -> Result<(), String> {
    if settings.version != CONFIG_VERSION {
        return Err("Unsupported diagnostics config version.".into());
    }
    Ok(())
}

fn is_qnc_root(path: &Path) -> bool {
    path.join("AGENTS.md").is_file()
        && path.join("Cargo.toml").is_file()
        && path.join("contracts").is_dir()
}

fn env_flag(name: &str) -> Option<bool> {
    let value = env::var(name).ok()?;
    let normalized = value.trim().to_ascii_lowercase();
    Some(!matches!(
        normalized.as_str(),
        "" | "0" | "false" | "off" | "no"
    ))
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn diagnostic_checks_have_unique_ids() {
        let ids = diagnostic_checks()
            .iter()
            .map(|check| check.id)
            .collect::<BTreeSet<_>>();

        assert_eq!(ids.len(), diagnostic_checks().len());
    }

    #[test]
    fn diagnostic_runner_rejects_invalid_root_before_cargo() {
        let root = env::temp_dir().join(format!("qnc_diag_invalid_{}", unix_ms()));

        let report = run_diagnostic_checks(&root, &[DiagnosticCheckId::DiagnosticsTool]);

        assert!(!report.success());
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].output, "QNC root nije valjan.");
    }

    #[test]
    fn missing_config_loads_disabled_defaults() {
        let root = env::temp_dir().join(format!("qnc_diag_missing_{}", unix_ms()));

        let settings = load_from_root(&root).unwrap();

        assert!(!settings.player_diagnostics);
        assert!(!settings.filmstrip_diagnostics);
        assert!(!settings.wave_diagnostics);
    }

    #[test]
    fn saved_settings_roundtrip() {
        let root = env::temp_dir().join(format!("qnc_diag_roundtrip_{}", unix_ms()));
        let settings = DiagnosticsSettings {
            player_diagnostics: true,
            filmstrip_diagnostics: true,
            wave_diagnostics: true,
            ..Default::default()
        };

        save_to_root(&root, settings).unwrap();
        let loaded = load_from_root(&root).unwrap();

        assert!(loaded.player_diagnostics);
        assert!(loaded.filmstrip_diagnostics);
        assert!(loaded.wave_diagnostics);
        assert_eq!(loaded.version, CONFIG_VERSION);
        assert!(loaded.updated_unix_ms > 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn log_tail_reads_recent_lines() {
        let root = env::temp_dir().join(format!("qnc_diag_log_{}", unix_ms()));
        let path = log_path(&root, DiagnosticsStream::Filmstrip);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "a\nb\nc\n").unwrap();

        let lines = recent_lines_from_root(&root, DiagnosticsStream::Filmstrip, 2).unwrap();

        assert_eq!(lines, ["b", "c"]);
        clear_log(&root, DiagnosticsStream::Filmstrip).unwrap();
        assert!(
            recent_lines_from_root(&root, DiagnosticsStream::Filmstrip, 2)
                .unwrap()
                .is_empty()
        );
        let _ = fs::remove_dir_all(root);
    }
}

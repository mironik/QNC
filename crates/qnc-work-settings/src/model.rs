use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

pub const VERSION: &str = "0.1.0";

/// The loudness a delivered story keeps (EBU R 128 by default): integrated loudness of A1 and
/// of A2, the highest true peak and how far a lane may lie from its target.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Loudness {
    pub a1_lufs: f64,
    pub a2_lufs: f64,
    pub true_peak_dbtp: f64,
    pub tolerance_lu: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackInput {
    Original,
    Proxy,
    ProxyIfAvailable,
    /// The optimized copy the import made (H.264 8-bit 4:2:0 of the original, same frames,
    /// timecode and sound), else the original (user 2026-10-09).
    OptimizedIfAvailable,
}

/// Whether a timeline artifact of the project is made in the background after Select
/// (`auto`) or never (`off`); the project settings decide (user rule 2026-09-30).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactMode {
    Auto,
    Off,
}

/// A timeline artifact the project settings govern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactKind {
    Filmstrip,
    Wave,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadError {
    pub code: String,
    pub message: String,
}

impl ReadError {
    pub(crate) fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ReadError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkSettings {
    pub contract_version: String,
    pub project_id: String,
    pub project_name: String,
    pub workspace_db_uri: String,
    pub output_root_uri: String,
    pub storage: StoragePolicy,
    pub products: ProductLocations,
    pub input: Value,
    pub playback: Value,
    pub video: Value,
    pub audio: Value,
    pub ai: Value,
    pub keyboard_shortcuts: Value,
    /// `artifacts` as saved (`Null` when the project has none); read through
    /// `artifact_mode`, never substituted.
    #[serde(default)]
    pub artifacts: Value,
    /// `export` as saved (the delivery format of the project; `Null` when it has none),
    /// read only: the prerender of the story writes this format.
    #[serde(default)]
    pub export: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductArea {
    Thumbnails,
    Filmstrip,
    VirtualShorts,
    VirtualSegments,
    BRollVirtualClips,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductLocations {
    pub root: String,
    pub thumbnails: String,
    pub filmstrip: String,
    pub virtual_shorts: String,
    pub virtual_segments: String,
    pub b_roll_virtual_clips: String,
    /// The prerender of the story (user 2026-10-12); a project made before it has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerender: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoragePolicy {
    pub ingest_profile: String,
    pub ingest_media: String,
    pub proxy_policy: String,
    pub original_policy: String,
}

impl WorkSettings {
    /// Interpret the existing saved policy; never substitute storage policy or a default.
    pub fn playback_input(&self) -> Result<PlaybackInput, ReadError> {
        match self.playback.get("input").and_then(Value::as_str) {
            Some("original") => Ok(PlaybackInput::Original),
            Some("proxy") => Ok(PlaybackInput::Proxy),
            Some("proxy_if_available") => Ok(PlaybackInput::ProxyIfAvailable),
            Some("optimized_if_available") => Ok(PlaybackInput::OptimizedIfAvailable),
            _ => Err(ReadError::new(
                "playback_input",
                "The database holds an unsupported playback.input.",
            )),
        }
    }

    /// Whether the filmstrip or wave of this project is made (`artifacts.filmstrip`,
    /// `artifacts.wave`). A project without it gets a controlled error, not a default.
    pub fn artifact_mode(&self, kind: ArtifactKind) -> Result<ArtifactMode, ReadError> {
        let key = match kind {
            ArtifactKind::Filmstrip => "filmstrip",
            ArtifactKind::Wave => "wave",
        };
        match self.artifacts.get(key).and_then(Value::as_str) {
            Some("auto") => Ok(ArtifactMode::Auto),
            Some("off") => Ok(ArtifactMode::Off),
            _ => Err(ReadError::new(
                "artifacts",
                "The project settings have no valid artifacts.filmstrip / artifacts.wave.",
            )),
        }
    }

    /// For showing only: filmstrips are made unless the project says `off` (the poster
    /// then fills the filmstrip row). Making them reads `artifact_mode`, strictly.
    pub fn filmstrip_made(&self) -> bool {
        self.artifact_mode(ArtifactKind::Filmstrip) != Ok(ArtifactMode::Off)
    }

    pub fn audio_channels(&self) -> Result<u16, ReadError> {
        self.audio
            .get("channels")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| (1..=64).contains(value))
            .ok_or_else(|| {
                ReadError::new("audio_channels", "The database holds an unsupported audio.channels.")
            })
    }

    /// Frames of the crossfade on every A2 cut (user 2026-10-21); a project without it is
    /// completed when Project opens it, a missing value is a controlled error.
    pub fn audio_crossfade_frames(&self) -> Result<u16, ReadError> {
        self.audio
            .get("crossfade_frames")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value <= 50)
            .ok_or_else(|| ReadError::new("audio_crossfade_frames", "The database holds no valid audio.crossfade_frames."))
    }

    /// The loudness targets of A1 and A2 (user 2026-10-24: EBU R 128 by default, changeable in
    /// the project); a missing or unlikely value is a controlled error, never a default here.
    pub fn audio_loudness(&self) -> Result<Loudness, ReadError> {
        let bad = || ReadError::new("audio_loudness", "The database holds no valid audio.loudness.");
        let loudness = self.audio.get("loudness").ok_or_else(bad)?;
        let number = |key: &str| loudness.get(key).and_then(Value::as_f64).filter(|value| value.is_finite()).ok_or_else(bad);
        let target = |key: &str| number(key).and_then(|lufs| if (-70.0..=0.0).contains(&lufs) { Ok(lufs) } else { Err(bad()) });
        let true_peak_dbtp = number("true_peak_dbtp")?;
        let tolerance_lu = number("tolerance_lu")?;
        if !(-20.0..=0.0).contains(&true_peak_dbtp) || !(0.0..=10.0).contains(&tolerance_lu) {
            return Err(bad());
        }
        Ok(Loudness { a1_lufs: target("a1_lufs")?, a2_lufs: target("a2_lufs")?, true_peak_dbtp, tolerance_lu })
    }

    /// The lead-out after the end of the story in seconds (user 2026-10-24: 2 s by default,
    /// the client may ask other); a missing value is a controlled error.
    pub fn export_tail_seconds(&self) -> Result<f64, ReadError> {
        self.export
            .get("tail_seconds")
            .and_then(Value::as_f64)
            .filter(|seconds| seconds.is_finite() && (0.0..=600.0).contains(seconds))
            .ok_or_else(|| ReadError::new("export_tail_seconds", "The database holds no valid export.tail_seconds."))
    }

    pub(crate) fn from_saved(
        project_id: String,
        project_name: String,
        workspace_db_uri: String,
        output_root_uri: String,
        saved: Value,
    ) -> Result<Self, ReadError> {
        let storage = saved.get("storage").ok_or_else(incomplete)?;
        let result = Self {
            contract_version: VERSION.into(),
            project_id,
            project_name,
            workspace_db_uri,
            output_root_uri,
            storage: StoragePolicy {
                ingest_profile: required_text(storage, "ingest_profile")?,
                ingest_media: required_text(storage, "ingest_media")?,
                proxy_policy: required_text(storage, "proxy_policy")?,
                original_policy: required_text(storage, "original_policy")?,
            },
            products: ProductLocations {
                root: required_text(saved.get("products").ok_or_else(incomplete)?, "root")?,
                thumbnails: required_text(
                    saved.get("products").ok_or_else(incomplete)?,
                    "thumbnails",
                )?,
                filmstrip: required_text(
                    saved.get("products").ok_or_else(incomplete)?,
                    "filmstrip",
                )?,
                virtual_shorts: required_text(
                    saved.get("products").ok_or_else(incomplete)?,
                    "virtual_shorts",
                )?,
                virtual_segments: required_text(
                    saved.get("products").ok_or_else(incomplete)?,
                    "virtual_segments",
                )?,
                b_roll_virtual_clips: required_text(
                    saved.get("products").ok_or_else(incomplete)?,
                    "b_roll_virtual_clips",
                )?,
                prerender: saved
                    .get("products")
                    .and_then(|products| products.get("prerender"))
                    .map(|_| required_text(saved.get("products").ok_or_else(incomplete)?, "prerender"))
                    .transpose()?,
            },
            input: object(&saved, "input")?,
            playback: object(&saved, "playback")?,
            video: object(&saved, "video")?,
            audio: object(&saved, "audio")?,
            ai: object(&saved, "ai")?,
            keyboard_shortcuts: object(&saved, "keyboard_shortcuts")?,
            artifacts: saved.get("artifacts").cloned().unwrap_or(Value::Null),
            export: saved.get("export").cloned().unwrap_or(Value::Null),
        };
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), ReadError> {
        if self.contract_version != VERSION {
            return Err(ReadError::new(
                "contract_version",
                "Unsupported version of the work settings.",
            ));
        }
        if self.project_id.is_empty() || self.project_id.contains(['/', '\\', ':']) {
            return Err(incomplete());
        }
        let root = qnc_contracts::parse_qnc_uri(&self.output_root_uri).map_err(|_| incomplete())?;
        let db = qnc_contracts::parse_qnc_uri(&self.workspace_db_uri).map_err(|_| incomplete())?;
        if root.resource_kind != "project"
            || root.resource_id != self.project_id
            || db.resource_kind != "db"
            || db.resource_id != format!("project_workspace/{}", self.project_id)
            || root.environment != db.environment
            || root.authority != db.authority
        {
            return Err(incomplete());
        }
        for value in [
            &self.storage.ingest_profile,
            &self.storage.ingest_media,
            &self.storage.proxy_policy,
            &self.storage.original_policy,
        ] {
            if value.trim().is_empty() {
                return Err(incomplete());
            }
        }
        for value in self.products.all_relative_paths() {
            validate_relative_product_path(value)?;
        }
        if let Some(prerender) = &self.products.prerender {
            validate_relative_product_path(prerender)?;
        }
        required_text(&self.input, "mode")?;
        required_text(&self.playback, "input")?;
        required_text(&self.keyboard_shortcuts, "active_preset")?;
        if self.ai.get("enabled").and_then(Value::as_bool).is_none()
            || !self.video.is_object()
            || !self.audio.is_object()
            || self
                .video
                .get("fps")
                .and_then(Value::as_f64)
                .filter(|v| v.is_finite() && *v > 0.0)
                .is_none()
            || self
                .audio
                .get("sample_rate")
                .and_then(Value::as_u64)
                .filter(|v| *v > 0)
                .is_none()
        {
            return Err(incomplete());
        }
        self.audio_channels()?;
        Ok(())
    }

    /// The keyboard preset the project chose (Project, Advanced); `None` leaves the
    /// catalog's own preset.
    pub fn keyboard_preset(&self) -> Option<&str> {
        self.keyboard_shortcuts.get("active_preset").and_then(Value::as_str)
    }

    pub fn ai_enabled(&self) -> bool {
        self.ai.get("enabled").and_then(Value::as_bool) == Some(true)
    }

    pub fn product_uri(&self, area: ProductArea) -> String {
        format!(
            "{}/{}",
            self.output_root_uri.trim_end_matches('/'),
            self.products.relative_path(area).trim_matches('/')
        )
    }

    pub fn product_local_dir(&self, project_dir: &Path, area: ProductArea) -> PathBuf {
        project_dir.join(self.products.relative_path(area))
    }

    /// Where the prerender of the story is written in the project folder, and its URI;
    /// a project made before the prerender has none (a controlled error, no default).
    pub fn prerender_location(&self, project_dir: &Path) -> Result<(PathBuf, String), ReadError> {
        let relative = self.products.prerender.as_deref().ok_or_else(|| {
            ReadError::new("products_prerender", "The project has no prerender folder (products.prerender).")
        })?;
        Ok((
            project_dir.join(relative),
            format!("{}/{}", self.output_root_uri.trim_end_matches('/'), relative.trim_matches('/')),
        ))
    }

    /// The project folder as a media source: media copied into the project (an imported
    /// original or proxy) is read through it like media on a card, by `<this>/file/<path>`.
    /// Same environment and authority as the project.
    pub fn project_media_source_uri(&self) -> Result<String, ReadError> {
        let root = qnc_contracts::parse_qnc_uri(&self.output_root_uri).map_err(|_| incomplete())?;
        let authority = root.authority.map(|a| format!("{a}/")).unwrap_or_default();
        Ok(format!(
            "qnc://{}/{authority}source/project-{}",
            root.environment, self.project_id
        ))
    }
}

impl ProductLocations {
    pub fn relative_path(&self, area: ProductArea) -> &str {
        match area {
            ProductArea::Thumbnails => &self.thumbnails,
            ProductArea::Filmstrip => &self.filmstrip,
            ProductArea::VirtualShorts => &self.virtual_shorts,
            ProductArea::VirtualSegments => &self.virtual_segments,
            ProductArea::BRollVirtualClips => &self.b_roll_virtual_clips,
        }
    }

    fn all_relative_paths(&self) -> [&str; 6] {
        [
            &self.root,
            &self.thumbnails,
            &self.filmstrip,
            &self.virtual_shorts,
            &self.virtual_segments,
            &self.b_roll_virtual_clips,
        ]
    }
}

fn incomplete() -> ReadError {
    ReadError::new(
        "incomplete_settings",
        "The database has no complete work settings. There are no substitute settings.",
    )
}

fn required_text(value: &Value, key: &str) -> Result<String, ReadError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(incomplete)
}

fn object(value: &Value, key: &str) -> Result<Value, ReadError> {
    value
        .get(key)
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(incomplete)
}

fn validate_relative_product_path(value: &str) -> Result<(), ReadError> {
    let value = value.trim();
    if value.is_empty() || value.contains('\\') || value.starts_with('/') {
        return Err(incomplete());
    }
    let path = Path::new(value);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(incomplete());
    }
    Ok(())
}

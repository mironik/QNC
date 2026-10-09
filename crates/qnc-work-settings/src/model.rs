use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Component, Path, PathBuf};

pub const VERSION: &str = "0.1.0";

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
                "Baza sadrzi nepodrzani playback.input.",
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
                "Projektne postavke nemaju valjan artifacts.filmstrip / artifacts.wave.",
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
                ReadError::new("audio_channels", "Baza sadrzi nepodrzani audio.channels.")
            })
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
            },
            input: object(&saved, "input")?,
            playback: object(&saved, "playback")?,
            video: object(&saved, "video")?,
            audio: object(&saved, "audio")?,
            ai: object(&saved, "ai")?,
            keyboard_shortcuts: object(&saved, "keyboard_shortcuts")?,
            artifacts: saved.get("artifacts").cloned().unwrap_or(Value::Null),
        };
        result.validate()?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), ReadError> {
        if self.contract_version != VERSION {
            return Err(ReadError::new(
                "contract_version",
                "Nepodrzana verzija radnih postavki.",
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
        "Baza nema potpune radne postavke. Nema zamjenskih postavki.",
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

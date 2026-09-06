use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const VERSION: &str = "0.1.0";

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
    pub input: Value,
    pub playback: Value,
    pub video: Value,
    pub audio: Value,
    pub ai: Value,
    pub keyboard_shortcuts: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoragePolicy {
    pub ingest_profile: String,
    pub ingest_media: String,
    pub proxy_policy: String,
    pub original_policy: String,
}

impl WorkSettings {
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
            input: object(&saved, "input")?,
            playback: object(&saved, "playback")?,
            video: object(&saved, "video")?,
            audio: object(&saved, "audio")?,
            ai: object(&saved, "ai")?,
            keyboard_shortcuts: object(&saved, "keyboard_shortcuts")?,
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
        Ok(())
    }

    pub fn ai_enabled(&self) -> bool {
        self.ai.get("enabled").and_then(Value::as_bool) == Some(true)
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

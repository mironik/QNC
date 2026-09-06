use qnc_contracts::parse_qnc_uri;
use qnc_transport_resolver::safe_relative_path;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
pub const CATALOG_ID: &str = "qnc.catalog.applications";
pub const SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_URI: &str = "qnc://local/catalog/applications";
pub const CATALOG_LIMIT: u64 = 8 * 1024 * 1024;

mod selection;
mod transport;
pub use selection::*;
pub use transport::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostMode {
    EmbeddedPublicApi,
    ExternalComponent,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Application {
    pub application_id: String,
    pub tab_id: String,
    pub label: String,
    pub priority_group: String,
    pub system: bool,
    pub removable: bool,
    pub host_mode: HostMode,
    pub desktop_entry: String,
    pub standalone_executable: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableReason {
    Disabled,
    MissingExecutable,
    NotExecutable,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableApplication {
    pub application_id: String,
    pub label: String,
    pub priority_group: String,
    pub reason: UnavailableReason,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub catalog_id: String,
    pub schema_version: u32,
    pub catalog_uri: String,
    pub selection_policy: String,
    pub observed_at_unix_ms: u64,
    pub target_os: String,
    pub target_cpu: String,
    pub applications: Vec<Application>,
    pub unavailable: Vec<UnavailableApplication>,
}

pub fn token(value: &str, dots: bool) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value.contains("..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || (dots && b == b'.'))
    {
        return Err(format!("Invalid neutral identifier: {value:?}").into());
    }
    Ok(())
}

pub fn identity(id: &str, label: &str, group: &str) -> Result<()> {
    token(id, true)?;
    if !id.starts_with("qnc.") || id.starts_with("qnc.module.") || id.len() <= 4 {
        return Err(format!("Not a QNC application ID: {id}").into());
    }
    if label.trim().is_empty() || label.len() > 256 || label.chars().any(char::is_control) {
        return Err(format!("Invalid application label for {id}").into());
    }
    if group.len() != 1 || !group.as_bytes()[0].is_ascii_lowercase() {
        return Err(format!("Application {id} must have priority_group a-z").into());
    }
    Ok(())
}

pub fn validate_application(app: &Application) -> Result<()> {
    identity(&app.application_id, &app.label, &app.priority_group)?;
    token(&app.tab_id, true)?;
    token(&app.desktop_entry, false)?;
    // The platform suffix belongs to the installation adapter, never the registration.
    token(&app.standalone_executable, false)?;
    if app.system && app.removable {
        return Err(format!(
            "System application cannot be removable: {}",
            app.application_id
        )
        .into());
    }
    Ok(())
}

pub fn validate_uri(uri: &str) -> Result<()> {
    if uri != uri.trim() {
        return Err("Catalog URI must not contain surrounding whitespace".into());
    }
    let parsed = parse_qnc_uri(uri)?;
    if parsed.resource_kind != "catalog" || parsed.resource_id != "applications" {
        return Err("Expected a QNC catalog/applications URI".into());
    }
    safe_relative_path(&parsed.resource_kind, &parsed.resource_id)?;
    if let Some(authority) = parsed.authority {
        token(&authority, true)?;
    }
    Ok(())
}

pub fn validate_catalog(catalog: &Catalog) -> Result<()> {
    if catalog.catalog_id != CATALOG_ID
        || catalog.schema_version != SCHEMA_VERSION
        || catalog.selection_policy != "one_per_priority_group"
    {
        return Err("Unsupported application catalog identity/schema".into());
    }
    validate_uri(&catalog.catalog_uri)?;
    if catalog.observed_at_unix_ms == 0
        || !matches!(catalog.target_os.as_str(), "windows" | "linux" | "macos")
        || !matches!(
            catalog.target_cpu.as_str(),
            "x86" | "x86_64" | "arm" | "aarch64"
        )
    {
        return Err("Invalid catalog observation/platform metadata".into());
    }
    let mut ids = HashSet::new();
    let mut tabs = HashSet::new();
    let mut previous_group = None;
    for app in &catalog.applications {
        validate_application(app)?;
        if !ids.insert(app.application_id.as_str()) || !tabs.insert(app.tab_id.as_str()) {
            return Err("Duplicate available application ID/tab".into());
        }
        if previous_group.is_some_and(|previous| previous > app.priority_group.as_str()) {
            return Err("Available applications must be in ascending group order".into());
        }
        previous_group = Some(app.priority_group.as_str());
    }
    for app in &catalog.unavailable {
        identity(&app.application_id, &app.label, &app.priority_group)?;
        if !ids.insert(app.application_id.as_str()) {
            return Err("Application appears more than once in the catalog".into());
        }
    }
    Ok(())
}

pub fn select<'a>(catalog: &'a Catalog, ids: &[&str]) -> Result<Vec<&'a Application>> {
    validate_catalog(catalog)?;
    let mut selected = Vec::new();
    let mut groups = HashSet::new();
    let mut seen_ids = HashSet::new();
    for id in ids {
        if !seen_ids.insert(*id) {
            return Err(format!("Duplicate selection: {id}").into());
        }
        let app = catalog
            .applications
            .iter()
            .find(|app| app.application_id == *id)
            .ok_or_else(|| format!("Application is not available in this catalog: {id}"))?;
        if !groups.insert(&app.priority_group) {
            return Err(format!(
                "Group {} is already selected; choose one variant only",
                app.priority_group
            )
            .into());
        }
        selected.push(app);
    }
    selected.sort_by(|a, b| a.priority_group.cmp(&b.priority_group));
    Ok(selected)
}

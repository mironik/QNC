mod local;
mod model;
pub mod server;

pub use model::{ReadError, StoragePolicy, WorkSettings, VERSION};
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

pub const ENDPOINT: &str = "/v1/work-settings/read";
pub const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderConfig {
    pub registry_uri: String,
    pub registry_file: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub token_env: Option<String>,
}

#[derive(Clone)]
pub struct SettingsReader {
    config: ReaderConfig,
}

impl std::fmt::Debug for SettingsReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingsReader").finish_non_exhaustive()
    }
}

impl SettingsReader {
    pub fn local(registry_file: impl Into<PathBuf>) -> Self {
        Self {
            config: ReaderConfig {
                registry_uri: local::LOCAL_REGISTRY_URI.into(),
                registry_file: Some(registry_file.into()),
                endpoint: None,
                token_env: None,
            },
        }
    }

    pub fn from_config(config: ReaderConfig) -> Result<Self, ReadError> {
        registry_context(&config.registry_uri)?;
        let uri = qnc_contracts::parse_qnc_uri(&config.registry_uri).map_err(|_| config_error())?;
        if uri.environment == "local" {
            if config.registry_file.is_none() || config.endpoint.is_some() {
                return Err(config_error());
            }
        } else if config.registry_file.is_some() || config.endpoint.is_none() {
            return Err(config_error());
        }
        Ok(Self { config })
    }

    pub fn from_root(root: &Path) -> Result<Self, ReadError> {
        let configured = std::env::var_os("QNC_WORK_SETTINGS_CONFIG").map(PathBuf::from);
        let path = configured
            .clone()
            .unwrap_or_else(|| root.join("data").join("work-settings-transport.json"));
        if configured.is_none() && !path.exists() {
            return Ok(Self::local(root.join("data").join("project_store.db")));
        }
        let text = std::fs::read_to_string(&path).map_err(|_| config_error())?;
        let mut config: ReaderConfig = serde_json::from_str(&text).map_err(|_| config_error())?;
        if let Some(file) = &mut config.registry_file {
            if file.is_relative() {
                *file = path.parent().ok_or_else(config_error)?.join(&*file);
            }
        }
        Self::from_config(config)
    }

    pub fn read(&self) -> Result<WorkSettings, ReadError> {
        let parsed =
            qnc_contracts::parse_qnc_uri(&self.config.registry_uri).map_err(|_| config_error())?;
        if parsed.environment == "local" {
            return local::read(
                self.config
                    .registry_file
                    .as_deref()
                    .ok_or_else(config_error)?,
                &self.config.registry_uri,
            );
        }
        let base = self.config.endpoint.as_deref().ok_or_else(config_error)?;
        let url = url::Url::parse(base).map_err(|_| config_error())?;
        let loopback = url.host_str().is_some_and(|h| {
            h == "localhost"
                || h.parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || (url.scheme() != "https" && !(url.scheme() == "http" && loopback))
        {
            return Err(ReadError::new(
                "transport_config",
                "Mrezni pristup zahtijeva HTTPS; HTTP je dopusten samo na loopback adresi.",
            ));
        }
        let authority = parsed.authority.as_deref().ok_or_else(config_error)?;
        let mut resolver = ResolverConfig::new(PathBuf::new());
        resolver = if parsed.environment == "lan" {
            resolver.with_lan_authority(authority, base)
        } else {
            resolver.with_intranet_authority(authority, base)
        };
        let ResolvedEndpoint::NetworkEndpoint { base_url, .. } = resolver
            .resolve(&self.config.registry_uri)
            .map_err(|_| config_error())?
            .endpoint
        else {
            return Err(config_error());
        };
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(10))
            .redirects(0)
            .build();
        let mut request = agent.post(&format!("{base_url}{ENDPOINT}"));
        if let Some(key) = &self.config.token_env {
            let token = std::env::var(key)
                .ok()
                .filter(|t| !t.is_empty())
                .ok_or_else(config_error)?;
            request = request.set("Authorization", &format!("Bearer {token}"));
        }
        let response = request
            .send_json(serde_json::json!({"registry_uri": self.config.registry_uri}))
            .map_err(|_| {
                ReadError::new(
                    "transport_unavailable",
                    "Transport baze radnih postavki nije dostupan ili pristup nije odobren.",
                )
            })?;
        let mut bytes = Vec::new();
        response
            .into_reader()
            .take(MAX_RESPONSE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| config_error())?;
        if bytes.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(config_error());
        }
        let reply: ReadReply = serde_json::from_slice(&bytes).map_err(|_| config_error())?;
        let result = reply.result?;
        result.validate()?;
        if !result.workspace_db_uri.starts_with(&format!(
            "{}/db/project_workspace/",
            registry_context(&self.config.registry_uri)?
        )) {
            return Err(ReadError::new(
                "wrong_authority",
                "Odgovor ne pripada trazenom transport izvoru.",
            ));
        }
        Ok(result)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadReply {
    pub result: Result<WorkSettings, ReadError>,
}

pub(crate) fn registry_context(uri: &str) -> Result<String, ReadError> {
    let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|_| config_error())?;
    if parsed.resource_kind != "db" || parsed.resource_id != "project_registry" {
        return Err(config_error());
    }
    Ok(match parsed.authority {
        Some(authority) => format!("qnc://{}/{authority}", parsed.environment),
        None => format!("qnc://{}", parsed.environment),
    })
}

fn config_error() -> ReadError {
    ReadError::new(
        "transport_config",
        "Neispravna transport konfiguracija ili odgovor baze radnih postavki.",
    )
}

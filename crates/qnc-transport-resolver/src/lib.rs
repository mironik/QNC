use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

use qnc_contracts::{parse_qnc_uri, QncUri};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedEndpoint {
    LocalPath(PathBuf),
    NetworkEndpoint {
        base_url: String,
        resource_kind: String,
        resource_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedResource {
    pub uri: String,
    pub environment: String,
    pub authority: Option<String>,
    pub endpoint: ResolvedEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    InvalidUri(String),
    MissingAuthority {
        environment: String,
        authority: String,
    },
    InvalidAuthorityBase {
        environment: String,
        authority: String,
    },
    UnsafeResourceId(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUri(error) => write!(f, "{error}"),
            Self::MissingAuthority {
                environment,
                authority,
            } => write!(f, "missing {environment} transport authority '{authority}'"),
            Self::InvalidAuthorityBase {
                environment,
                authority,
            } => write!(
                f,
                "invalid {environment} transport base for authority '{authority}'"
            ),
            Self::UnsafeResourceId(resource_id) => {
                write!(f, "unsafe QNC URI resource id '{resource_id}'")
            }
        }
    }
}

impl std::error::Error for ResolveError {}

#[derive(Debug, Clone)]
pub struct ResolverConfig {
    local_root: PathBuf,
    local_bindings: HashMap<String, PathBuf>,
    lan_authorities: HashMap<String, String>,
    intranet_authorities: HashMap<String, String>,
}

impl ResolverConfig {
    pub fn new(local_root: impl Into<PathBuf>) -> Self {
        Self {
            local_root: local_root.into(),
            local_bindings: HashMap::new(),
            lan_authorities: HashMap::new(),
            intranet_authorities: HashMap::new(),
        }
    }

    pub fn with_local_binding(mut self, uri: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        self.local_bindings.insert(uri.into(), path.into());
        self
    }

    pub fn with_lan_authority(
        mut self,
        authority: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        self.lan_authorities
            .insert(authority.into(), base_url.into());
        self
    }

    pub fn with_intranet_authority(
        mut self,
        authority: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        self.intranet_authorities
            .insert(authority.into(), base_url.into());
        self
    }

    pub fn resolve(&self, uri: &str) -> Result<ResolvedResource, ResolveError> {
        let parsed = parse_qnc_uri(uri).map_err(ResolveError::InvalidUri)?;
        match parsed.environment.as_str() {
            "local" => self.resolve_local(uri, parsed),
            "lan" => self.resolve_network(uri, parsed, &self.lan_authorities),
            "intranet" => self.resolve_network(uri, parsed, &self.intranet_authorities),
            _ => Err(ResolveError::InvalidUri(format!(
                "unsupported QNC environment '{}'",
                parsed.environment
            ))),
        }
    }

    fn resolve_local(&self, uri: &str, parsed: QncUri) -> Result<ResolvedResource, ResolveError> {
        if let Some(path) = self.local_bindings.get(uri) {
            return Ok(ResolvedResource {
                uri: uri.to_string(),
                environment: parsed.environment,
                authority: None,
                endpoint: ResolvedEndpoint::LocalPath(path.clone()),
            });
        }

        let relative = safe_relative_path(&parsed.resource_kind, &parsed.resource_id)?;
        Ok(ResolvedResource {
            uri: uri.to_string(),
            environment: parsed.environment,
            authority: None,
            endpoint: ResolvedEndpoint::LocalPath(self.local_root.join(relative)),
        })
    }

    fn resolve_network(
        &self,
        uri: &str,
        parsed: QncUri,
        authorities: &HashMap<String, String>,
    ) -> Result<ResolvedResource, ResolveError> {
        let authority = parsed.authority.clone().ok_or_else(|| {
            ResolveError::InvalidUri(format!("{} URI requires authority", parsed.environment))
        })?;
        let base_url =
            authorities
                .get(&authority)
                .cloned()
                .ok_or_else(|| ResolveError::MissingAuthority {
                    environment: parsed.environment.clone(),
                    authority: authority.clone(),
                })?;
        if !is_valid_network_base(&base_url) {
            return Err(ResolveError::InvalidAuthorityBase {
                environment: parsed.environment,
                authority,
            });
        }

        Ok(ResolvedResource {
            uri: uri.to_string(),
            environment: parsed.environment,
            authority: Some(authority),
            endpoint: ResolvedEndpoint::NetworkEndpoint {
                base_url: base_url.trim_end_matches('/').to_string(),
                resource_kind: parsed.resource_kind,
                resource_id: parsed.resource_id,
            },
        })
    }
}

pub fn safe_relative_path(resource_kind: &str, resource_id: &str) -> Result<PathBuf, ResolveError> {
    let mut path = PathBuf::new();
    push_safe_segment(&mut path, resource_kind)?;
    for segment in resource_id.split('/') {
        push_safe_segment(&mut path, segment)?;
    }
    Ok(path)
}

fn push_safe_segment(path: &mut PathBuf, segment: &str) -> Result<(), ResolveError> {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.contains('\\')
        || segment.contains(':')
        || Path::new(segment).is_absolute()
    {
        return Err(ResolveError::UnsafeResourceId(segment.to_string()));
    }
    path.push(segment);
    Ok(())
}

fn is_valid_network_base(base_url: &str) -> bool {
    let lower = base_url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_local_qnc_uri_to_process_local_path() {
        let config = ResolverConfig::new(PathBuf::from("qnc-data"));
        let resolved = config
            .resolve("qnc://local/db/project_registry")
            .expect("resolve local db");

        assert_eq!(resolved.environment, "local");
        assert_eq!(resolved.authority, None);
        assert_eq!(
            resolved.endpoint,
            ResolvedEndpoint::LocalPath(
                PathBuf::from("qnc-data")
                    .join("db")
                    .join("project_registry")
            )
        );
    }

    #[test]
    fn resolves_bound_local_qnc_uri_to_private_path() {
        let config = ResolverConfig::new(PathBuf::from("qnc-data")).with_local_binding(
            "qnc://local/db/project_registry",
            PathBuf::from("data").join("project_store.db"),
        );
        let resolved = config
            .resolve("qnc://local/db/project_registry")
            .expect("resolve local db binding");

        assert_eq!(
            resolved.endpoint,
            ResolvedEndpoint::LocalPath(PathBuf::from("data").join("project_store.db"))
        );
    }

    #[test]
    fn rejects_raw_os_paths_as_public_input() {
        let config = ResolverConfig::new(PathBuf::from("qnc-data"));
        assert!(config.resolve(r"C:\media\clip001.mp4").is_err());
        assert!(config.resolve("/mnt/media/clip001.mp4").is_err());
    }

    #[test]
    fn resolves_lan_uri_through_registered_authority() {
        let config = ResolverConfig::new(PathBuf::from("qnc-data"))
            .with_lan_authority("storage-a", "http://storage-a.local/qnc");
        let resolved = config
            .resolve("qnc://lan/storage-a/media/source_123/clip_456/original")
            .expect("resolve lan media");

        assert_eq!(resolved.environment, "lan");
        assert_eq!(resolved.authority.as_deref(), Some("storage-a"));
        assert_eq!(
            resolved.endpoint,
            ResolvedEndpoint::NetworkEndpoint {
                base_url: "http://storage-a.local/qnc".to_string(),
                resource_kind: "media".to_string(),
                resource_id: "source_123/clip_456/original".to_string(),
            }
        );
    }

    #[test]
    fn lan_and_intranet_require_registered_authority() {
        let config = ResolverConfig::new(PathBuf::from("qnc-data"));
        assert!(matches!(
            config.resolve("qnc://lan/storage-a/media/clip_1"),
            Err(ResolveError::MissingAuthority { .. })
        ));
        assert!(matches!(
            config.resolve("qnc://intranet/mam-a/db/story/news_story_001"),
            Err(ResolveError::MissingAuthority { .. })
        ));
    }

    #[test]
    fn rejects_path_traversal_in_resource_ids() {
        assert!(safe_relative_path("db", "../project_registry").is_err());
        assert!(safe_relative_path("db", "project_registry").is_ok());
    }
}

//! Read-only source bindings: which QNC source URI is served from which local
//! root or network authority. They come from the host transport configuration
//! (the same file the owner of the sources uses), never from a database, and
//! this module changes nothing: it does not scan, probe or write.
//!
//! A binding is private storage information. It is only meant to be handed to a
//! player launch; it is not a public identity of a source.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use serde::Deserialize;

pub const MODULE_ID: &str = "qnc.module.source-bindings";
pub const VERSION: &str = "0.1.0";

const CONFIG_ENV: &str = "QNC_INGEST_TRANSPORT_CONFIG";
const CONFIG_FILE: &str = "ingest-transport.json";
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_SOURCES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBinding {
    pub uri: String,
    /// Absolute local root, when the source is local.
    pub file: Option<PathBuf>,
    /// Network authority URL, when the source is served over LAN or intranet.
    pub endpoint: Option<String>,
    /// Name of the environment variable that holds the network credential.
    pub token_env: Option<String>,
}

impl SourceBinding {
    /// The network credential, read from the environment at the moment of use.
    pub fn token(&self) -> Result<Option<String>, String> {
        self.token_env
            .as_ref()
            .map(|name| {
                std::env::var(name).map_err(|_| "transport credential is unavailable".to_string())
            })
            .transpose()
    }
}

#[derive(Deserialize)]
struct Config {
    version: String,
    #[serde(default)]
    sources: Vec<ConfigSource>,
}

#[derive(Deserialize)]
struct ConfigSource {
    location: ConfigLocation,
}

#[derive(Deserialize)]
struct ConfigLocation {
    uri: String,
    file: Option<PathBuf>,
    endpoint: Option<String>,
    token_env: Option<String>,
}

/// Loads the bindings from the host transport configuration under `root`
/// (or from the file named by `QNC_INGEST_TRANSPORT_CONFIG`).
pub fn load(root: &Path) -> Result<Vec<SourceBinding>, String> {
    let file = std::env::var_os(CONFIG_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("data").join(CONFIG_FILE));
    let file = file
        .canonicalize()
        .map_err(|_| "Nedostaje konfiguracija izvora medija (ingest-transport.json).".to_string())?;
    let metadata = std::fs::metadata(&file).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err("Konfiguracija izvora medija je prevelika.".to_string());
    }
    let bytes = std::fs::read(&file).map_err(|error| error.to_string())?;
    let base = file.parent().ok_or("Konfiguracija izvora nema direktorij.")?;
    parse(&bytes, base)
}

fn parse(bytes: &[u8], base: &Path) -> Result<Vec<SourceBinding>, String> {
    let config: Config = serde_json::from_slice(bytes)
        .map_err(|error| format!("Konfiguracija izvora medija nije valjana: {error}"))?;
    if config.version != "0.1.0" || config.sources.len() > MAX_SOURCES {
        return Err("Konfiguracija izvora medija nije podrzana.".to_string());
    }
    let mut seen = BTreeSet::new();
    let mut bindings = Vec::new();
    for source in config.sources {
        let location = source.location;
        if !seen.insert(location.uri.clone()) {
            return Err(format!("Izvor {} je naveden dvaput.", location.uri));
        }
        let file = location.file.map(|path| {
            if path.is_relative() {
                base.join(path)
            } else {
                path
            }
        });
        // A binding is either a local root or a network authority, never both.
        match (&file, &location.endpoint) {
            (Some(_), None) | (None, Some(_)) => {}
            _ => return Err(format!("Izvor {} nema jedinstvenu vezu.", location.uri)),
        }
        bindings.push(SourceBinding {
            uri: location.uri,
            file,
            endpoint: location.endpoint,
            token_env: location.token_env,
        });
    }
    Ok(bindings)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "C:/qnc/data";

    #[test]
    fn local_and_network_sources_are_read_and_relative_roots_are_anchored() {
        let bindings = parse(
            br#"{"version":"0.1.0","parallelism":8,"sources":[
                {"location":{"uri":"qnc://local/source/a","file":"cards/a"},"name":"A","probe":{}},
                {"location":{"uri":"qnc://lan/nas/source/b","endpoint":"http://nas.local/qnc","token_env":"QNC_TOKEN"},"name":"B"}
            ]}"#,
            Path::new(BASE),
        )
        .unwrap();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].file, Some(Path::new(BASE).join("cards/a")));
        assert_eq!(bindings[1].endpoint.as_deref(), Some("http://nas.local/qnc"));
        assert_eq!(bindings[1].token_env.as_deref(), Some("QNC_TOKEN"));
    }

    #[test]
    fn duplicate_or_ambiguous_sources_are_rejected() {
        let duplicate = br#"{"version":"0.1.0","sources":[
            {"location":{"uri":"qnc://local/source/a","file":"x"}},
            {"location":{"uri":"qnc://local/source/a","file":"y"}}]}"#;
        assert!(parse(duplicate, Path::new(BASE)).is_err());
        let both = br#"{"version":"0.1.0","sources":[
            {"location":{"uri":"qnc://local/source/a","file":"x","endpoint":"http://h"}}]}"#;
        assert!(parse(both, Path::new(BASE)).is_err());
        let neither = br#"{"version":"0.1.0","sources":[{"location":{"uri":"qnc://local/source/a"}}]}"#;
        assert!(parse(neither, Path::new(BASE)).is_err());
    }

    #[test]
    fn unknown_version_is_rejected() {
        assert!(parse(br#"{"version":"9.9.9","sources":[]}"#, Path::new(BASE)).is_err());
    }

    #[test]
    fn missing_credential_is_a_controlled_error() {
        let binding = SourceBinding {
            uri: "qnc://lan/x/source/y".into(),
            file: None,
            endpoint: Some("http://h".into()),
            token_env: Some("QNC_SURELY_UNSET_TOKEN_VARIABLE".into()),
        };
        assert!(binding.token().is_err());
    }
}

//! Read-only transport bindings from the host configuration: which QNC source
//! URI is served from which local root or network authority, and where the media
//! records database is. They come from the host transport configuration (the
//! same file the owner of the sources uses), never from a database, and this
//! module changes nothing: it does not scan, probe or write.
//!
//! A binding is private storage information. It is only meant to be handed to a
//! resolver or a player launch; it is not the public identity of a source.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use qnc_transport_resolver::ResolverConfig;
use serde::Deserialize;

pub const MODULE_ID: &str = "qnc.module.source-bindings";
pub const VERSION: &str = "0.2.0";

const CONFIG_ENV: &str = "QNC_INGEST_TRANSPORT_CONFIG";
const CONFIG_FILE: &str = "ingest-transport.json";
const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MAX_SOURCES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceBinding {
    pub uri: String,
    /// Absolute local root or file, when the binding is local.
    pub file: Option<PathBuf>,
    /// Network authority URL, when the binding is served over LAN or intranet.
    pub endpoint: Option<String>,
    /// Name of the environment variable that holds the network credential.
    pub token_env: Option<String>,
}

impl SourceBinding {
    /// Resolver for this binding: a local path, or a LAN / intranet authority.
    pub fn resolver(&self) -> Result<ResolverConfig, String> {
        let parsed = qnc_contracts::parse_qnc_uri(&self.uri)?;
        let resolver = ResolverConfig::new(PathBuf::new());
        match (parsed.environment.as_str(), &self.file, &self.endpoint) {
            ("local" | "lan" | "intranet", Some(file), None)
                if file.is_absolute() && self.token_env.is_none() =>
            {
                Ok(resolver.with_local_binding(&self.uri, file))
            }
            ("lan", None, Some(url)) => Ok(resolver.with_lan_authority(
                parsed.authority.ok_or("missing authority")?,
                url,
            )),
            ("intranet", None, Some(url)) => Ok(resolver.with_intranet_authority(
                parsed.authority.ok_or("missing authority")?,
                url,
            )),
            _ => Err("invalid transport binding".to_string()),
        }
    }

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

/// What the host transport configuration binds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportBindings {
    pub sources: Vec<SourceBinding>,
    pub media_records: Option<SourceBinding>,
}

#[derive(Deserialize)]
struct Config {
    version: String,
    #[serde(default)]
    sources: Vec<ConfigSource>,
    media_records: Option<ConfigLocation>,
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
pub fn load(root: &Path) -> Result<TransportBindings, String> {
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

fn binding(location: ConfigLocation, base: &Path) -> Result<SourceBinding, String> {
    let file = location.file.map(|path| {
        if path.is_relative() {
            base.join(path)
        } else {
            path
        }
    });
    // A binding is either a local path or a network authority, never both.
    match (&file, &location.endpoint) {
        (Some(_), None) | (None, Some(_)) => {}
        _ => return Err(format!("Veza {} nije jedinstvena.", location.uri)),
    }
    Ok(SourceBinding {
        uri: location.uri,
        file,
        endpoint: location.endpoint,
        token_env: location.token_env,
    })
}

fn parse(bytes: &[u8], base: &Path) -> Result<TransportBindings, String> {
    let config: Config = serde_json::from_slice(bytes)
        .map_err(|error| format!("Konfiguracija izvora medija nije valjana: {error}"))?;
    if config.version != "0.1.0" || config.sources.len() > MAX_SOURCES {
        return Err("Konfiguracija izvora medija nije podrzana.".to_string());
    }
    let mut seen = BTreeSet::new();
    let mut sources = Vec::new();
    for source in config.sources {
        if !seen.insert(source.location.uri.clone()) {
            return Err(format!("Izvor {} je naveden dvaput.", source.location.uri));
        }
        sources.push(binding(source.location, base)?);
    }
    let media_records = config
        .media_records
        .map(|location| binding(location, base))
        .transpose()?;
    Ok(TransportBindings {
        sources,
        media_records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = "C:/qnc/data";

    #[test]
    fn local_and_network_sources_and_records_are_read_and_relative_paths_anchored() {
        let bindings = parse(
            br#"{"version":"0.1.0","parallelism":8,
                "media_records":{"uri":"qnc://local/db/media_records","file":"records.db"},
                "sources":[
                {"location":{"uri":"qnc://local/source/a","file":"cards/a"},"name":"A","probe":{}},
                {"location":{"uri":"qnc://lan/nas/source/b","endpoint":"http://nas.local/qnc","token_env":"QNC_TOKEN"},"name":"B"}
            ]}"#,
            Path::new(BASE),
        )
        .unwrap();
        assert_eq!(bindings.sources.len(), 2);
        assert_eq!(bindings.sources[0].file, Some(Path::new(BASE).join("cards/a")));
        assert_eq!(
            bindings.sources[1].endpoint.as_deref(),
            Some("http://nas.local/qnc")
        );
        assert_eq!(bindings.sources[1].token_env.as_deref(), Some("QNC_TOKEN"));
        let records = bindings.media_records.unwrap();
        assert_eq!(records.uri, "qnc://local/db/media_records");
        assert_eq!(records.file, Some(Path::new(BASE).join("records.db")));
    }

    #[test]
    fn media_records_binding_is_optional() {
        let bindings = parse(br#"{"version":"0.1.0","sources":[]}"#, Path::new(BASE)).unwrap();
        assert!(bindings.media_records.is_none());
    }

    #[test]
    fn duplicate_or_ambiguous_bindings_are_rejected() {
        let duplicate = br#"{"version":"0.1.0","sources":[
            {"location":{"uri":"qnc://local/source/a","file":"x"}},
            {"location":{"uri":"qnc://local/source/a","file":"y"}}]}"#;
        assert!(parse(duplicate, Path::new(BASE)).is_err());
        let both = br#"{"version":"0.1.0","sources":[
            {"location":{"uri":"qnc://local/source/a","file":"x","endpoint":"http://h"}}]}"#;
        assert!(parse(both, Path::new(BASE)).is_err());
        let neither = br#"{"version":"0.1.0","sources":[{"location":{"uri":"qnc://local/source/a"}}]}"#;
        assert!(parse(neither, Path::new(BASE)).is_err());
        let ambiguous_records = br#"{"version":"0.1.0","media_records":{"uri":"qnc://local/db/media_records"}}"#;
        assert!(parse(ambiguous_records, Path::new(BASE)).is_err());
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

    #[test]
    fn the_real_host_configuration_shape_is_accepted() {
        // Same shape as data/ingest-transport.json: extra fields are ignored.
        let bindings = parse(
            br#"{"version":"0.1.0","parallelism":8,
                "catalog":{"uri":"qnc://local/catalog/x","file":"../c.sqlite"},
                "source_index":{"uri":"qnc://local/db/source_index","file":"ingest_source_index.db"},
                "media_records":{"uri":"qnc://local/db/media_records","file":"ingest_media_records.db"},
                "sources":[{"location":{"uri":"qnc://local/source/volume-de666c9f","file":"G:/"},
                    "name":"G:","serial_number":"de666c9f","volume_name":"","scope":"card_relative",
                    "probe":{"kind":"local","executable":"C:/ffmpeg.exe","probe_size_bytes":1}}]}"#,
            Path::new(BASE),
        )
        .unwrap();
        assert_eq!(bindings.sources[0].uri, "qnc://local/source/volume-de666c9f");
        assert_eq!(bindings.sources[0].file, Some(PathBuf::from("G:/")));
        assert!(bindings.media_records.is_some());
    }
}

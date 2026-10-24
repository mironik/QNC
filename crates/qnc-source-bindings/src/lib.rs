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
            ("lan", None, Some(url)) => {
                Ok(resolver.with_lan_authority(parsed.authority.ok_or("missing authority")?, url))
            }
            ("intranet", None, Some(url)) => {
                Ok(resolver
                    .with_intranet_authority(parsed.authority.ok_or("missing authority")?, url))
            }
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
    pub config_version: String,
    pub catalog: Option<SourceBinding>,
    pub source_index: Option<SourceBinding>,
    pub sources: Vec<SourceBinding>,
    pub registered_sources: Vec<RegisteredSource>,
    pub media_records: Option<SourceBinding>,
    pub parallelism: Option<usize>,
}

impl TransportBindings {
    /// Readers of the sources this computer can open now (an absent or unreadable one is
    /// left out); one place for every caller (the Story application had its own copy).
    pub fn readers(&self) -> Vec<qnc_source_reader::SourceReader> {
        self.sources.iter().filter_map(SourceBinding::reader).collect()
    }
}

impl SourceBinding {
    /// A reader of this source: local when it has a file here, else remote.
    pub fn reader(&self) -> Option<qnc_source_reader::SourceReader> {
        self.resolver().ok()?;
        if let Some(path) = &self.file {
            qnc_source_reader::SourceReader::local(&self.uri, path).ok()
        } else {
            let token = self.token().ok()??;
            qnc_source_reader::SourceReader::remote(&self.uri, self.endpoint.as_deref()?, &token).ok()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSource {
    pub location: SourceBinding,
    pub name: Option<String>,
    pub serial_number: Option<String>,
    pub volume_name: Option<String>,
    pub scope: Option<String>,
    pub probe: Option<ProbeBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeBinding {
    Local {
        executable: PathBuf,
        probe_size_bytes: u64,
        analyze_duration_us: u64,
    },
    Remote {
        binding: SourceBinding,
    },
}

#[derive(Deserialize)]
struct Config {
    version: String,
    catalog: Option<ConfigLocation>,
    source_index: Option<ConfigLocation>,
    #[serde(default)]
    sources: Vec<ConfigSource>,
    media_records: Option<ConfigLocation>,
    parallelism: Option<usize>,
}

#[derive(Deserialize)]
struct ConfigSource {
    location: ConfigLocation,
    name: Option<String>,
    serial_number: Option<String>,
    volume_name: Option<String>,
    scope: Option<String>,
    probe: Option<ConfigProbe>,
}

#[derive(Deserialize)]
struct ConfigLocation {
    uri: String,
    file: Option<PathBuf>,
    endpoint: Option<String>,
    token_env: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ConfigProbe {
    Local {
        executable: PathBuf,
        probe_size_bytes: u64,
        analyze_duration_us: u64,
    },
    Remote {
        binding: ConfigLocation,
    },
}

/// Loads the bindings from the host transport configuration under `root`
/// (or from the file named by `QNC_INGEST_TRANSPORT_CONFIG`).
pub fn load(root: &Path) -> Result<TransportBindings, String> {
    let file = std::env::var_os(CONFIG_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("data").join(CONFIG_FILE));
    let file = file.canonicalize().map_err(|_| {
        qnc_ui_text::text("source.media_source_configuration_ingest_transport_json").to_string()
    })?;
    let metadata = std::fs::metadata(&file).map_err(|error| error.to_string())?;
    if metadata.len() > MAX_CONFIG_BYTES {
        return Err(qnc_ui_text::text("source.media_source_configuration_too_large").to_string());
    }
    let bytes = std::fs::read(&file).map_err(|error| error.to_string())?;
    let base = file
        .parent()
        .ok_or(qnc_ui_text::text("source.source_configuration_folder"))?;
    let mut bindings = parse(&bytes, base)?;
    with_attached(&mut bindings, &qnc_source_volumes::mounted());
    Ok(bindings)
}

/// The disks attached now are sources without being added by hand (user 2026-10-16): a
/// registered disk is served from the letter it has now, an unregistered one becomes
/// `volume-<serial>` with the probe of the local sources. Nothing is removed and the
/// configuration file is not written.
pub fn with_attached(bindings: &mut TransportBindings, volumes: &[qnc_source_volumes::Volume]) {
    let probe = bindings
        .registered_sources
        .iter()
        .find(|source| source.location.uri.starts_with("qnc://local/") && source.probe.is_some())
        .and_then(|source| source.probe.clone());
    for volume in volumes {
        let serial = volume.serial_number.to_ascii_lowercase();
        let known = bindings.registered_sources.iter().position(|source| {
            source.serial_number.as_deref().is_some_and(|known| known.eq_ignore_ascii_case(&serial)) || source.location.uri == volume.source_uri()
        });
        match known {
            Some(index) => {
                let source = &mut bindings.registered_sources[index];
                if source.location.file.as_ref().is_some_and(|file| file != &volume.root) && source.location.endpoint.is_none() {
                    let uri = source.location.uri.clone();
                    source.location.file = Some(volume.root.clone());
                    source.name = Some(volume_label(volume));
                    for binding in bindings.sources.iter_mut().filter(|binding| binding.uri == uri) {
                        binding.file = Some(volume.root.clone());
                    }
                }
            }
            None => {
                let location = SourceBinding { uri: volume.source_uri(), file: Some(volume.root.clone()), endpoint: None, token_env: None };
                bindings.sources.push(location.clone());
                bindings.registered_sources.push(RegisteredSource {
                    location,
                    name: Some(volume_label(volume)),
                    serial_number: Some(serial),
                    volume_name: volume.volume_name.clone(),
                    scope: Some("card_relative".into()),
                    probe: probe.clone(),
                });
            }
        }
    }
}

/// Sources read once, with the disks attached now added (user 2026-10-24: a card that
/// vanished for a moment while a project opened stayed missing for that form until the
/// project was opened again). A disk already among them is kept as it is.
pub fn with_mounted(sources: &[SourceBinding]) -> Vec<SourceBinding> {
    with_volumes(sources, &qnc_source_volumes::mounted())
}

fn with_volumes(sources: &[SourceBinding], volumes: &[qnc_source_volumes::Volume]) -> Vec<SourceBinding> {
    let mut all = sources.to_vec();
    for volume in volumes {
        let uri = volume.source_uri();
        if !all.iter().any(|source| source.uri == uri) {
            all.push(SourceBinding { uri, file: Some(volume.root.clone()), endpoint: None, token_env: None });
        }
    }
    all
}

/// `E:` as the source's name.
fn volume_label(volume: &qnc_source_volumes::Volume) -> String {
    volume.root.to_string_lossy().trim_end_matches(['\\', '/']).to_string()
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
        _ => return Err(qnc_ui_text::fill("source.binding_not_unique", &[("uri", &location.uri)])),
    }
    Ok(SourceBinding {
        uri: location.uri,
        file,
        endpoint: location.endpoint,
        token_env: location.token_env,
    })
}

fn probe(probe: ConfigProbe, base: &Path) -> Result<ProbeBinding, String> {
    match probe {
        ConfigProbe::Local {
            mut executable,
            probe_size_bytes,
            analyze_duration_us,
        } => {
            if executable.is_relative() {
                executable = base.join(executable);
            }
            Ok(ProbeBinding::Local {
                executable,
                probe_size_bytes,
                analyze_duration_us,
            })
        }
        ConfigProbe::Remote { binding: remote } => Ok(ProbeBinding::Remote {
            binding: binding(remote, base)?,
        }),
    }
}

fn parse(bytes: &[u8], base: &Path) -> Result<TransportBindings, String> {
    let config: Config = serde_json::from_slice(bytes)
        .map_err(|error| qnc_ui_text::fill("source.config_invalid", &[("error", &error.to_string())]))?;
    if config.version != "0.1.0" || config.sources.len() > MAX_SOURCES {
        return Err(qnc_ui_text::text("source.media_source_configuration_supported").to_string());
    }
    let mut seen = BTreeSet::new();
    let mut sources = Vec::new();
    let mut registered_sources = Vec::new();
    for source in config.sources {
        if !seen.insert(source.location.uri.clone()) {
            return Err(qnc_ui_text::fill("source.listed_twice", &[("uri", &source.location.uri)]));
        }
        let location = binding(source.location, base)?;
        sources.push(location.clone());
        registered_sources.push(RegisteredSource {
            location,
            name: source.name,
            serial_number: source.serial_number,
            volume_name: source.volume_name,
            scope: source.scope,
            probe: source.probe.map(|config| probe(config, base)).transpose()?,
        });
    }
    let catalog = config
        .catalog
        .map(|location| binding(location, base))
        .transpose()?;
    let source_index = config
        .source_index
        .map(|location| binding(location, base))
        .transpose()?;
    let media_records = config
        .media_records
        .map(|location| binding(location, base))
        .transpose()?;
    Ok(TransportBindings {
        config_version: config.version,
        catalog,
        source_index,
        sources,
        registered_sources,
        media_records,
        parallelism: config.parallelism,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_attached_disk_is_a_source_and_a_known_one_follows_its_letter() {
        let config = br#"{"version":"0.1.0","sources":[
            {"location":{"uri":"qnc://local/source/volume-de666c9f","file":"G:/"},"name":"G:","serial_number":"de666c9f",
             "probe":{"kind":"local","executable":"C:/ffprobe.exe","probe_size_bytes":8,"analyze_duration_us":1}}]}"#;
        let mut bindings = parse(config, Path::new(BASE)).unwrap();
        let volume = |root: &str, serial: &str| qnc_source_volumes::Volume { root: PathBuf::from(root), serial_number: serial.into(), volume_name: None };
        with_attached(&mut bindings, &[volume("F:\\", "DE666C9F"), volume("E:\\", "7e80f1fb")]);
        assert_eq!(bindings.sources.len(), 2, "nothing removed, the new disk added");
        assert_eq!(bindings.sources[0].file.as_deref(), Some(Path::new("F:\\")), "the card moved to F:");
        let new = &bindings.registered_sources[1];
        assert_eq!(new.location.uri, "qnc://local/source/volume-7e80f1fb");
        assert_eq!((new.name.as_deref(), new.serial_number.as_deref()), (Some("E:"), Some("7e80f1fb")));
        assert!(new.probe.is_some(), "the probe of the local sources");
    }

    #[test]
    fn a_disk_attached_after_the_sources_were_read_is_added_and_a_known_one_kept() {
        let volume = |root: &str, serial: &str| qnc_source_volumes::Volume { root: PathBuf::from(root), serial_number: serial.into(), volume_name: None };
        let read = vec![SourceBinding { uri: "qnc://local/source/volume-0e1dd2b8".into(), file: Some(PathBuf::from("C:/")), endpoint: None, token_env: None }];
        let all = with_volumes(&read, &[volume("C:\\", "0e1dd2b8"), volume("D:\\", "a8155186")]);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].file.as_deref(), Some(Path::new("C:/")), "the one already read is kept");
        assert_eq!(all[1].uri, "qnc://local/source/volume-a8155186");
        assert_eq!(all[1].file.as_deref(), Some(Path::new("D:\\")));
    }

    const BASE: &str = "C:/qnc/data";

    #[test]
    fn local_and_network_sources_and_records_are_read_and_relative_paths_anchored() {
        let bindings = parse(
            br#"{"version":"0.1.0","parallelism":8,
                "media_records":{"uri":"qnc://local/db/media_records","file":"records.db"},
                "sources":[
                {"location":{"uri":"qnc://local/source/a","file":"cards/a"},"name":"A",
                    "probe":{"kind":"local","executable":"ffprobe","probe_size_bytes":1,"analyze_duration_us":1}},
                {"location":{"uri":"qnc://lan/nas/source/b","endpoint":"http://nas.local/qnc","token_env":"QNC_TOKEN"},"name":"B"}
            ]}"#,
            Path::new(BASE),
        )
        .unwrap();
        assert_eq!(bindings.sources.len(), 2);
        assert_eq!(
            bindings.sources[0].file,
            Some(Path::new(BASE).join("cards/a"))
        );
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
        assert!(bindings.catalog.is_none());
        assert!(bindings.source_index.is_none());
        assert!(bindings.parallelism.is_none());
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
        let neither =
            br#"{"version":"0.1.0","sources":[{"location":{"uri":"qnc://local/source/a"}}]}"#;
        assert!(parse(neither, Path::new(BASE)).is_err());
        let ambiguous_records =
            br#"{"version":"0.1.0","media_records":{"uri":"qnc://local/db/media_records"}}"#;
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
                    "probe":{"kind":"local","executable":"C:/ffmpeg.exe","probe_size_bytes":1,"analyze_duration_us":1}}]}"#,
            Path::new(BASE),
        )
        .unwrap();
        assert_eq!(
            bindings.sources[0].uri,
            "qnc://local/source/volume-de666c9f"
        );
        assert_eq!(bindings.catalog.unwrap().uri, "qnc://local/catalog/x");
        assert_eq!(
            bindings.source_index.unwrap().file,
            Some(Path::new(BASE).join("ingest_source_index.db"))
        );
        assert_eq!(bindings.parallelism, Some(8));
        assert_eq!(bindings.registered_sources[0].name.as_deref(), Some("G:"));
        assert_eq!(
            bindings.registered_sources[0].scope.as_deref(),
            Some("card_relative")
        );
        assert!(matches!(
            bindings.registered_sources[0].probe,
            Some(ProbeBinding::Local { .. })
        ));
        assert_eq!(bindings.sources[0].file, Some(PathBuf::from("G:/")));
        assert!(bindings.media_records.is_some());
    }
}

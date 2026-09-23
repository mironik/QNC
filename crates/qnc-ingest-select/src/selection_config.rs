use qnc_dir_browser::{BrowserEntry, BrowserSource, TransportBrowserSession};
use qnc_media_probe::{Binding as MediaBinding, Executor, OwnerConfig, ProbeBackend};
use qnc_source_bindings::{ProbeBinding, RegisteredSource, SourceBinding, TransportBindings};
use qnc_source_reader::{SourceReader, SourceReference};
use qnc_transport_resolver::ResolverConfig;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub uri: String,
    pub file: Option<PathBuf>,
    pub endpoint: Option<String>,
    pub token_env: Option<String>,
}
impl Binding {
    pub fn resolver(&self) -> Result<ResolverConfig> {
        let p = qnc_contracts::parse_qnc_uri(&self.uri)?;
        let r = ResolverConfig::new(PathBuf::new());
        match (p.environment.as_str(), &self.file, &self.endpoint) {
            ("local" | "lan" | "intranet", Some(file), None)
                if file.is_absolute() && self.token_env.is_none() =>
            {
                Ok(r.with_local_binding(&self.uri, file))
            }
            ("lan", None, Some(url)) => {
                Ok(r.with_lan_authority(p.authority.ok_or("missing authority")?, url))
            }
            ("intranet", None, Some(url)) => {
                Ok(r.with_intranet_authority(p.authority.ok_or("missing authority")?, url))
            }
            _ => Err("invalid owner transport binding".into()),
        }
    }
    pub fn token(&self) -> Result<Option<String>> {
        self.token_env
            .as_ref()
            .map(|name| {
                std::env::var(name).map_err(|_| "transport credential is unavailable".into())
            })
            .transpose()
    }
    pub fn source(&self) -> Result<SourceReader> {
        self.resolver()?;
        if let Some(path) = &self.file {
            Ok(SourceReader::local(&self.uri, path)?)
        } else {
            Ok(SourceReader::remote(
                &self.uri,
                self.endpoint.as_deref().ok_or("source endpoint missing")?,
                self.token()?
                    .as_deref()
                    .ok_or("source credential missing")?,
            )?)
        }
    }
    pub fn media_db(&self) -> Result<qnc_media_record_db::Client> {
        let r = self.resolver()?;
        if self.file.is_some() {
            Ok(qnc_media_record_db::Client::create_local(&r, &self.uri)?)
        } else {
            Ok(qnc_media_record_db::Client::open(
                &r,
                &self.uri,
                qnc_media_record_db::Access::ReadWrite,
                self.token()?.as_deref(),
            )?)
        }
    }
    pub fn source_db(&self) -> Result<qnc_source_index_db::Client> {
        let r = self.resolver()?;
        if self.file.is_some() {
            Ok(qnc_source_index_db::Client::create_local(&r, &self.uri)?)
        } else {
            Ok(qnc_source_index_db::Client::open(
                &r,
                &self.uri,
                qnc_source_index_db::Access::ReadWrite,
                self.token()?.as_deref(),
            )?)
        }
    }
}

impl From<SourceBinding> for Binding {
    fn from(binding: SourceBinding) -> Self {
        Self {
            uri: binding.uri,
            file: binding.file,
            endpoint: binding.endpoint,
            token_env: binding.token_env,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProbeConfig {
    Local {
        executable: PathBuf,
        probe_size_bytes: u64,
        analyze_duration_us: u64,
    },
    Remote {
        binding: Binding,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceConfig {
    pub location: Binding,
    pub name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub scope: qnc_camera_detector::SourceScope,
    pub probe: ProbeConfig,
}
impl SourceConfig {
    pub fn reader(&self) -> Result<SourceReader> {
        if let Some(path) = &self.location.file {
            qnc_dir_browser::verify_local_volume_serial(path, &self.serial_number)?;
        }
        self.location.source()
    }

    pub fn local_media_path(&self, media_uri: &str) -> Result<Option<PathBuf>> {
        let Some(root) = &self.location.file else {
            return Ok(None);
        };
        qnc_dir_browser::verify_local_volume_serial(root, &self.serial_number)?;
        let reference = SourceReference::from_uri(media_uri)?;
        if reference.source_uri() != self.location.uri {
            return Err("media source mismatch".into());
        }
        let root = root.canonicalize()?;
        let path = root.join(reference.relative_path()).canonicalize()?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err("media escapes source binding".into());
        }
        Ok(Some(path))
    }

    pub fn backend(&self, media: &[SourceReference]) -> Result<Box<dyn ProbeBackend + Send>> {
        match &self.probe {
            ProbeConfig::Remote { binding } => Ok(Box::new(qnc_media_probe::Client::connect(
                &binding.resolver()?,
                &binding.uri,
                binding
                    .token()?
                    .as_deref()
                    .ok_or("probe credential missing")?,
            )?)),
            ProbeConfig::Local {
                executable,
                probe_size_bytes,
                analyze_duration_us,
            } => {
                let root = self
                    .location
                    .file
                    .as_ref()
                    .ok_or("remote source requires a remote probe binding")?
                    .canonicalize()?;
                let bindings = media
                    .iter()
                    .map(|r| -> Result<MediaBinding> {
                        r.validate()?;
                        if r.source_uri() != self.location.uri {
                            return Err("probe source mismatch".into());
                        }
                        // This is the private storage owner's binding boundary, not a public path.
                        let path = root.join(r.relative_path()).canonicalize()?;
                        if !path.starts_with(&root) || !path.is_file() {
                            return Err("media escapes source binding".into());
                        }
                        Ok(MediaBinding {
                            media_uri: r.uri(),
                            private_file: path,
                        })
                    })
                    .collect::<Result<_>>()?;
                Ok(Box::new(Executor::new(OwnerConfig {
                    executable: executable.clone(),
                    bindings,
                    timeout_ms: 30_000,
                    probe_size_bytes: *probe_size_bytes,
                    analyze_duration_us: *analyze_duration_us,
                    demuxers: vec!["mov".into(), "mxf".into()],
                })?))
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionConfig {
    pub version: String,
    pub catalog: Binding,
    pub source_index: Binding,
    pub media_records: Binding,
    pub sources: Vec<SourceConfig>,
    pub parallelism: usize,
}
impl SelectionConfig {
    pub fn load(root: &Path) -> Result<Self> {
        Self::from_transport_bindings(qnc_source_bindings::load(root)?)
    }

    pub fn from_transport_bindings(bindings: TransportBindings) -> Result<Self> {
        let mut config = Self {
            version: bindings.config_version,
            catalog: bindings
                .catalog
                .ok_or("transport catalog binding missing")?
                .into(),
            source_index: bindings
                .source_index
                .ok_or("transport source_index binding missing")?
                .into(),
            media_records: bindings
                .media_records
                .ok_or("transport media_records binding missing")?
                .into(),
            sources: bindings
                .registered_sources
                .into_iter()
                .map(source_config)
                .collect::<Result<Vec<_>>>()?,
            parallelism: bindings
                .parallelism
                .ok_or("transport parallelism missing")?,
        };
        if config.version != "0.1.0"
            || !(1..=8).contains(&config.parallelism)
            || config.sources.len() > 256
        {
            return Err("invalid Select configuration".into());
        }
        for binding in [
            &mut config.catalog,
            &mut config.source_index,
            &mut config.media_records,
        ] {
            binding.resolver()?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for source in &mut config.sources {
            source.location.resolver()?;
            SourceReference::new(&source.location.uri, ".")?;
            if !ids.insert(source.location.uri.clone()) || source.name.trim().is_empty() {
                return Err("invalid registered source".into());
            }
            match &mut source.probe {
                ProbeConfig::Local { .. } => {}
                ProbeConfig::Remote { binding } => {
                    binding.resolver()?;
                }
            }
        }
        Ok(config)
    }
    pub fn browser(&self) -> Result<TransportBrowserSession> {
        let sources = self
            .sources
            .iter()
            .map(|s| {
                let source = s.clone();
                let mut browser_source = BrowserSource::new(
                    BrowserEntry {
                        name: s.name.clone(),
                        qnc_uri: s.location.uri.clone(),
                        serial_number: s.serial_number.clone(),
                        volume_name: s.volume_name.clone(),
                    },
                    move || source.reader().map_err(|e| e.to_string()),
                );
                if let Some(path) = &s.location.file {
                    browser_source = browser_source.with_private_local_root(path.clone());
                }
                browser_source
            })
            .collect();
        Ok(TransportBrowserSession::new(sources)?)
    }
}

fn source_config(source: RegisteredSource) -> Result<SourceConfig> {
    let probe = match source.probe.ok_or("source probe missing")? {
        ProbeBinding::Local {
            executable,
            probe_size_bytes,
            analyze_duration_us,
        } => ProbeConfig::Local {
            executable,
            probe_size_bytes,
            analyze_duration_us,
        },
        ProbeBinding::Remote { binding } => ProbeConfig::Remote {
            binding: binding.into(),
        },
    };
    Ok(SourceConfig {
        location: source.location.into(),
        name: required(source.name, "source name missing")?,
        serial_number: required(source.serial_number, "source serial_number missing")?,
        volume_name: source.volume_name.unwrap_or_default(),
        scope: source_scope(required(source.scope, "source scope missing")?.as_str())?,
        probe,
    })
}

fn required(value: Option<String>, message: &'static str) -> Result<String> {
    value.ok_or_else(|| message.into())
}

fn source_scope(value: &str) -> Result<qnc_camera_detector::SourceScope> {
    match value {
        "card_relative" => Ok(qnc_camera_detector::SourceScope::CardRelative),
        "recording_relative" => Ok(qnc_camera_detector::SourceScope::RecordingRelative),
        "reel_relative" => Ok(qnc_camera_detector::SourceScope::ReelRelative),
        _ => Err("invalid source scope".into()),
    }
}

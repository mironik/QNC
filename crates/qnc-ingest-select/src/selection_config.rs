use qnc_dir_browser::{BrowserEntry, BrowserSource, TransportBrowserSession};
use qnc_media_probe::{Binding as MediaBinding, Executor, OwnerConfig, ProbeBackend};
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
            ("local", Some(file), None) if file.is_absolute() && self.token_env.is_none() => {
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
    fn absolutize(&mut self, base: &Path) {
        if let Some(path) = &mut self.file {
            if path.is_relative() {
                *path = base.join(&*path);
            }
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
        let file = std::env::var_os("QNC_INGEST_TRANSPORT_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("data").join("ingest-transport.json"));
        let file = file
            .canonicalize()
            .map_err(|_| "Nedostaje ingest-transport.json konfiguracija izvora i baza.")?;
        if std::fs::metadata(&file)?.len() > 1024 * 1024 {
            return Err("transport config too large".into());
        }
        let mut config: Self = serde_json::from_slice(&std::fs::read(&file)?)?;
        if config.version != "0.1.0"
            || !(1..=8).contains(&config.parallelism)
            || config.sources.len() > 256
        {
            return Err("invalid Select configuration".into());
        }
        let base = file.parent().ok_or("config parent missing")?;
        for binding in [
            &mut config.catalog,
            &mut config.source_index,
            &mut config.media_records,
        ] {
            binding.absolutize(base);
            binding.resolver()?;
        }
        let mut ids = std::collections::BTreeSet::new();
        for source in &mut config.sources {
            source.location.absolutize(base);
            source.location.resolver()?;
            SourceReference::new(&source.location.uri, ".")?;
            if !ids.insert(source.location.uri.clone()) || source.name.trim().is_empty() {
                return Err("invalid registered source".into());
            }
            match &mut source.probe {
                ProbeConfig::Local { executable, .. } => {
                    if executable.is_relative() {
                        let resolved = base.join(executable.as_path());
                        *executable = resolved;
                    }
                }
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
                BrowserSource::new(
                    BrowserEntry {
                        name: s.name.clone(),
                        qnc_uri: s.location.uri.clone(),
                        serial_number: s.serial_number.clone(),
                        volume_name: s.volume_name.clone(),
                    },
                    move || source.reader().map_err(|e| e.to_string()),
                )
            })
            .collect();
        Ok(TransportBrowserSession::new(sources)?)
    }
}

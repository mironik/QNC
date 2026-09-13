//! Host deployment selection. No media discovery, application identity or DB ownership.
use qnc_frame_timebase::FrameTimebase;
use qnc_media_decode::*;
use qnc_media_stream::CodecEndpoint;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    io::Read,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicBool},
};

pub const VERSION: &str = "1";
const MAX_CATALOG_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub version: String,
    pub selected: String,
    pub adapters: Vec<Registration>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub id: String,
    pub version: String,
    pub driver: Driver,
    pub executable: Executable,
    pub supported_os: Vec<String>,
    pub supported_cpu: Vec<String>,
    pub containers: Vec<String>,
    pub codecs: Vec<String>,
    pub pixel_formats: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedDeployment {
    pub id: String,
    pub driver: Driver,
    pub executable: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FilmstripExtractFrame {
    pub seek_sec: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilmstripExtractMode {
    RandomSeek,
    KeyframeSeek,
}

#[derive(Debug, Clone)]
pub struct LocalFilmstripExtractor {
    deployment: SelectedDeployment,
}

impl LocalFilmstripExtractor {
    pub fn extract_frames(
        &self,
        source: &Path,
        frames: &[FilmstripExtractFrame],
        source_timebase: FrameTimebase,
        mode: FilmstripExtractMode,
        thumb_size: [u32; 2],
        temp_dir: &Path,
    ) -> std::result::Result<(), String> {
        let cancel = AtomicBool::new(false);
        self.extract_frames_with_cancel(
            source,
            frames,
            source_timebase,
            mode,
            thumb_size,
            temp_dir,
            &cancel,
        )
    }

    pub fn extract_frames_with_cancel(
        &self,
        source: &Path,
        frames: &[FilmstripExtractFrame],
        source_timebase: FrameTimebase,
        mode: FilmstripExtractMode,
        thumb_size: [u32; 2],
        temp_dir: &Path,
        cancel: &AtomicBool,
    ) -> std::result::Result<(), String> {
        match &self.deployment.driver {
            Driver::FfmpegCliV1 => qnc_ffmpeg_decode::extract_filmstrip_frames_with_cancel(
                &self.deployment.executable,
                source,
                &frames
                    .iter()
                    .map(|frame| qnc_ffmpeg_decode::FfmpegFilmstripFrame {
                        seek_sec: frame.seek_sec,
                    })
                    .collect::<Vec<_>>(),
                source_timebase,
                match mode {
                    FilmstripExtractMode::RandomSeek => {
                        qnc_ffmpeg_decode::FfmpegFilmstripMode::RandomSeek
                    }
                    FilmstripExtractMode::KeyframeSeek => {
                        qnc_ffmpeg_decode::FfmpegFilmstripMode::KeyframeSeek
                    }
                },
                thumb_size,
                temp_dir,
                cancel,
            ),
            Driver::QncPacketsV1 { .. } => {
                Err("selected decoder does not provide local filmstrip extraction".into())
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "protocol", rename_all = "snake_case", deny_unknown_fields)]
pub enum Driver {
    FfmpegCliV1,
    QncPacketsV1 { args: Vec<String> },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Executable {
    Command {
        name: String,
    },
    /// Host-private path, resolved relative to the catalog, never a media URI.
    Path {
        path: PathBuf,
    },
}
fn bad(message: impl Into<String>) -> DecodeError {
    DecodeError::new(ErrorKind::Contract, message)
}
fn id_valid(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn list_valid(list: &[String]) -> bool {
    !list.is_empty()
        && list.len() <= 512
        && list
            .iter()
            .all(|s| !s.is_empty() && s.len() <= 128 && !s.contains('*'))
        && list.iter().collect::<BTreeSet<_>>().len() == list.len()
}
impl Catalog {
    pub fn read(path: &Path) -> Result<Self> {
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| bad("decoder catalog unavailable"))?
            .take(MAX_CATALOG_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| bad("cannot read decoder catalog"))?;
        if bytes.len() as u64 > MAX_CATALOG_BYTES {
            return Err(bad("decoder catalog too large"));
        }
        let catalog: Self =
            serde_json::from_slice(&bytes).map_err(|_| bad("invalid decoder catalog JSON"))?;
        catalog.validate()?;
        Ok(catalog)
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != VERSION || !id_valid(&self.selected) || self.adapters.len() > 128 {
            return Err(bad("invalid decoder catalog version or selection"));
        }
        let mut ids = BTreeSet::new();
        for a in &self.adapters {
            if !id_valid(&a.id)
                || !id_valid(&a.version)
                || !ids.insert(&a.id)
                || !list_valid(&a.supported_os)
                || !list_valid(&a.supported_cpu)
                || !list_valid(&a.containers)
                || !list_valid(&a.codecs)
                || !list_valid(&a.pixel_formats)
            {
                return Err(bad("invalid or duplicate decoder registration"));
            }
            if let Driver::QncPacketsV1 { args } = &a.driver
                && (args.len() > 32 || args.iter().any(|s| s.len() > 4096 || s.contains('\0')))
            {
                return Err(bad("invalid decoder launch arguments"));
            }
        }
        if !ids.contains(&self.selected) {
            return Err(bad("selected decoder is not registered"));
        }
        Ok(())
    }
    pub fn available<'a>(&'a self, directory: &Path) -> Vec<&'a Registration> {
        self.adapters
            .iter()
            .filter(|a| a.executable_path(directory).is_ok())
            .collect()
    }
    pub fn selected_config(&self, directory: &Path) -> Result<DecoderConfig> {
        let deployment = self.selected_deployment(directory)?;
        let registration = self
            .adapters
            .iter()
            .find(|a| a.id == deployment.id)
            .ok_or_else(|| bad("missing decoder selection"))?
            .clone();
        let adapter: Arc<dyn DecoderAdapter> = match &registration.driver {
            Driver::FfmpegCliV1 => {
                Arc::new(qnc_ffmpeg_decode::FfmpegAdapter::new(deployment.executable))
            }
            Driver::QncPacketsV1 { args } => Arc::new(ExternalAdapter {
                adapter_id: registration.id.clone(),
                executable: deployment.executable,
                args: args.clone(),
            }),
        };
        Ok(DecoderConfig::new(SelectedAdapter {
            registration: registration.clone(),
            adapter,
        }))
    }

    pub fn selected_deployment(&self, directory: &Path) -> Result<SelectedDeployment> {
        self.validate()?;
        let registration = self
            .adapters
            .iter()
            .find(|a| a.id == self.selected)
            .ok_or_else(|| bad("missing decoder selection"))?;
        Ok(SelectedDeployment {
            id: registration.id.clone(),
            driver: registration.driver.clone(),
            executable: registration.executable_path(directory)?,
        })
    }
}
impl Registration {
    fn executable_path(&self, directory: &Path) -> Result<PathBuf> {
        if !self.supported_os.iter().any(|s| s == std::env::consts::OS)
            || !self
                .supported_cpu
                .iter()
                .any(|s| s == std::env::consts::ARCH)
        {
            return Err(DecodeError::new(
                ErrorKind::Unsupported,
                "selected decoder does not support this OS/CPU",
            ));
        }
        let path = match &self.executable {
            Executable::Path { path } => {
                if path.as_os_str().is_empty() {
                    return Err(bad("empty decoder executable path"));
                }
                directory.join(path)
            }
            Executable::Command { name } => {
                if !id_valid(name) {
                    return Err(bad("invalid decoder command name"));
                }
                let mut file = name.clone();
                if cfg!(windows) && !file.to_ascii_lowercase().ends_with(".exe") {
                    file.push_str(".exe");
                }
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                    .filter(|p| p.is_absolute())
                    .map(|p| p.join(&file))
                    .find(|p| p.is_file())
                    .ok_or_else(|| bad(format!("selected decoder {} is not installed", self.id)))?
            }
        };
        if !path.is_file() {
            return Err(bad(format!(
                "selected decoder {} is not installed",
                self.id
            )));
        }
        path.canonicalize()
            .map_err(|_| bad("cannot resolve decoder executable"))
    }
}
#[derive(Debug)]
struct SelectedAdapter {
    registration: Registration,
    adapter: Arc<dyn DecoderAdapter>,
}
impl DecoderAdapter for SelectedAdapter {
    fn validate(&self, request: &DecodeRequest, plan: &DecodePlan) -> Result<()> {
        let a = &self.registration;
        if !a.containers.contains(&plan.container)
            || !a.codecs.contains(&plan.codec)
            || matches!(&plan.format, DecodedFormat::Video { pixel_format, .. } if !a.pixel_formats.contains(pixel_format))
        {
            return Err(DecodeError::new(
                ErrorKind::Unsupported,
                format!(
                    "selected decoder {} does not declare this saved format",
                    a.id
                ),
            ));
        }
        self.adapter.validate(request, plan)
    }
    fn launch(
        &self,
        request: &DecodeRequest,
        plan: &DecodePlan,
        ep: &CodecEndpoint,
        stamp: &str,
    ) -> Result<ProcessLaunch> {
        self.adapter.launch(request, plan, ep, stamp)
    }
}

/// An explicit override is authoritative: a bad override is never replaced by a default.
pub fn installed_config() -> Result<DecoderConfig> {
    let (catalog, directory) = installed_catalog()?;
    catalog.selected_config(&directory)
}

pub fn installed_deployment() -> Result<SelectedDeployment> {
    let (catalog, directory) = installed_catalog()?;
    catalog.selected_deployment(&directory)
}

pub fn installed_filmstrip_extractor() -> Result<Option<LocalFilmstripExtractor>> {
    let deployment = installed_deployment()?;
    Ok(match deployment.driver {
        Driver::FfmpegCliV1 => Some(LocalFilmstripExtractor { deployment }),
        Driver::QncPacketsV1 { .. } => None,
    })
}

fn installed_catalog() -> Result<(Catalog, PathBuf)> {
    let path = if let Some(path) = std::env::var_os("QNC_DECODER_CATALOG") {
        if path.is_empty() {
            return Err(bad("empty decoder catalog override"));
        }
        PathBuf::from(path)
    } else {
        let exe = std::env::current_exe().map_err(|_| bad("cannot locate decoder deployment"))?;
        exe.parent()
            .into_iter()
            .flat_map(Path::ancestors)
            .map(|p| p.join("catalogs/decoders/catalog.json"))
            .find(|p| p.is_file())
            .ok_or_else(|| bad("decoder catalog missing; configure QNC_DECODER_CATALOG"))?
    };
    let directory = path
        .parent()
        .ok_or_else(|| bad("invalid catalog path"))?
        .to_path_buf();
    Ok((Catalog::read(&path)?, directory))
}

#[cfg(test)]
mod tests;

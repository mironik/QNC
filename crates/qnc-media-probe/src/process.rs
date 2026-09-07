use crate::*;
use qnc_transport_resolver::{ResolvedEndpoint, ResolverConfig};
use std::{
    collections::BTreeMap,
    io::Read,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Instant,
};

/// Private storage-owner configuration, never part of a public request or DB record.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub media_uri: String,
    pub private_file: PathBuf,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerConfig {
    pub executable: PathBuf,
    pub bindings: Vec<Binding>,
    pub timeout_ms: u64,
    pub probe_size_bytes: u64,
    pub analyze_duration_us: u64,
    /// Explicit single-file demuxers; playlists, device inputs and scripts are excluded.
    pub demuxers: Vec<String>,
}
pub struct Executor {
    config: OwnerConfig,
    bindings: BTreeMap<String, (ResolverConfig, String)>,
}
impl Executor {
    pub fn new(config: OwnerConfig) -> Result<Self> {
        if !config.executable.is_absolute()
            || !(1..=30000).contains(&config.timeout_ms)
            || config.demuxers.is_empty()
        {
            return Err(Error::Configuration);
        }
        if !(32..=67108864).contains(&config.probe_size_bytes)
            || !(1..=5000000).contains(&config.analyze_duration_us)
        {
            return Err(Error::Configuration);
        }
        if config.demuxers.iter().any(|s| {
            s.is_empty()
                || !s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || matches!(
                    s.as_str(),
                    "concat" | "hls" | "dash" | "image2" | "avisynth" | "vapoursynth"
                )
        }) {
            return Err(Error::Configuration);
        }
        let mut bindings = BTreeMap::new();
        for (i, binding) in config.bindings.iter().enumerate() {
            validate_resource_uri(&binding.media_uri).map_err(|_| Error::Configuration)?;
            if !binding.private_file.is_absolute() {
                return Err(Error::Configuration);
            }
            let uri = format!("qnc://local/media/bound-{i}");
            let resolver =
                ResolverConfig::new(PathBuf::new()).with_local_binding(&uri, &binding.private_file);
            if bindings
                .insert(binding.media_uri.clone(), (resolver, uri))
                .is_some()
            {
                return Err(Error::Configuration);
            }
        }
        Ok(Self { config, bindings })
    }
    pub fn execute(&self, request: &Request) -> Result<Report> {
        request.validate()?;
        let (resolver, uri) = self
            .bindings
            .get(&request.media_uri)
            .ok_or(Error::UnboundMedia)?;
        let ResolvedEndpoint::LocalPath(file) = resolver
            .resolve(uri)
            .map_err(|_| Error::Configuration)?
            .endpoint
        else {
            return Err(Error::Configuration);
        };
        let mut command = Command::new(&self.config.executable);
        command
            .args(arguments(&self.config))
            .arg("-i")
            .arg(file)
            .env_remove("FFREPORT")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let started = Instant::now();
        let bytes = run_command(&mut command, Duration::from_millis(self.config.timeout_ms))?;
        let mut json: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Error::InvalidOutput)?;
        let format = json
            .get_mut("format")
            .and_then(|v| v.as_object_mut())
            .ok_or(Error::InvalidOutput)?;
        format.insert("filename".into(), request.media_uri.clone().into());
        let report = Report {
            request_id: request.request_id.clone(),
            media_uri: request.media_uri.clone(),
            document_uri: request.document_uri.clone(),
            json: serde_json::to_string(&json).map_err(|_| Error::InvalidOutput)?,
            elapsed_ms: started.elapsed().as_millis() as u64,
        };
        report.validate(request)?;
        Ok(report)
    }
}

pub(crate) fn run_command(command: &mut Command, timeout: Duration) -> Result<Vec<u8>> {
    let started = Instant::now();
    let mut child = command.spawn().map_err(|_| Error::Spawn)?;
    let overflow = Arc::new(AtomicBool::new(false));
    let out = capture(
        child.stdout.take().ok_or(Error::Spawn)?,
        MAX_DOCUMENT_BYTES,
        overflow.clone(),
    );
    let err = capture(
        child.stderr.take().ok_or(Error::Spawn)?,
        65536,
        overflow.clone(),
    );
    let outcome = loop {
        if overflow.load(Ordering::Relaxed) {
            break Err(Error::OutputLimit);
        }
        if started.elapsed() >= timeout {
            break Err(Error::Timeout);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(Error::Failed)
                }
            }
            Ok(None) => thread::sleep(Duration::from_millis(2)),
            Err(_) => break Err(Error::Failed),
        }
    };
    if outcome.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let bytes = out.join().map_err(|_| Error::InvalidOutput)?;
    let _ = err.join().map_err(|_| Error::InvalidOutput)?;
    outcome?;
    if overflow.load(Ordering::Relaxed) {
        return Err(Error::OutputLimit);
    }
    bytes
}

pub(crate) fn arguments(config: &OwnerConfig) -> Vec<String> {
    [
        "-v",
        "error",
        "-probesize",
        &config.probe_size_bytes.to_string(),
        "-analyzeduration",
        &config.analyze_duration_us.to_string(),
        "-threads",
        "1",
        "-protocol_whitelist",
        "file",
        "-format_whitelist",
        &config.demuxers.join(","),
        "-show_format",
        "-show_streams",
        "-show_programs",
        "-show_chapters",
        "-show_data",
        "-show_program_version",
        "-show_library_versions",
        "-show_error",
        "-show_optional_fields",
        "always",
        "-of",
        "json",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
fn capture(
    mut pipe: impl Read + Send + 'static,
    limit: usize,
    overflow: Arc<AtomicBool>,
) -> thread::JoinHandle<Result<Vec<u8>>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = pipe.read(&mut buffer).map_err(|_| Error::InvalidOutput)?;
            if n == 0 {
                break;
            }
            let keep = n.min(limit.saturating_sub(bytes.len()));
            bytes.extend_from_slice(&buffer[..keep]);
            if keep < n {
                overflow.store(true, Ordering::Relaxed);
            }
        }
        Ok(bytes)
    })
}

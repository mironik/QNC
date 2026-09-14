use qnc_broadcast_engine::{DecodeMediaAccess, InputPlan};
use qnc_json_transport::Credentials;
use qnc_media_stream::{CodecEndpoint, LocalSource, MediaStream, SourceReference};
use qnc_player_contract::VERSION;
use qnc_player_input::PreparedInput;
use serde::Deserialize;
use std::{
    io::{self, Read},
    path::PathBuf,
};

const MAX_BOOT_BYTES: u64 = 4 * 1024 * 1024;
type MediaOpener = Box<dyn FnMut(&str) -> io::Result<DecodeMediaAccess>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boot {
    pub contract_version: String,
    pub session_id: String,
    pub source_generation: u64,
    pub input: PreparedInput,
    pub media_binding: Binding,
    pub read_token: String,
    pub command_token: String,
    pub idle_timeout_ms: u64,
    pub listen_port: u16,
}

// Private process launch bindings, never fields in public command/event messages.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Binding {
    Local {
        source_uri: String,
        root: PathBuf,
    },
    Network {
        environment: String,
        authority: String,
        base_url: String,
        token: String,
    },
}
impl Binding {
    pub fn opener(self, media_uri: &str) -> crate::Result<MediaOpener> {
        match self {
            Self::Local { source_uri, root } => {
                if SourceReference::from_uri(media_uri)?.source_uri() != source_uri {
                    return Err("source binding differs from saved media".into());
                }
                let root = root.canonicalize()?;
                let source = LocalSource::new(&source_uri, &root)?;
                Ok(Box::new(move |uri| {
                    let media = MediaStream::local(&source, uri)?;
                    let storage_stamp = media.info().storage_stamp.clone();
                    let path = local_codec_path(&source_uri, &root, uri)?;
                    Ok(DecodeMediaAccess::Endpoint {
                        endpoint: CodecEndpoint::for_local_file(path, uri)?,
                        storage_stamp,
                    })
                }))
            }
            Self::Network {
                environment,
                authority,
                base_url,
                token,
            } => {
                if !matches!(environment.as_str(), "lan" | "intranet")
                    || authority.trim().is_empty()
                    || base_url.trim().is_empty()
                    || token.trim().is_empty()
                {
                    return Err("invalid media transport environment".into());
                }
                Ok(Box::new(move |_uri| {
                    Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "network playback requires a seekable QNC decoder endpoint; HTTP/raw TCP decoder input is disabled",
                    ))
                }))
            }
        }
    }
}

fn local_codec_path(
    source_uri: &str,
    root: &std::path::Path,
    media_uri: &str,
) -> io::Result<PathBuf> {
    let reference = SourceReference::from_uri(media_uri)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
    if reference.source_uri() != source_uri {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "media source mismatch",
        ));
    }
    let path = root.join(reference.relative_path()).canonicalize()?;
    if !path.starts_with(root) || !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "media escapes source binding",
        ));
    }
    Ok(path)
}

impl Boot {
    pub fn read(input: impl Read) -> crate::Result<Self> {
        let mut bytes = Vec::new();
        input.take(MAX_BOOT_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BOOT_BYTES {
            return Err("player bootstrap too large".into());
        }
        let boot: Self = serde_json::from_slice(&bytes)?;
        boot.validate()?;
        Ok(boot)
    }
    fn validate(&self) -> crate::Result<()> {
        if self.contract_version != VERSION
            || self.session_id.is_empty()
            || self.session_id.len() > 128
            || !self
                .session_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            || self.source_generation == 0
            || !(1000..=300_000).contains(&self.idle_timeout_ms)
        {
            return Err("invalid player bootstrap identity/timeout/version".into());
        }
        Credentials::new(&self.read_token, &self.command_token)?;
        self.plan()?;
        Ok(())
    }
    pub fn plan(&self) -> crate::Result<InputPlan> {
        InputPlan::new(
            &self.input,
            &self.input.workspace_db_uri,
            &self.input.snapshot.metadata.clip_id,
        )
        .map_err(Into::into)
    }
}

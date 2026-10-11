use qnc_broadcast_engine::{DecodeMediaAccess, InputPlan, ProgramPlan};
use qnc_json_transport::Credentials;
use qnc_media_stream::{CodecEndpoint, LocalSource, MediaStream, SourceReference};
use qnc_player_contract::VERSION;
use qnc_player_input::{PreparedInput, ProgramInput};
use serde::Deserialize;
use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

const MAX_BOOT_BYTES: u64 = 4 * 1024 * 1024;
type MediaOpener = Box<dyn FnMut(&str) -> io::Result<DecodeMediaAccess>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boot {
    pub contract_version: String,
    pub session_id: String,
    pub source_generation: u64,
    /// One clip; or `program` with a binding per media source, never both.
    #[serde(default)]
    pub input: Option<PreparedInput>,
    #[serde(default)]
    pub media_binding: Option<Binding>,
    /// The source of the sound of the clip when it is not the source of its picture.
    #[serde(default)]
    pub sound_binding: Option<Binding>,
    #[serde(default)]
    pub program: Option<ProgramInput>,
    #[serde(default)]
    pub program_bindings: Vec<Binding>,
    pub read_token: String,
    pub command_token: String,
    pub idle_timeout_ms: u64,
    pub listen_port: u16,
    pub monitor_frame_map: Option<PathBuf>,
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
        if let Self::Local { source_uri, .. } = &self
            && SourceReference::from_uri(media_uri)?.source_uri() != source_uri
        {
            return Err("source binding differs from saved media".into());
        }
        Ok(self.open_source()?.1)
    }

    /// The opener of this binding and the source it binds (`None`: network).
    fn open_source(self) -> crate::Result<(Option<String>, MediaOpener)> {
        match self {
            Self::Local { source_uri, root } => {
                let root = root.canonicalize()?;
                let source = LocalSource::new(&source_uri, &root)?;
                let bound = source_uri.clone();
                Ok((
                    Some(bound),
                    Box::new(move |uri| {
                        let media = MediaStream::local(&source, uri)?;
                        let storage_stamp = media.info().storage_stamp.clone();
                        let path = local_codec_path(&source_uri, &root, uri)?;
                        Ok(DecodeMediaAccess::Endpoint {
                            endpoint: CodecEndpoint::for_local_file(path, uri)?,
                            storage_stamp,
                        })
                    }),
                ))
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
                Ok((
                    None,
                    Box::new(move |_uri| {
                        Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "network playback requires a seekable QNC decoder endpoint; HTTP/raw TCP decoder input is disabled",
                        ))
                    }),
                ))
            }
        }
    }
}

/// One opener for the media of a program: each URI goes to the binding of its
/// own source (a program can hold clips of several cards). A source without a
/// binding is an error, never another source's opener.
pub fn program_opener(bindings: Vec<Binding>) -> crate::Result<MediaOpener> {
    let mut openers = bindings
        .into_iter()
        .map(Binding::open_source)
        .collect::<crate::Result<Vec<_>>>()?;
    Ok(Box::new(move |uri| {
        let source = SourceReference::from_uri(uri)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?
            .source_uri()
            .to_string();
        let opener = openers
            .iter_mut()
            .find(|(bound, _)| bound.as_deref() == Some(source.as_str()))
            .map(|(_, opener)| opener)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "program media source has no binding",
                )
            })?;
        opener(uri)
    }))
}

fn local_codec_path(source_uri: &str, root: &Path, media_uri: &str) -> io::Result<PathBuf> {
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
    /// The saved input of the session: one clip or one story program.
    pub fn plan(&self) -> crate::Result<Plan> {
        match (&self.input, &self.program) {
            (Some(input), None)
                if self.media_binding.is_some() && self.program_bindings.is_empty() =>
            {
                Ok(Plan::Clip(InputPlan::new(
                    input,
                    &input.workspace_db_uri,
                    &input.snapshot.metadata.clip_id,
                )?))
            }
            (None, Some(program))
                if self.media_binding.is_none()
                    && self.sound_binding.is_none()
                    && !self.program_bindings.is_empty() =>
            {
                Ok(Plan::Program(ProgramPlan::new(program)?))
            }
            _ => Err(
                "player bootstrap needs one clip with its binding or one program with its bindings"
                    .into(),
            ),
        }
    }

    /// The media opener of the session, taken once.
    pub fn take_opener(&mut self) -> crate::Result<MediaOpener> {
        if let (Some(input), Some(binding)) = (&self.input, self.media_binding.take()) {
            let Some(sound) = self.sound_binding.take() else {
                return binding.opener(&input.media()?.media_uri);
            };
            // Picture and sound from two sources: each URI goes to its own binding.
            return program_opener(vec![binding, sound]);
        }
        program_opener(std::mem::take(&mut self.program_bindings))
    }
}

pub enum Plan {
    Clip(InputPlan),
    Program(ProgramPlan),
}

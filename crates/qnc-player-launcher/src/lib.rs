use qnc_player_client::{Launch, LaunchInput, MediaBinding};
use std::path::PathBuf;

pub const MODULE_ID: &str = "qnc.module.player-launcher";
pub const VERSION: &str = "0.1.0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceTransportBinding {
    Local {
        source_uri: String,
        root: PathBuf,
    },
    Network {
        source_uri: String,
        environment: String,
        authority: String,
        base_url: String,
        token: String,
    },
}

impl SourceTransportBinding {
    pub fn local(source_uri: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        Self::Local {
            source_uri: source_uri.into(),
            root: root.into(),
        }
    }

    pub fn network(
        source_uri: impl Into<String>,
        base_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Result<Self, String> {
        let source_uri = source_uri.into();
        let parsed = qnc_contracts::parse_qnc_uri(&source_uri).map_err(|e| e.to_string())?;
        Ok(Self::Network {
            source_uri,
            environment: parsed.environment,
            authority: parsed.authority.ok_or("Source authority missing.")?,
            base_url: base_url.into(),
            token: token.into(),
        })
    }

    fn source_uri(&self) -> &str {
        match self {
            Self::Local { source_uri, .. } | Self::Network { source_uri, .. } => source_uri,
        }
    }

    fn media_binding(&self) -> MediaBinding {
        match self {
            Self::Local { source_uri, root } => MediaBinding::Local {
                source_uri: source_uri.clone(),
                root: root.clone(),
            },
            Self::Network {
                environment,
                authority,
                base_url,
                token,
                ..
            } => MediaBinding::Network {
                environment: environment.clone(),
                authority: authority.clone(),
                base_url: base_url.clone(),
                token: token.clone(),
            },
        }
    }
}

pub fn sibling_executable(name: &str) -> Result<PathBuf, String> {
    let executable = format!("{name}{}", std::env::consts::EXE_SUFFIX);
    let path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name(&executable);
    if !path.is_file() {
        return Err(format!(
            "Nedostaje {executable} pored {}. Izgradi -p qnc-player-runner istim profilom.",
            path.parent()
                .map(|parent| parent.display().to_string())
                .unwrap_or_else(|| ".".into())
        ));
    }
    Ok(path)
}

pub fn prepare_launch(
    input: qnc_player_input::PreparedInput,
    sources: &[SourceTransportBinding],
    executable: PathBuf,
) -> Result<Launch, String> {
    let picture = input.media().map_err(|e| e.to_string())?.media_uri.clone();
    let media_binding = binding_for(&picture, sources)?;
    let sound = &input.audio_media().media_uri;
    let source_of = |uri: &str| {
        qnc_source_reader::SourceReference::from_uri(uri)
            .map(|reference| reference.source_uri().to_string())
            .map_err(|e| e.to_string())
    };
    let sound_binding = if source_of(sound)? == source_of(&picture)? {
        None
    } else {
        Some(binding_for(sound, sources)?)
    };
    Ok(Launch {
        executable,
        input: LaunchInput::Clip {
            input,
            media_binding,
            sound_binding,
        },
    })
}

/// A story program: one binding for every media source its clips read.
pub fn prepare_program_launch(
    program: qnc_player_input::ProgramInput,
    sources: &[SourceTransportBinding],
    executable: PathBuf,
) -> Result<Launch, String> {
    let mut source_uris = Vec::new();
    for media in program
        .playlist
        .items
        .iter()
        .flat_map(|item| &item.sources)
        .map(|source| &source.media.media_uri)
    {
        let source_uri = qnc_source_reader::SourceReference::from_uri(media)
            .map_err(|e| e.to_string())?
            .source_uri()
            .to_string();
        if !source_uris.contains(&source_uri) {
            source_uris.push(source_uri);
        }
    }
    let media_bindings = source_uris
        .iter()
        .map(|source_uri| {
            sources
                .iter()
                .find(|binding| binding.source_uri() == source_uri)
                .map(SourceTransportBinding::media_binding)
                .ok_or_else(|| "Program source has no transport binding.".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Launch {
        executable,
        input: LaunchInput::Program {
            program,
            media_bindings,
        },
    })
}

fn binding_for(
    media_uri: &str,
    sources: &[SourceTransportBinding],
) -> Result<MediaBinding, String> {
    let reference =
        qnc_source_reader::SourceReference::from_uri(media_uri).map_err(|e| e.to_string())?;
    Ok(sources
        .iter()
        .find(|binding| binding.source_uri() == reference.source_uri())
        .ok_or("Player source has no transport binding.")?
        .media_binding())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_binding_is_derived_from_qnc_source_uri() {
        let binding = SourceTransportBinding::network(
            "qnc://lan/studio/source/card-a",
            "https://lan.example.test",
            "secret",
        )
        .unwrap();
        assert_eq!(
            binding,
            SourceTransportBinding::Network {
                source_uri: "qnc://lan/studio/source/card-a".into(),
                environment: "lan".into(),
                authority: "studio".into(),
                base_url: "https://lan.example.test".into(),
                token: "secret".into(),
            }
        );
    }
}

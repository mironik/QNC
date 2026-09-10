use qnc_player_client::{Launch, MediaBinding};
use qnc_work_settings::SettingsReader;
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
    std::env::current_exe()
        .map_err(|e| e.to_string())
        .map(|path| path.with_file_name(executable))
}

pub fn prepare_launch(
    settings: SettingsReader,
    workspace_db_uri: &str,
    clip_id: &str,
    sources: &[SourceTransportBinding],
    executable: PathBuf,
) -> Result<Launch, String> {
    let input = qnc_player_input::InputReader::new(settings)
        .load(workspace_db_uri, clip_id)
        .map_err(|e| e.to_string())?;
    let media_uri = &input.media().map_err(|e| e.to_string())?.media_uri;
    let reference =
        qnc_source_reader::SourceReference::from_uri(media_uri).map_err(|e| e.to_string())?;
    let media_binding = sources
        .iter()
        .find(|binding| binding.source_uri() == reference.source_uri())
        .ok_or("Player source has no transport binding.")?
        .media_binding();
    Ok(Launch {
        executable,
        input,
        media_binding,
    })
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

use crate::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Original,
    Proxy,
}

impl Folder {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Folder::Original => "original",
            Folder::Proxy => "proxy",
        }
    }

    pub(crate) fn copy(self) -> ImportedCopy {
        match self {
            Folder::Original => ImportedCopy::Original,
            Folder::Proxy => ImportedCopy::Proxy,
        }
    }
}

/// What the project settings ask for one clip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Keep the media where it is; the imported media is this source URI.
    Link { media_uri: String },
    /// Copy this source medium into the project folder.
    Copy { source_uri: String, folder: Folder },
    /// Make the optimized copy of the original into the project; with the original
    /// copied too, else the original stays linked where it is.
    Optimize { with_original: bool },
}

pub fn action_for(clip: &StoredClip, plan: &IngestWorkPlan) -> Result<Action, String> {
    let binding = &clip.clip.snapshot.binding;
    let original = binding.original_uri.clone();
    let proxy = binding.proxy_uri.clone();
    match plan.media {
        IngestMedia::Link => {
            let media_uri = match plan.playback_input {
                PlaybackInput::Original => original,
                PlaybackInput::Proxy => proxy
                    .ok_or_else(|| qnc_ui_text::text("import.playback_input_proxy_but_camera_proxy").to_string())?,
                PlaybackInput::ProxyIfAvailable => proxy.unwrap_or(original),
                // A link makes no optimized copy, so the original plays.
                PlaybackInput::OptimizedIfAvailable => original,
            };
            Ok(Action::Link { media_uri })
        }
        IngestMedia::Proxy => match proxy {
            Some(source_uri) => Ok(Action::Copy {
                source_uri,
                folder: Folder::Proxy,
            }),
            None => Err(qnc_ui_text::text("import.clip_proxy_making_proxy_supported_yet").into()),
        },
        IngestMedia::Original => Ok(Action::Copy {
            source_uri: original,
            folder: Folder::Original,
        }),
        IngestMedia::Optimized { with_original } => Ok(Action::Optimize { with_original }),
    }
}

/// A file name that is safe on every operating system and inside a QNC URI.
/// The name of the source file is kept readable (decoded from the URI) with its extension.
pub(crate) fn safe_name(clip_id: &str, source_uri: &str) -> String {
    let decoded = qnc_source_reader::SourceReference::from_uri(source_uri)
        .ok()
        .and_then(|reference| reference.relative_path().rsplit('/').next().map(str::to_owned));
    let last = decoded.unwrap_or_else(|| source_uri.rsplit('/').next().unwrap_or("media").to_owned());
    let name: String = format!("{clip_id}_{last}")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if name.len() <= 120 {
        return name;
    }
    let extension = name.rfind('.').map(|dot| name[dot..].to_owned()).filter(|e| e.len() <= 16);
    let extension = extension.unwrap_or_default();
    format!("{}{extension}", &name[..120 - extension.len()])
}

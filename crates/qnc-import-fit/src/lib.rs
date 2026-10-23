//! Whether a clip's stored media record serves what the project uses (user 2026-10-23: "the
//! import checks only what the project uses"). The original is always used: its sound plays
//! and goes to the export. The proxy is used only when the project plays it (`playback.input`
//! proxy, or proxy if available and the clip has one) or copies it (`storage.ingest_media`
//! proxy). A gap in a representation the project does not use never stops the import.
//! Pure rule over the stored record: no database, no media, no probe.

use qnc_media_records::{EvidenceKind, Phase, Snapshot};
use qnc_work_settings::{PlaybackInput, WorkSettings};
use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.import-fit";

/// What of a clip the project uses, from its settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needs {
    pub playback: Playback,
    /// The import copies the proxy into the project.
    pub copies_proxy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Playback {
    /// The original picture, or the optimized copy the import makes of it.
    Original,
    Proxy,
    ProxyIfAvailable,
}

impl Needs {
    /// The project's needs; settings without a readable playback input give their error.
    pub fn of(settings: &WorkSettings) -> Result<Self, String> {
        let playback = match settings.playback_input().map_err(|e| e.to_string())? {
            PlaybackInput::Proxy => Playback::Proxy,
            PlaybackInput::ProxyIfAvailable => Playback::ProxyIfAvailable,
            PlaybackInput::Original | PlaybackInput::OptimizedIfAvailable => Playback::Original,
        };
        Ok(Self { playback, copies_proxy: settings.storage.ingest_media == "proxy" })
    }

    /// Only the original matters (a clip without a proxy is fine).
    pub fn original_only() -> Self {
        Self { playback: Playback::Original, copies_proxy: false }
    }

    fn proxy_required(&self) -> bool {
        self.copies_proxy || self.playback == Playback::Proxy
    }
}

/// What is in the way of a clip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Conflict {
    /// The project uses the proxy and the card has none for the clip.
    ProxyMissing,
    /// The proxy the project uses lacks facts its one probe could not give.
    ProxyIncomplete { missing: Vec<String> },
    /// The original lacks facts its one probe could not give.
    OriginalIncomplete { missing: Vec<String> },
}

/// What stops the clip of `snapshot` from being imported for `needs`, if anything. A record
/// still in the camera phase is completed in the background: only a proxy the card does not
/// have is known then.
pub fn conflict(snapshot: &Snapshot, needs: Needs) -> Option<Conflict> {
    let has_proxy = snapshot.metadata.proxy.is_some();
    if needs.proxy_required() && !has_proxy {
        return Some(Conflict::ProxyMissing);
    }
    let probed = snapshot.metadata.evidence.iter().any(|e| e.kind == EvidenceKind::Ffprobe);
    if snapshot.phase != Phase::Final || !probed {
        return None; // a card record: what playback lacks is completed in the background (v5)
    }
    let (proxy, original): (Vec<String>, Vec<String>) =
        snapshot.report.issues.iter().map(|issue| issue.path.clone()).partition(|path| path.starts_with("proxy"));
    if !original.is_empty() {
        return Some(Conflict::OriginalIncomplete { missing: original });
    }
    let plays_proxy = match needs.playback {
        Playback::Proxy => true,
        Playback::ProxyIfAvailable => has_proxy,
        Playback::Original => false,
    };
    ((plays_proxy || needs.copies_proxy) && !proxy.is_empty()).then_some(Conflict::ProxyIncomplete { missing: proxy })
}

#[cfg(test)]
mod tests;

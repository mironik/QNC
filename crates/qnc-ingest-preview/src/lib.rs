//! The Ingest content database as the clip source of the source preview.
//!
//! The preview itself is the neutral `qnc-source-preview`; this crate only says where
//! Ingest keeps its clips and which sources carry the media.

use qnc_ingest_select::SelectionConfig;
use qnc_ingest_store::content::{Access, ContentTarget};
use qnc_player_input::{PlayerClipRecord, PlayerContentRead};
use qnc_source_bindings::SourceBinding;
use qnc_source_preview::PreviewContext;
use qnc_work_settings::{SettingsReader, WorkSettings};
use std::sync::Arc;

pub const MODULE_ID: &str = "qnc.module.ingest-preview";
pub const VERSION: &str = "0.1.0";

#[derive(Clone)]
struct IngestPlayerContent {
    target: ContentTarget,
}

impl PlayerContentRead for IngestPlayerContent {
    fn read_clip(&self, clip_id: &str) -> Result<Option<PlayerClipRecord>, String> {
        let stored = self.target.open(Access::ReadOnly)?.read(clip_id)?;
        Ok(stored.map(|stored| PlayerClipRecord {
            name: stored.clip.name,
            snapshot: stored.clip.snapshot,
            imported_media_uri: stored.imported_media_uri,
        }))
    }
}

pub fn preview_context(
    reader: SettingsReader,
    settings: WorkSettings,
    config: &SelectionConfig,
    target: ContentTarget,
) -> PreviewContext {
    let sources = config
        .sources
        .iter()
        .map(|source| SourceBinding {
            uri: source.location.uri.clone(),
            file: source.location.file.clone(),
            endpoint: source.location.endpoint.clone(),
            token_env: source.location.token_env.clone(),
        })
        .collect();
    PreviewContext::with_readers(
        reader,
        settings,
        sources,
        Arc::new(IngestPlayerContent { target }),
        None,
    )
}

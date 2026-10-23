//! The Ingest content database as the source of the clips the universal preview opens.

use qnc_ingest_select::SelectionConfig;
use qnc_ingest_store::content::{Access, ContentTarget};
use qnc_player_input::{PlayerClipRecord, PlayerContentRead};
use qnc_source_bindings::SourceBinding;
use qnc_source_preview::PreviewContext;
use qnc_work_settings::{SettingsReader, WorkSettings};
use std::sync::Arc;

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
            imported_copy_of: stored.imported_copy_of.map(|copy| match copy {
                qnc_ingest_store::content::ImportedCopy::Original => qnc_player_input::Representation::Original,
                qnc_ingest_store::content::ImportedCopy::Proxy => qnc_player_input::Representation::Proxy,
            }),
            imported_optimized: stored.imported_optimized,
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
    let artifacts = qnc_content_artifacts::timeline_artifact_reader(&reader, &settings).ok();
    PreviewContext::with_readers(
        reader,
        settings,
        sources,
        Arc::new(IngestPlayerContent { target }),
        artifacts,
    )
}


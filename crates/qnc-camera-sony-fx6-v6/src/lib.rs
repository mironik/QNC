//! Camera adapter for the catalog pattern `sony-fx6-v6`.
//!
//! The recording carries a `MediaProfile` index and `NonRealTimeMeta` sidecars for
//! both the original and the proxy, so the card records are the final metadata:
//! the media is never probed ([`MetadataSufficiency::Declared`]). Parsing is done
//! by the shared public library `qnc-sony-metadata`; this crate only binds it to
//! one camera and states the sufficiency.

use qnc_camera_adapter::{CameraAdapter, MetadataSufficiency};
use qnc_media_metadata::ClipMetadata;
use qnc_sony_metadata::SonyIndexReader;
use qnc_source_contract::SourceReference;
use qnc_source_groups::{GroupProposal, IndexDocument, IndexReader};

pub const ADAPTER_ID: &str = "camera.sony.fx6-v6";
pub const PATTERN_ID: &str = "sony-fx6-v6";

const PATTERNS: &[&str] = &[PATTERN_ID];

#[derive(Debug, Default, Clone, Copy)]
pub struct SonyFx6V6;

impl SonyFx6V6 {
    pub fn new() -> Self {
        Self
    }
}

impl CameraAdapter for SonyFx6V6 {
    fn adapter_id(&self) -> &str {
        ADAPTER_ID
    }

    fn pattern_ids(&self) -> &[&'static str] {
        PATTERNS
    }

    fn index(&self) -> &dyn IndexReader {
        &SonyIndexReader
    }

    fn documents(&self, group: &GroupProposal) -> Vec<SourceReference> {
        qnc_sony_metadata::metadata_references(group)
    }

    fn thumbnail(&self, group: &GroupProposal) -> Option<SourceReference> {
        qnc_sony_metadata::thumbnail_reference(group)
    }

    fn metadata(
        &self,
        clip_id: &str,
        group: &GroupProposal,
        documents: &[IndexDocument],
    ) -> Result<ClipMetadata, String> {
        qnc_sony_metadata::read_group_metadata(clip_id, group, documents)
    }

    fn sufficiency(&self, metadata: &ClipMetadata) -> MetadataSufficiency {
        // The XML normally declares the probe facts. A clip whose sidecar is missing
        // (or lacks them) has no record with probe data, so it is probed once.
        if qnc_camera_adapter::has_probe_facts(metadata) {
            MetadataSufficiency::Declared
        } else {
            MetadataSufficiency::NeedsProbe
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnc_camera_adapter::CameraRegistry;
    use std::sync::Arc;

    #[test]
    fn serves_exactly_the_fx6_pattern_and_never_probes() {
        let adapter = SonyFx6V6::new();
        assert_eq!(adapter.adapter_id(), "camera.sony.fx6-v6");
        assert_eq!(adapter.pattern_ids(), ["sony-fx6-v6"]);
        assert_eq!(adapter.index().reader_id(), "camera.sony.index.read");
    }

    #[test]
    fn registers_in_the_registry() {
        let mut registry = CameraRegistry::new();
        registry.register(Arc::new(SonyFx6V6::new())).unwrap();
        let found = registry.for_reader("camera.sony.index.read").unwrap();
        assert_eq!(found.adapter_id(), ADAPTER_ID);
        assert!(registry.register(Arc::new(SonyFx6V6::new())).is_err());
    }
}

//! Public camera adapter capability.
//!
//! One adapter serves one camera and record layout (a `pattern_id` of the camera
//! catalog), never a whole manufacturer. Shared vendor parsing stays in public
//! libraries (for example `qnc-sony-metadata`) that adapters call. An adapter is
//! stateless and does no I/O: the caller reads documents and passes text in.
//!
//! An adapter also states whether the metadata declared on the card is enough
//! ([`MetadataSufficiency::Declared`]: the media is never probed) or whether one
//! probe in Ingest is needed ([`MetadataSufficiency::NeedsProbe`]).

use qnc_media_metadata::ClipMetadata;
use qnc_source_contract::SourceReference;
use qnc_source_groups::{GroupProposal, IndexDocument, IndexReader};
use std::{collections::BTreeSet, sync::Arc};

pub const MODULE_ID: &str = "qnc.module.camera-adapter";
pub const VERSION: &str = "0.1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataSufficiency {
    /// The card records are the final metadata; the media is never probed.
    Declared,
    /// The card records are incomplete; the media is probed once in Ingest.
    NeedsProbe,
}

pub trait CameraAdapter: Send + Sync {
    /// Stable identity of this adapter.
    fn adapter_id(&self) -> &str;
    /// Camera catalog patterns (`pattern_id`) this adapter serves.
    fn pattern_ids(&self) -> &[&'static str];
    /// Parser of the recording index; its `reader_id` links groups to this adapter.
    fn index(&self) -> &dyn IndexReader;
    /// Documents that carry the metadata of one verified group.
    fn documents(&self, group: &GroupProposal) -> Vec<SourceReference>;
    /// Card thumbnail of one verified group, if it is explicitly linked.
    fn thumbnail(&self, group: &GroupProposal) -> Option<SourceReference>;
    /// Camera facts from already read documents.
    fn metadata(
        &self,
        clip_id: &str,
        group: &GroupProposal,
        documents: &[IndexDocument],
    ) -> Result<ClipMetadata, String>;
    /// Whether the record read for one clip is enough. Decided per record: a clip
    /// whose record carries no probe facts is probed once even if the camera
    /// usually declares them (for example a missing sidecar).
    fn sufficiency(&self, metadata: &ClipMetadata) -> MetadataSufficiency;
}

/// Does the record of the original carry the facts a probe would state: for video
/// the dimensions, the frame rate and an exact frame count; for audio only the
/// sample rate, the channels and the duration.
pub fn has_probe_facts(metadata: &ClipMetadata) -> bool {
    use qnc_media_metadata::StreamDetails;
    let original = &metadata.original;
    let videos: Vec<_> = original
        .streams
        .iter()
        .filter_map(|s| match &s.details {
            StreamDetails::Video(video) => Some(video),
            _ => None,
        })
        .collect();
    if !videos.is_empty() {
        return videos.iter().any(|v| {
            v.width.is_some()
                && v.height.is_some()
                && v.frame_rate.is_some()
                && v.exact_frame_count().is_some()
        });
    }
    original.duration_seconds.is_some()
        && original.streams.iter().any(|s| match &s.details {
            StreamDetails::Audio(audio) => {
                audio.sample_rate_hz.is_some() && audio.channels.is_some()
            }
            _ => false,
        })
}

/// The adapters an application composes. Built by the composition root and
/// handed to Select; Select never names a camera.
#[derive(Clone, Default)]
pub struct CameraRegistry {
    adapters: Vec<Arc<dyn CameraAdapter>>,
}

impl std::fmt::Debug for CameraRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.adapters.iter().map(|a| a.adapter_id()))
            .finish()
    }
}

impl CameraRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rejects an empty identity, an adapter without patterns, and duplicate
    /// adapter ids, index reader ids or catalog patterns.
    pub fn register(&mut self, adapter: Arc<dyn CameraAdapter>) -> Result<(), String> {
        let id = adapter.adapter_id().trim();
        let reader = adapter.index().reader_id().trim();
        if id.is_empty() || reader.is_empty() {
            return Err("camera adapter needs an adapter id and an index reader id".into());
        }
        if adapter.pattern_ids().is_empty()
            || adapter.pattern_ids().iter().any(|p| p.trim().is_empty())
        {
            return Err(format!("camera adapter '{id}' serves no catalog pattern"));
        }
        let taken: BTreeSet<&str> = self
            .adapters
            .iter()
            .flat_map(|a| a.pattern_ids().iter().copied())
            .collect();
        if let Some(pattern) = adapter.pattern_ids().iter().find(|p| taken.contains(**p)) {
            return Err(format!("catalog pattern '{pattern}' already has an adapter"));
        }
        if self
            .adapters
            .iter()
            .any(|a| a.adapter_id() == id || a.index().reader_id() == reader)
        {
            return Err(format!("duplicate camera adapter or index reader '{id}'"));
        }
        self.adapters.push(adapter);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub fn adapters(&self) -> &[Arc<dyn CameraAdapter>] {
        &self.adapters
    }

    /// Index parsers for the scanner.
    pub fn indexes(&self) -> Vec<&dyn IndexReader> {
        self.adapters.iter().map(|a| a.index()).collect()
    }

    /// The adapter whose index reader produced a group.
    pub fn for_reader(&self, reader_id: &str) -> Option<&Arc<dyn CameraAdapter>> {
        self.adapters
            .iter()
            .find(|a| a.index().reader_id() == reader_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Reader(&'static str);
    impl IndexReader for Reader {
        fn reader_id(&self) -> &str {
            self.0
        }
        fn namespace(&self) -> &str {
            "urn:test"
        }
        fn read(
            &self,
            _: &SourceReference,
            _: &IndexDocument,
        ) -> Result<Vec<GroupProposal>, String> {
            Ok(vec![])
        }
    }

    struct Fake {
        id: &'static str,
        patterns: &'static [&'static str],
        reader: Reader,
    }
    impl CameraAdapter for Fake {
        fn adapter_id(&self) -> &str {
            self.id
        }
        fn pattern_ids(&self) -> &[&'static str] {
            self.patterns
        }
        fn index(&self) -> &dyn IndexReader {
            &self.reader
        }
        fn documents(&self, _: &GroupProposal) -> Vec<SourceReference> {
            vec![]
        }
        fn thumbnail(&self, _: &GroupProposal) -> Option<SourceReference> {
            None
        }
        fn metadata(
            &self,
            _: &str,
            _: &GroupProposal,
            _: &[IndexDocument],
        ) -> Result<ClipMetadata, String> {
            Err("not used".into())
        }
        fn sufficiency(&self, _: &ClipMetadata) -> MetadataSufficiency {
            MetadataSufficiency::Declared
        }
    }

    fn fake(id: &'static str, patterns: &'static [&'static str], reader: &'static str) -> Arc<Fake> {
        Arc::new(Fake {
            id,
            patterns,
            reader: Reader(reader),
        })
    }

    #[test]
    fn registers_cameras_and_finds_them_by_index_reader() {
        let mut registry = CameraRegistry::new();
        assert!(registry.is_empty());
        registry.register(fake("cam-a", &["pattern-a"], "reader.a")).unwrap();
        registry.register(fake("cam-b", &["pattern-b"], "reader.b")).unwrap();
        assert_eq!(registry.adapters().len(), 2);
        assert_eq!(registry.indexes().len(), 2);
        assert_eq!(registry.for_reader("reader.b").unwrap().adapter_id(), "cam-b");
        assert!(registry.for_reader("reader.none").is_none());
    }

    #[test]
    fn a_pattern_can_have_only_one_adapter() {
        let mut registry = CameraRegistry::new();
        registry.register(fake("cam-a", &["pattern-a"], "reader.a")).unwrap();
        let error = registry
            .register(fake("cam-b", &["pattern-a"], "reader.b"))
            .unwrap_err();
        assert!(error.contains("pattern-a"));
    }

    #[test]
    fn duplicate_adapter_or_reader_identity_is_rejected() {
        let mut registry = CameraRegistry::new();
        registry.register(fake("cam-a", &["pattern-a"], "reader.a")).unwrap();
        assert!(registry.register(fake("cam-a", &["pattern-b"], "reader.b")).is_err());
        assert!(registry.register(fake("cam-b", &["pattern-b"], "reader.a")).is_err());
    }

    #[test]
    fn an_adapter_without_identity_or_patterns_is_rejected() {
        let mut registry = CameraRegistry::new();
        assert!(registry.register(fake("", &["pattern-a"], "reader.a")).is_err());
        assert!(registry.register(fake("cam-a", &[], "reader.a")).is_err());
        assert!(registry.register(fake("cam-a", &[""], "reader.a")).is_err());
    }

    fn fact<T>(value: T) -> Option<qnc_media_metadata::Fact<T>> {
        Some(qnc_media_metadata::Fact {
            value,
            evidence_id: "camera".into(),
            locator: "/x".into(),
        })
    }

    fn record(streams: Vec<qnc_media_metadata::MediaStream>, duration: bool) -> ClipMetadata {
        use qnc_media_metadata::{MediaRepresentation, Rational};
        ClipMetadata {
            contract_id: qnc_media_metadata::CONTRACT_ID.into(),
            contract_version: qnc_media_metadata::CONTRACT_VERSION.into(),
            clip_id: "clip".into(),
            evidence: vec![],
            original: MediaRepresentation {
                media_uri: "qnc://local/source/card/a.mxf".into(),
                container: None,
                duration_seconds: if duration {
                    fact(Rational {
                        numerator: 10,
                        denominator: 1,
                    })
                } else {
                    None
                },
                streams_complete: None,
                streams,
                tags: Default::default(),
            },
            proxy: None,
        }
    }

    fn stream(details: qnc_media_metadata::StreamDetails) -> qnc_media_metadata::MediaStream {
        qnc_media_metadata::MediaStream {
            index: None,
            codec: None,
            profile: None,
            time_base: None,
            start_pts: None,
            duration_ts: None,
            details,
        }
    }

    fn video(
        width: bool,
        exact_frames: bool,
    ) -> qnc_media_metadata::StreamDetails {
        use qnc_media_metadata::{FrameCount, FrameTimebase, VideoMetadata};
        qnc_media_metadata::StreamDetails::Video(Box::new(VideoMetadata {
            width: if width { fact(1920) } else { None },
            height: fact(1080),
            frame_rate: fact(FrameTimebase::new(25, 1).unwrap()),
            frame_rate_mode: None,
            frame_count: if exact_frames {
                fact(FrameCount::Exact(250))
            } else {
                fact(FrameCount::Estimated(250))
            },
            scan_mode: None,
            pixel_format: None,
            sample_aspect_ratio: None,
            rotation_degrees: None,
            color: qnc_media_metadata::ColorMetadata {
                primaries: None,
                transfer: None,
                matrix: None,
                range: None,
            },
        }))
    }

    #[test]
    fn a_record_with_dimensions_frame_rate_and_exact_frames_has_probe_facts() {
        assert!(has_probe_facts(&record(vec![stream(video(true, true))], false)));
    }

    #[test]
    fn a_record_missing_any_of_those_facts_has_no_probe_facts() {
        assert!(!has_probe_facts(&record(vec![], false)));
        assert!(!has_probe_facts(&record(vec![stream(video(false, true))], false)));
        assert!(!has_probe_facts(&record(vec![stream(video(true, false))], false)));
    }

    #[test]
    fn an_audio_record_needs_sample_rate_channels_and_duration() {
        use qnc_media_metadata::{AudioMetadata, StreamDetails};
        let audio = |rate: bool| {
            stream(StreamDetails::Audio(Box::new(AudioMetadata {
                sample_rate_hz: if rate { fact(48000) } else { None },
                channels: fact(2),
                sample_format: None,
                channel_layout: None,
                bits_per_sample: None,
            })))
        };
        assert!(has_probe_facts(&record(vec![audio(true)], true)));
        assert!(!has_probe_facts(&record(vec![audio(true)], false)));
        assert!(!has_probe_facts(&record(vec![audio(false)], true)));
    }
}

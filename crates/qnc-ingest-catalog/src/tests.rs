use crate::*;

mod loader_tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct TestClip {
        clip_id: String,
        selected: bool,
    }

    impl CatalogClipItem for TestClip {
        fn catalog_clip_id(&self) -> &str {
            &self.clip_id
        }

        fn catalog_selected(&self) -> bool {
            self.selected
        }

        fn from_catalog_row(row: CatalogClipRow) -> Self {
            Self {
                clip_id: row.clip_id,
                selected: row.selected,
            }
        }
    }

    #[test]
    fn an_idle_loader_delivers_nothing_and_cancels_safely() {
        let mut loader = CatalogLoader::new();
        assert!(!loader.is_busy());
        assert!(loader.poll().is_none());
        loader.cancel();
        assert!(!loader.is_busy());
    }

    #[test]
    fn applying_catalog_rows_preserves_local_selection_and_collects_thumbnails() {
        let current = vec![TestClip {
            clip_id: "clip-b".into(),
            selected: true,
        }];
        let rows = vec![
            CatalogClipRow {
                clip_id: "clip-a".into(),
                name: "A".into(),
                duration_seconds: 1.0,
                selected: false,
                imported: false,
                previously_seen: true,
                metadata_revision: 1,
                thumb_uri: Some("thumb://a".into()),
                thumb_status: CatalogThumbStatus::Pending,
            },
            CatalogClipRow {
                clip_id: "clip-b".into(),
                name: "B".into(),
                duration_seconds: 1.0,
                selected: false,
                imported: false,
                previously_seen: true,
                metadata_revision: 1,
                thumb_uri: None,
                thumb_status: CatalogThumbStatus::Missing,
            },
        ];

        let applied = apply_catalog_rows(&current, rows);

        assert_eq!(
            applied.clips,
            vec![
                TestClip {
                    clip_id: "clip-a".into(),
                    selected: false
                },
                TestClip {
                    clip_id: "clip-b".into(),
                    selected: true
                }
            ]
        );
        assert_eq!(
            applied.thumbnail_requests,
            vec![ThumbnailRequest {
                clip_id: "clip-a".into(),
                thumb_uri: "thumb://a".into()
            }]
        );
    }
}

mod new_after_reload_tests {
    use super::*;

    /// A clip as a view keeps it: whether it was seen before the current Select.
    struct Shown {
        clip_id: String,
        seen: bool,
    }

    impl CatalogClipItem for Shown {
        fn catalog_clip_id(&self) -> &str {
            &self.clip_id
        }
        fn catalog_selected(&self) -> bool {
            false
        }
        fn catalog_new(&self) -> bool {
            !self.seen
        }
        fn from_catalog_row(row: CatalogClipRow) -> Self {
            Self { clip_id: row.clip_id, seen: row.previously_seen }
        }
    }

    fn row(clip_id: &str) -> CatalogClipRow {
        CatalogClipRow {
            clip_id: clip_id.into(),
            name: clip_id.into(),
            duration_seconds: 1.0,
            selected: false,
            imported: false,
            previously_seen: true,
            metadata_revision: 1,
            thumb_uri: None,
            thumb_status: CatalogThumbStatus::Missing,
        }
    }

    #[test]
    fn a_reload_after_select_keeps_the_new_clips_new() {
        let shown = vec![
            Shown { clip_id: "old".into(), seen: true },
            Shown { clip_id: "new".into(), seen: false },
        ];
        let applied = apply_catalog_rows(&shown, vec![row("old"), row("new"), row("other")]);
        let seen: Vec<_> = applied.clips.iter().map(|clip| (clip.clip_id.as_str(), clip.seen)).collect();
        assert_eq!(seen, [("old", true), ("new", false), ("other", true)], "the New filter still shows it");
    }
}

use super::*;

fn summary(id: &str, poster: Option<&str>) -> ClipSummary {
    ClipSummary {
        clip_id: id.into(),
        name: id.into(),
        duration_seconds: 1.0,
        duration_frames: 25,
        imported: true,
        thumbnail_uri: poster.map(str::to_string),
        import_status: "imported".into(),
        imported_media_uri: None,
    }
}

fn loaded_clip(id: &str, poster: &str) -> EditorialClip {
    EditorialClip {
        clip_id: id.into(),
        name: id.into(),
        duration_seconds: 1.0,
        duration_frames: 25,
        imported: true,
        thumb_uri: Some(poster.into()),
        import_status: String::new(),
        imported_media_uri: String::new(),
        thumb_image: Some(std::sync::Arc::new(qnc_image_assets::RgbaImage {
            size: [1, 1],
            pixels: vec![0, 0, 0, 255],
            content_key: 1,
        })),
        ..Default::default()
    }
}

#[test]
fn a_poster_already_loaded_for_the_same_address_is_kept_when_the_list_changes() {
    let previous = vec![loaded_clip("a", "qnc://x/a.jpg")];
    let merged = merge_clips(
        vec![
            summary("a", Some("qnc://x/a.jpg")),
            summary("b", Some("qnc://x/b.jpg")),
        ],
        &previous,
    );
    assert!(merged[0].thumb_image.is_some());
    assert!(merged[1].thumb_image.is_none());
    assert_eq!(merged[1].thumb_uri.as_deref(), Some("qnc://x/b.jpg"));
}

#[test]
fn a_poster_at_another_address_is_loaded_again() {
    let previous = vec![loaded_clip("a", "qnc://x/card/a.jpg")];
    let merged = merge_clips(vec![summary("a", Some("qnc://x/project/a.jpg"))], &previous);
    assert!(
        merged[0].thumb_image.is_none(),
        "the project poster replaces the card poster"
    );
}

#[test]
fn a_clip_without_a_poster_address_keeps_the_placeholder() {
    let merged = merge_clips(vec![summary("a", None)], &[]);
    assert!(merged[0].thumb_uri.is_none() && merged[0].thumb_image.is_none());
}

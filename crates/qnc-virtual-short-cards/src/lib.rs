//! Public view model for virtual short cards.
//!
//! The rule is owned here: the IN still of a saved virtual short is the poster
//! shown by any form that uses virtual short cards. Forms do not derive that
//! rule, and application roots only pass DB rows and loaded poster rasters.

use std::sync::Arc;

pub const MODULE_ID: &str = "qnc.module.virtual-short-cards";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
pub struct ParentClip {
    pub duration_seconds: f64,
    pub duration_frames: u64,
    pub import_status: String,
    pub imported_media_uri: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualShortCard {
    pub shot_id: String,
    pub clip_id: String,
    pub name: String,
    pub in_frame: u64,
    pub out_frame: u64,
    /// v5 card duration: whole seconds and leftover frames, `seconds:ff`.
    pub duration_label: String,
    pub import_status: String,
    pub imported_media_uri: String,
    pub poster_uri: Option<String>,
    pub poster_image: Option<Arc<qnc_image_assets::RgbaImage>>,
}

pub fn build_card(
    row: qnc_virtual_shots::ShortClip,
    parent: Option<ParentClip>,
) -> VirtualShortCard {
    VirtualShortCard {
        shot_id: row.shot_id,
        clip_id: row.clip_id,
        name: row.name,
        in_frame: row.in_frame,
        out_frame: row.out_frame,
        duration_label: duration_label(row.out_frame.saturating_sub(row.in_frame), parent.as_ref()),
        import_status: parent
            .as_ref()
            .map(|clip| clip.import_status.clone())
            .unwrap_or_default(),
        imported_media_uri: parent
            .map(|clip| clip.imported_media_uri)
            .unwrap_or_default(),
        poster_uri: row.in_still_uri,
        poster_image: None,
    }
}

pub fn preserve_loaded_posters(cards: &mut [VirtualShortCard], previous: &[VirtualShortCard]) {
    for card in cards {
        card.poster_image = previous
            .iter()
            .find(|old| old.shot_id == card.shot_id && old.poster_uri == card.poster_uri)
            .and_then(|old| old.poster_image.clone());
    }
}

pub fn poster_requests(cards: &[VirtualShortCard]) -> Vec<(String, String)> {
    cards
        .iter()
        .filter(|card| card.poster_image.is_none())
        .filter_map(|card| Some((card.shot_id.clone(), card.poster_uri.clone()?)))
        .collect()
}

pub fn apply_poster(
    cards: &mut [VirtualShortCard],
    shot_id: &str,
    image: Arc<qnc_image_assets::RgbaImage>,
) -> bool {
    let Some(card) = cards.iter_mut().find(|card| card.shot_id == shot_id) else {
        return false;
    };
    card.poster_image = Some(image);
    true
}

fn duration_label(frames: u64, clip: Option<&ParentClip>) -> String {
    let Some(clip) = clip else {
        return "0:00".into();
    };
    if clip.duration_seconds <= 0.0 || clip.duration_frames == 0 {
        return "0:00".into();
    }
    let fps = (clip.duration_frames as f64 / clip.duration_seconds)
        .round()
        .max(1.0) as u64;
    let seconds = frames / fps;
    let rem = frames % fps;
    format!("{seconds}:{rem:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> qnc_virtual_shots::ShortClip {
        qnc_virtual_shots::ShortClip {
            shot_id: "shot-1".into(),
            clip_id: "clip-a".into(),
            in_frame: 25,
            out_frame: 75,
            name: "Mironik 001".into(),
            in_still_uri: Some("qnc://local/project/p1/virtual_shorts/shot-1/in.jpg".into()),
            out_still_uri: Some("qnc://local/project/p1/virtual_shorts/shot-1/out.jpg".into()),
            still_status: "ready".into(),
        }
    }

    #[test]
    fn in_still_is_the_public_card_poster() {
        let card = build_card(
            row(),
            Some(ParentClip {
                duration_seconds: 10.0,
                duration_frames: 250,
                import_status: "imported".into(),
                imported_media_uri: "qnc://media".into(),
            }),
        );
        assert_eq!(
            card.poster_uri.as_deref(),
            Some("qnc://local/project/p1/virtual_shorts/shot-1/in.jpg")
        );
        assert_eq!(card.duration_label, "2:00");
    }

    #[test]
    fn poster_requests_use_shot_identity() {
        let card = build_card(row(), None);
        assert_eq!(
            poster_requests(&[card]),
            vec![(
                "shot-1".to_string(),
                "qnc://local/project/p1/virtual_shorts/shot-1/in.jpg".to_string()
            )]
        );
    }
}

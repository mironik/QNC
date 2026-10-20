//! One clip row for every form, and what a media pool card shows of any item (`PoolItem`) (user 2026-10-20, rule 11: the Ingest and Story forms each
//! had their own clip type with its own card, poster and catalog code). A row says what the
//! media pool shows of a clip: name, duration, poster, import state and, where a form
//! selects several clips, its tick. It knows no form, reads no database and opens no media;
//! the pieces that fill it (catalog, Select, posters) implement their own traits on it.

use std::sync::Arc;

use qnc_image_assets::RgbaImage;
use serde::{Deserialize, Serialize};

pub const MODULE_ID: &str = "qnc.module.clip-row";

/// The dot in the top-right corner of a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolMarker {
    None,
    Imported,
    NotImported,
}

/// A line over the picture while the card's record is being written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolOverlay {
    None,
    Saving,
    SaveFailed,
}

/// What one card shows. The application's own clip or shot says it; the grid paints it.
pub trait PoolItem {
    fn card_id(&self) -> &str;
    fn card_title(&self) -> &str;
    /// Where the poster is and the poster once it is loaded.
    fn card_poster(&self) -> (Option<&str>, Option<&RgbaImage>);
    fn card_seconds(&self) -> f64 {
        0.0
    }
    /// A duration already written as text (shown instead of the seconds).
    fn card_duration_label(&self) -> &str {
        ""
    }
    /// The catalog status and the imported media, for the status dots.
    fn card_import(&self) -> (&str, &str) {
        ("", "")
    }
    fn card_marker(&self) -> PoolMarker {
        PoolMarker::None
    }
    fn card_overlay(&self) -> PoolOverlay {
        PoolOverlay::None
    }
    /// Ticked among several selected (where the pool shows a selection, not the chosen one).
    fn card_selected(&self) -> bool {
        false
    }
}


/// Whether the clip's record is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SaveState {
    Pending,
    Failed,
    #[default]
    Saved,
}

/// Whether its poster is loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThumbStatus {
    Ready,
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ClipRow {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub duration_frames: u64,
    /// Ticked for an action on several clips (Ingest's import).
    pub selected: bool,
    /// Import finished.
    pub imported: bool,
    /// The clip was in the catalog before the current Select.
    pub previously_seen: bool,
    pub metadata_revision: u32,
    pub save_state: SaveState,
    /// Where the poster is, as the project database says.
    pub thumb_uri: Option<String>,
    pub thumb_status: ThumbStatus,
    /// The poster once it is loaded.
    #[serde(skip)]
    pub thumb_image: Option<Arc<RgbaImage>>,
    /// The catalog record status and imported media, for the status dots.
    pub import_status: String,
    pub imported_media_uri: String,
}

impl ClipRow {
    /// A row of the project catalog's summary of an imported clip.
    pub fn from_summary(clip: qnc_content_read::ClipSummary) -> Self {
        Self {
            clip_id: clip.clip_id,
            name: clip.name,
            duration_seconds: clip.duration_seconds,
            duration_frames: clip.duration_frames,
            imported: clip.imported,
            thumb_uri: clip.thumbnail_uri,
            import_status: clip.import_status,
            imported_media_uri: clip.imported_media_uri.unwrap_or_default(),
            ..Self::default()
        }
    }
}

impl PoolItem for ClipRow {
    fn card_id(&self) -> &str {
        &self.clip_id
    }

    fn card_title(&self) -> &str {
        &self.name
    }

    fn card_poster(&self) -> (Option<&str>, Option<&RgbaImage>) {
        (self.thumb_uri.as_deref(), self.thumb_image.as_deref())
    }

    fn card_seconds(&self) -> f64 {
        self.duration_seconds
    }

    fn card_import(&self) -> (&str, &str) {
        (&self.import_status, &self.imported_media_uri)
    }

    fn card_marker(&self) -> PoolMarker {
        if self.imported {
            PoolMarker::Imported
        } else {
            PoolMarker::NotImported
        }
    }

    fn card_overlay(&self) -> PoolOverlay {
        match self.save_state {
            SaveState::Pending => PoolOverlay::Saving,
            SaveState::Failed => PoolOverlay::SaveFailed,
            SaveState::Saved => PoolOverlay::None,
        }
    }

    fn card_selected(&self) -> bool {
        self.selected
    }
}

impl qnc_media_thumbnail::ThumbnailItem for ClipRow {
    fn item_id(&self) -> &str {
        &self.clip_id
    }

    fn thumbnail_uri(&self) -> Option<&str> {
        self.thumb_uri.as_deref()
    }

    fn set_thumbnail_ready(&mut self, image: Arc<RgbaImage>) {
        self.thumb_image = Some(image);
        self.thumb_status = ThumbStatus::Ready;
    }
}

impl qnc_media_thumbnail::LoadedThumbnail for ClipRow {
    fn thumbnail_image(&self) -> Option<&Arc<RgbaImage>> {
        self.thumb_image.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_of_the_catalog_summary_and_its_card() {
        let row = ClipRow::from_summary(qnc_content_read::ClipSummary {
            clip_id: "c1".into(),
            name: "Clip 1".into(),
            duration_seconds: 2.0,
            duration_frames: 100,
            imported: true,
            thumbnail_uri: Some("qnc://poster".into()),
            import_status: "imported".into(),
            imported_media_uri: None,
        });
        assert_eq!((row.card_id(), row.card_title(), row.card_seconds()), ("c1", "Clip 1", 2.0));
        assert_eq!(row.card_import(), ("imported", ""));
        assert_eq!(row.card_marker(), PoolMarker::Imported);
        assert_eq!(row.card_overlay(), PoolOverlay::None);
        assert_eq!(row.card_poster().0, Some("qnc://poster"));
    }
}

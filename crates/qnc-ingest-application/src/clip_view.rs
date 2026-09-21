use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipView {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub selected: bool,
    pub imported: bool,
    pub previously_seen: bool,
    pub metadata_revision: u32,
    pub save_state: SaveState,
    pub thumb_uri: Option<String>,
    pub thumb_status: ThumbStatus,
    #[serde(skip)]
    pub thumb_image: Option<std::sync::Arc<qnc_image_assets::RgbaImage>>,
}

impl Default for ClipView {
    fn default() -> Self {
        Self {
            clip_id: String::new(),
            name: String::new(),
            duration_seconds: 0.0,
            selected: false,
            imported: false,
            previously_seen: false,
            metadata_revision: 0,
            save_state: SaveState::Saved,
            thumb_uri: None,
            thumb_status: ThumbStatus::Missing,
            thumb_image: None,
        }
    }
}

impl From<selection::SelectedClip> for ClipView {
    fn from(clip: selection::SelectedClip) -> Self {
        Self {
            clip_id: clip.clip_id,
            name: clip.name,
            duration_seconds: clip.duration_seconds,
            selected: clip.selected,
            imported: clip.imported,
            previously_seen: clip.previously_seen,
            metadata_revision: clip.metadata_revision,
            save_state: match clip.save_state {
                selection::SelectSaveState::Pending => SaveState::Pending,
                selection::SelectSaveState::Failed => SaveState::Failed,
                selection::SelectSaveState::Saved => SaveState::Saved,
            },
            thumb_uri: clip.thumb_uri,
            thumb_status: match clip.thumb_status {
                selection::SelectThumbStatus::Ready => ThumbStatus::Ready,
                selection::SelectThumbStatus::Pending => ThumbStatus::Pending,
                selection::SelectThumbStatus::Missing => ThumbStatus::Missing,
            },
            thumb_image: clip.thumb_image,
        }
    }
}

impl From<catalog::CatalogClipRow> for ClipView {
    fn from(clip: catalog::CatalogClipRow) -> Self {
        Self {
            clip_id: clip.clip_id,
            name: clip.name,
            duration_seconds: clip.duration_seconds,
            selected: clip.selected,
            imported: clip.imported,
            previously_seen: clip.previously_seen,
            metadata_revision: clip.metadata_revision,
            save_state: SaveState::Saved,
            thumb_uri: clip.thumb_uri,
            thumb_status: match clip.thumb_status {
                catalog::CatalogThumbStatus::Pending => ThumbStatus::Pending,
                catalog::CatalogThumbStatus::Missing => ThumbStatus::Missing,
            },
            thumb_image: None,
        }
    }
}

impl catalog::CatalogClipItem for ClipView {
    fn catalog_clip_id(&self) -> &str {
        &self.clip_id
    }

    fn catalog_selected(&self) -> bool {
        self.selected
    }

    fn from_catalog_row(row: catalog::CatalogClipRow) -> Self {
        Self::from(row)
    }
}

impl qnc_ingest_clip_list::ClipListItem for ClipView {
    fn clip_id(&self) -> &str {
        &self.clip_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn selected(&self) -> bool {
        self.selected
    }

    fn metadata_revision(&self) -> u32 {
        self.metadata_revision
    }

    fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    fn set_previously_seen(&mut self, seen: bool) {
        self.previously_seen = seen;
    }

    fn set_save_failed(&mut self, failed: bool) {
        self.save_state = if failed {
            SaveState::Failed
        } else {
            SaveState::Saved
        };
    }

    fn from_selected_clip(clip: selection::SelectedClip) -> Self {
        Self::from(clip)
    }
}

impl qnc_media_thumbnail::ThumbnailItem for ClipView {
    fn item_id(&self) -> &str {
        &self.clip_id
    }

    fn thumbnail_uri(&self) -> Option<&str> {
        self.thumb_uri.as_deref()
    }

    fn set_thumbnail_ready(&mut self, image: std::sync::Arc<qnc_image_assets::RgbaImage>) {
        self.thumb_image = Some(image);
        self.thumb_status = ThumbStatus::Ready;
    }
}

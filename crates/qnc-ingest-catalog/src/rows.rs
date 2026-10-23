//! The rows of the catalog and how they go onto the clip list a form shows (its marks and
//! the clips new to this Select stay).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CatalogThumbStatus {
    Pending,
    #[default]
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatalogClipRow {
    pub clip_id: String,
    pub name: String,
    pub duration_seconds: f64,
    pub selected: bool,
    pub imported: bool,
    pub previously_seen: bool,
    pub metadata_revision: u32,
    pub thumb_uri: Option<String>,
    pub thumb_status: CatalogThumbStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThumbnailRequest {
    pub clip_id: String,
    pub thumb_uri: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AppliedCatalogClips<T> {
    pub clips: Vec<T>,
    pub thumbnail_requests: Vec<ThumbnailRequest>,
}

pub trait CatalogClipItem: Sized {
    fn catalog_clip_id(&self) -> &str;
    fn catalog_selected(&self) -> bool;
    /// The current Select found this clip new (it was not in the database before):
    /// a reload of the catalog keeps it new until the next Select decides again.
    fn catalog_new(&self) -> bool {
        false
    }
    fn from_catalog_row(row: CatalogClipRow) -> Self;
}

pub fn apply_catalog_rows<T: CatalogClipItem>(
    current: &[T],
    rows: Vec<CatalogClipRow>,
) -> AppliedCatalogClips<T> {
    let selected = current
        .iter()
        .map(|clip| (clip.catalog_clip_id().to_string(), (clip.catalog_selected(), clip.catalog_new())))
        .collect::<std::collections::HashMap<_, _>>();
    let mut thumbnail_requests = Vec::new();
    let clips = rows
        .into_iter()
        .map(|mut row| {
            if let Some((marked, new)) = selected.get(row.clip_id.as_str()) {
                row.selected = *marked;
                row.previously_seen &= !new;
            }
            if let Some(uri) = row.thumb_uri.as_ref() {
                thumbnail_requests.push(ThumbnailRequest {
                    clip_id: row.clip_id.clone(),
                    thumb_uri: uri.clone(),
                });
            }
            T::from_catalog_row(row)
        })
        .collect();
    AppliedCatalogClips {
        clips,
        thumbnail_requests,
    }
}

/// The clip row every form shows (piece `qnc-clip-row`), filled from the catalog.
impl CatalogClipItem for qnc_clip_row::ClipRow {
    fn catalog_clip_id(&self) -> &str {
        &self.clip_id
    }

    fn catalog_selected(&self) -> bool {
        self.selected
    }

    fn catalog_new(&self) -> bool {
        !self.previously_seen // new for this Select, kept on reload
    }

    fn from_catalog_row(row: CatalogClipRow) -> Self {
        qnc_clip_row::ClipRow {
            clip_id: row.clip_id,
            name: row.name,
            duration_seconds: row.duration_seconds,
            selected: row.selected,
            imported: row.imported,
            previously_seen: row.previously_seen,
            metadata_revision: row.metadata_revision,
            thumb_uri: row.thumb_uri,
            thumb_status: match row.thumb_status {
                CatalogThumbStatus::Pending => qnc_clip_row::ThumbStatus::Pending,
                CatalogThumbStatus::Missing => qnc_clip_row::ThumbStatus::Missing,
            },
            ..Default::default()
        }
    }
}

//! Public project content database. This module neither scans nor probes media.
mod database;
mod transport;
pub use database::ContentStore;
pub use qnc_json_transport::{Access, Credentials};
use qnc_media_records::{Phase, Snapshot};
pub use qnc_wave::WaveArtifactRecord;
use serde::{Deserialize, Serialize};
pub use transport::{
    respond, ContentClient, ContentTarget, ContentWriteCompletion, ContentWriteData,
    ContentWriteResult, ContentWriteTransport, ENDPOINT,
};

pub const VERSION: &str = "0.2.4";
pub const SCHEMA_VERSION: &str = "0.2.0";
pub const MAX_BYTES: usize = 16 * 1024 * 1024;
pub const PAGE_SIZE: usize = 64;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogClip {
    pub name: String,
    pub source_uri: String,
    pub source_name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub thumbnail_uri: Option<String>,
    pub media_records_uri: String,
    pub snapshot: Snapshot,
}

impl CatalogClip {
    pub fn id(&self) -> &str {
        &self.snapshot.metadata.clip_id
    }
    pub fn validate(&self) -> Result<()> {
        self.snapshot.validate().map_err(|e| e.to_string())?;
        qnc_media_records::validate_db_uri(&self.media_records_uri).map_err(|e| e.to_string())?;
        let source = qnc_contracts::parse_qnc_uri(&self.source_uri).map_err(|e| e.to_string())?;
        if source.resource_kind != "source" || self.name.trim().is_empty() || self.name.len() > 1024
        {
            return Err("Neispravan zapis klipa.".into());
        }
        for uri in std::iter::once(&self.snapshot.binding.original_uri)
            .chain(self.snapshot.binding.proxy_uri.iter())
        {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
            if !uri.starts_with(&format!("{}/", self.source_uri.trim_end_matches('/'))) {
                return Err("Medij ne pripada zapisanom izvoru.".into());
            }
        }
        if let Some(uri) = &self.thumbnail_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
            let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
            if parsed.resource_kind != "source" && parsed.resource_kind != "project" {
                return Err("Poster ne pripada izvoru ili projektu.".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Detected,
    Queued,
    Processing,
    Imported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredClip {
    pub clip: CatalogClip,
    pub selected: bool,
    pub import_status: ImportStatus,
    pub import_error: Option<String>,
    pub imported_media_uri: Option<String>,
}

/// Lightweight UI/catalog row. This is intentionally not enough for playback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredClipSummary {
    pub clip_id: String,
    pub name: String,
    pub source_uri: String,
    pub source_name: String,
    pub serial_number: String,
    pub volume_name: String,
    pub thumbnail_uri: Option<String>,
    pub duration_seconds: f64,
    pub selected: bool,
    pub import_status: ImportStatus,
    pub import_error: Option<String>,
    pub imported_media_uri: Option<String>,
    pub revision: u32,
    pub final_record: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripFrameRecord {
    pub index: usize,
    pub seek_sec: String,
    pub artifact_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilmstripArtifactRecord {
    pub clip_id: String,
    pub status: String,
    pub duration_sec: String,
    pub frame_count: usize,
    pub artifact_uri: String,
    pub frames: Vec<FilmstripFrameRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedShort {
    pub shot_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
}

/// One virtual short stored in the project DB, ordered by creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortClip {
    pub shot_id: String,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub name: String,
    pub in_still_uri: Option<String>,
    pub out_still_uri: Option<String>,
    pub still_status: String,
}

/// One segment of the edited program. Stored kind is `tonovi` or `offovi`.
/// Frames are source frames of its clip in the timebase `fps_num/fps_den`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramSegment {
    pub segment_id: String,
    pub kind: String,
    pub sort_index: u32,
    pub clip_id: String,
    pub in_frame: u64,
    pub out_frame: u64,
    pub fps_num: u32,
    pub fps_den: u32,
    /// False once deleted (v5 keeps the row, `active = 0`): shown greyed in the
    /// Segment tab, not part of the program.
    pub active: bool,
    /// Source channel heard on A1 (zero based), chosen on the Wrap segment;
    /// channel 1 (0) unless the user picks another one.
    #[serde(default)]
    pub a1_source_channel: u16,
}

/// An M marker on the program axis (v5 `story_markers`). `system_role` is
/// `program_start`, `program_end` (locked) or empty for a user marker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramMarker {
    pub marker_id: String,
    pub program_frame: u64,
    pub system_role: String,
}

/// An M-M slot (v5 `story_marker_slots`): `slot_id` is the marker pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramSlot {
    pub slot_id: String,
    pub start_frame: u64,
    pub end_frame: u64,
    pub start_marker_id: String,
    pub end_marker_id: String,
    pub has_cover: bool,
}

/// A cover (v5 `story_covers`) bound to its M-M slot: program frames, the
/// source frames of its clip and the source channel heard on A2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProgramCover {
    pub cover_id: String,
    pub slot_id: String,
    pub clip_id: String,
    pub virtual_shot_id: String,
    pub program_start_frame: u64,
    pub program_end_frame: u64,
    pub source_in_frame: u64,
    pub source_out_frame: u64,
    pub fps_num: u32,
    pub fps_den: u32,
    pub a2_source_channel: u16,
}

/// The stored Story selection (v5 `story_state`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StorySelection {
    pub selected_part_id: String,
    pub selected_slot_id: String,
    pub selected_cover_id: String,
}

/// Lightweight catalog signature for deciding whether a visible catalog is stale.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogStats {
    pub clip_count: u64,
    pub selected_count: u64,
    pub revision_sum: u64,
    pub max_revision: u32,
    pub fingerprint: u64,
}

impl StoredClipSummary {
    pub fn validate(&self) -> Result<()> {
        qnc_media_records::valid_id(&self.clip_id).map_err(|e| e.to_string())?;
        let source = qnc_contracts::parse_qnc_uri(&self.source_uri).map_err(|e| e.to_string())?;
        if source.resource_kind != "source" || self.name.trim().is_empty() {
            return Err("Neispravan sazetak klipa.".into());
        }
        if let Some(uri) = &self.thumbnail_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
        }
        if let Some(uri) = &self.imported_media_uri {
            qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

/// Lightweight reconciliation facts, without reloading probe JSON or thumbnails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventoryClip {
    pub clip_id: String,
    pub source_uri: String,
    pub original_uri: String,
    pub revision: u32,
    pub final_record: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(
    tag = "action",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Operation {
    Publish(Box<CatalogClip>),
    PublishBatch(Vec<CatalogClip>),
    Inventory {
        source_uri: String,
        after: Option<String>,
    },
    RemoveMissing {
        clips: Vec<InventoryClip>,
    },
    List {
        after: Option<String>,
    },
    ListSummary {
        after: Option<String>,
    },
    Stats,
    Read {
        clip_id: String,
    },
    PublishFilmstrip(Box<FilmstripArtifactRecord>),
    ReadFilmstrip {
        clip_id: String,
    },
    PublishWave(Box<WaveArtifactRecord>),
    ReadWave {
        clip_id: String,
    },
    Select {
        clip_ids: Vec<String>,
        selected: bool,
    },
    QueueSelected,
    ClaimNext,
    /// The importer still works on this clip: renews its lease.
    Heartbeat {
        clip_id: String,
    },
    /// What one process or form tells the others (playback, worker lease, result).
    SetRuntime {
        key: String,
        value: String,
    },
    GetRuntime {
        key: String,
    },
    FinishImport {
        clip_id: String,
        media_uri: Option<String>,
        /// The poster copied into the project together with the media, if any.
        #[serde(default)]
        thumbnail_uri: Option<String>,
        error: Option<String>,
    },
    SaveShort {
        project_id: String,
        clip_id: String,
        clip_name: String,
        in_frame: u64,
        out_frame: u64,
    },
    ListShorts,
    MarkShortStills {
        shot_id: String,
        status: String,
        in_uri: Option<String>,
        out_uri: Option<String>,
        error: Option<String>,
    },
    /// Appends a segment at the end of the program.
    CreateSegment {
        project_id: String,
        kind: String,
        clip_id: String,
        in_frame: u64,
        out_frame: u64,
        fps_num: u32,
        fps_den: u32,
    },
    DeleteSegment {
        segment_id: String,
    },
    /// Swaps the segment with its neighbour before (`up`) or after it.
    MoveSegment {
        segment_id: String,
        up: bool,
    },
    ListSegments,
    /// M placed on a Wrap segment (v5 `create_marker_from_part_frame`): the frame
    /// inside the segment `part_id`; the program frame follows from it. A marker
    /// already on that program frame is refreshed, not duplicated.
    CreateMarker {
        part_id: String,
        local_frame: u64,
    },
    MoveMarker {
        marker_id: String,
        program_frame: u64,
    },
    DeleteMarker {
        marker_id: String,
    },
    ListMarkers,
    ListSlots,
    ListCovers,
    ReadStorySelection,
    /// v5 `select_part`.
    SelectPart {
        part_id: String,
    },
    /// v5 `select_marker_slot`.
    SelectSlot {
        slot_id: String,
    },
    /// v5 `create_cover` from source frames: one write makes the B-roll virtual
    /// shot of the source IN/OUT and the cover of the slot, replacing a cover
    /// already there, and selects it. A2 starts on source channel 1.
    CreateCover {
        project_id: String,
        slot_id: String,
        clip_id: String,
        clip_name: String,
        in_frame: u64,
        out_frame: u64,
        fps_num: u32,
        fps_den: u32,
    },
    /// v5 `delete_cover`; its B-roll virtual shot stays.
    DeleteCover {
        cover_id: String,
    },
    /// v5 `select_cover`.
    SelectCover {
        cover_id: String,
    },
}
impl Operation {
    pub fn is_write(&self) -> bool {
        !matches!(
            self,
            Self::List { .. }
                | Self::ListSummary { .. }
                | Self::Stats
                | Self::Inventory { .. }
                | Self::Read { .. }
                | Self::ReadFilmstrip { .. }
                | Self::ReadWave { .. }
                | Self::GetRuntime { .. }
                | Self::ListShorts
                | Self::ListSegments
                | Self::ListMarkers
                | Self::ListSlots
                | Self::ListCovers
                | Self::ReadStorySelection
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: String,
    pub db_uri: String,
    pub operation: Operation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Data {
    Saved(Box<StoredClip>),
    Claimed(Option<Box<StoredClip>>),
    Clips(Vec<StoredClip>),
    ClipSummaries(Vec<StoredClipSummary>),
    CatalogStats(CatalogStats),
    Clip(Option<Box<StoredClip>>),
    Filmstrip(Option<Box<FilmstripArtifactRecord>>),
    Wave(Option<Box<WaveArtifactRecord>>),
    Inventory(Vec<InventoryClip>),
    Removed(Vec<String>),
    Runtime(Option<RuntimeEntry>),
    SavedShort(Box<SavedShort>),
    ShortClips(Vec<ShortClip>),
    Created(String),
    Segments(Vec<ProgramSegment>),
    Markers(Vec<ProgramMarker>),
    Slots(Vec<ProgramSlot>),
    Covers(Vec<ProgramCover>),
    StorySelection(StorySelection),
    Changed,
}

/// A runtime entry and how old it is by the clock of the database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeEntry {
    pub value: String,
    pub age_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: String,
    pub db_uri: String,
    pub result: Result<Data>,
}

pub fn content_uri(workspace_uri: &str) -> Result<String> {
    let uri = qnc_contracts::parse_qnc_uri(workspace_uri).map_err(|e| e.to_string())?;
    if uri.resource_kind != "db" || !uri.resource_id.starts_with("project_workspace/") {
        return Err("Nedostaje projektna baza iz radnih postavki.".into());
    }
    Ok(workspace_uri.replacen("/db/project_workspace/", "/db/ingest_content/", 1))
}

pub(crate) fn project_id(uri: &str) -> Result<String> {
    let parsed = qnc_contracts::parse_qnc_uri(uri).map_err(|e| e.to_string())?;
    if parsed.resource_kind != "db" {
        return Err("Neispravan DB URI.".into());
    }
    let id = parsed
        .resource_id
        .strip_prefix("ingest_content/")
        .ok_or("Nedostaje projektni identitet.")?;
    if id.is_empty() || id.contains(['/', '\\', ':']) || matches!(id, "." | "..") {
        return Err("Neispravan projektni identitet.".into());
    }
    Ok(id.into())
}

/// A clip can be imported when its metadata is final and either complete or declared
/// by the card: a final record without probe evidence is never probed to fill it, so
/// waiting for more would wait forever. A probed record that is still partial stays
/// blocked.
pub(crate) fn ready(clip: &StoredClip) -> bool {
    let snapshot = &clip.clip.snapshot;
    snapshot.phase == Phase::Final
        && (snapshot.completeness == qnc_media_records::Completeness::Complete
            || !snapshot
                .metadata
                .evidence
                .iter()
                .any(|e| e.kind == qnc_media_records::EvidenceKind::Ffprobe))
}

#[cfg(test)]
mod tests;

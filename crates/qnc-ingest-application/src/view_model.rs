use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestViewModel {
    #[serde(skip)]
    pub playback: qnc_player_client::View,
    #[serde(skip)]
    pub timeline: TimelineProjection,
    #[serde(skip)]
    pub timeline_assets: SourceTimelineAssets,
    pub source_kind: SourceKind,
    pub browser_roots: bool,
    pub browser_path_label: String,
    pub browser_current_uri: Option<String>,
    pub browser_parent_available: bool,
    pub browser_entries: Vec<LocationEntry>,
    pub browser_busy: bool,
    pub browser_error: Option<String>,
    pub selected_source_uri: Option<String>,
    pub selected_source_name: String,
    pub selected_source_serial_number: String,
    pub selected_source_volume_name: String,
    pub clips: Vec<ClipView>,
    pub clip_filter: ClipFilter,
    pub preview_clip_id: Option<String>,
    pub archive_original: bool,
    pub archive_original_available: bool,
    pub ai_mining: bool,
    pub command_busy: bool,
    pub select_warning_count: usize,
    pub work_settings_loading: bool,
    pub work_settings_ready: bool,
    pub work_settings_error: Option<String>,
    pub message: String,
}

impl Default for IngestViewModel {
    fn default() -> Self {
        Self {
            source_kind: SourceKind::Local,
            playback: Default::default(),
            timeline: Default::default(),
            timeline_assets: SourceTimelineAssets::empty(),
            browser_roots: true,
            browser_path_label: String::new(),
            browser_current_uri: None,
            browser_parent_available: false,
            browser_entries: Vec::new(),
            browser_busy: false,
            browser_error: None,
            selected_source_uri: None,
            selected_source_name: String::new(),
            selected_source_serial_number: String::new(),
            selected_source_volume_name: String::new(),
            clips: Vec::new(),
            clip_filter: ClipFilter::All,
            preview_clip_id: None,
            archive_original: false,
            archive_original_available: false,
            ai_mining: false,
            command_busy: false,
            select_warning_count: 0,
            work_settings_loading: false,
            work_settings_ready: false,
            work_settings_error: None,
            message: "Odaberi izvor.".to_string(),
        }
    }
}

impl IngestViewModel {
    pub fn visible_clips(&self) -> impl Iterator<Item = &ClipView> {
        self.clips
            .iter()
            .filter(|clip| self.clip_filter == ClipFilter::All || !clip.previously_seen)
    }

    pub fn total_count(&self) -> usize {
        self.clips.len()
    }

    pub fn selected_count(&self) -> usize {
        self.clips.iter().filter(|clip| clip.selected).count()
    }

    pub fn imported_count(&self) -> usize {
        self.clips.iter().filter(|clip| clip.imported).count()
    }

    pub fn pending_count(&self) -> usize {
        self.total_count().saturating_sub(self.imported_count())
    }

    pub fn current_clip_label(&self) -> &str {
        self.preview_clip_id
            .as_deref()
            .and_then(|clip_id| {
                self.clips
                    .iter()
                    .find(|clip| clip.clip_id == clip_id)
                    .map(|clip| clip.name.as_str())
            })
            .unwrap_or("Odaberi klip")
    }

    pub fn status_label(&self) -> String {
        if self.select_warning_count > 0 && !self.command_busy {
            return format!(
                "{} klipova; {} upozorenja",
                self.clips.len(),
                self.select_warning_count
            );
        }
        if self.command_busy {
            return format!("Select: {} klipova", self.clips.len());
        }
        let imported = self.imported_count();
        let pending = self.pending_count();
        let selected = self.selected_count();
        let total = self.total_count();
        if pending > 0 {
            format!("{imported} uvezeno · {pending} nije uvezeno · {selected}/{total}")
        } else {
            format!("{imported} uvezeno · {selected}/{total}")
        }
    }

    pub fn proxy_poster_approval_count(&self) -> usize {
        self.clips
            .iter()
            .filter(|clip| clip.selected && matches!(clip.thumb_status, ThumbStatus::Missing))
            .count()
    }

    pub fn timeline_filmstrip_background(
        &self,
    ) -> Option<&qnc_timeline_assets::FilmstripBackground> {
        self.timeline_assets.filmstrip_background()
    }

    pub fn timeline_a1_peaks(&self) -> &[f32] {
        self.timeline_assets.a1_peaks()
    }

    pub fn timeline_a2_peaks(&self) -> &[f32] {
        self.timeline_assets.a2_peaks()
    }

    pub fn timeline_a3_peaks(&self) -> &[f32] {
        self.timeline_assets.a3_peaks()
    }

    pub fn timeline_a4_peaks(&self) -> &[f32] {
        self.timeline_assets.a4_peaks()
    }
}

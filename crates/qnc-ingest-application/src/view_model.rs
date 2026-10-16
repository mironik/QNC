use super::*;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngestViewModel {
    /// The panel the keyboard acts on, the dock button and the last card grid step.
    #[serde(skip)]
    pub focus: qnc_focus_walk::WalkView,
    /// Original timecode of the previewed clip (camera XML, else the one probe).
    #[serde(skip)]
    pub source_timecode: Option<qnc_source_timecode::SourceTimecode>,
    #[serde(skip)]
    pub playback: qnc_player_client::View,
    #[serde(skip)]
    pub timeline: TimelineProjection,
    #[serde(skip)]
    pub timeline_assets: SourceTimelineAssets,
    pub source_kind: SourceKind,
    /// The source browser as its piece shows it.
    pub browser: qnc_source_browse::BrowserView,
    pub selected_source_uri: Option<String>,
    pub clips: Vec<ClipView>,
    pub clip_filter: ClipFilter,
    pub pool_tab: Option<String>, // chosen pool head tab (action id); None = the first
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
            focus: Default::default(),
            source_timecode: None,
            playback: Default::default(),
            timeline: Default::default(),
            timeline_assets: SourceTimelineAssets::empty(),
            browser: Default::default(),
            selected_source_uri: None,
            clips: Vec::new(),
            clip_filter: ClipFilter::All,
            pool_tab: None,
            preview_clip_id: None,
            archive_original: false,
            archive_original_available: false,
            ai_mining: false,
            command_busy: false,
            select_warning_count: 0,
            work_settings_loading: false,
            work_settings_ready: false,
            work_settings_error: None,
            message: qnc_ui_text::text("ingest.msg.pick_source").to_string(),
        }
    }
}

impl IngestViewModel {
    pub fn action_enabled(&self, action_id: &str) -> bool {
        match action_id {
            action_ids::INGEST_IMPORT_SELECTED => self.selected_count() > 0 && !self.command_busy,
            action_ids::INGEST_CLEAR_SELECTION => self.visible_clips().any(|clip| clip.selected),
            action_ids::INGEST_SELECT_ALL => self.visible_clips().next().is_some(),
            _ => !self.command_busy,
        }
    }

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
            .unwrap_or(qnc_ui_text::text("ingest.preview.empty"))
    }

    pub fn status_label(&self) -> String {
        let (clips, warnings) = (self.clips.len().to_string(), self.select_warning_count.to_string());
        if self.select_warning_count > 0 && !self.command_busy {
            return qnc_ui_text::fill("ingest.status.warnings", &[("clips", &clips), ("warnings", &warnings)]);
        }
        if self.command_busy {
            return qnc_ui_text::fill("ingest.status.select", &[("clips", &clips)]);
        }
        let [imported, pending, selected, total] = [self.imported_count(), self.pending_count(), self.selected_count(), self.total_count()].map(|count| count.to_string());
        let code = if self.pending_count() > 0 { "ingest.status.pending" } else { "ingest.status.idle" };
        qnc_ui_text::fill(code, &[("imported", &imported), ("pending", &pending), ("selected", &selected), ("total", &total)])
    }
}

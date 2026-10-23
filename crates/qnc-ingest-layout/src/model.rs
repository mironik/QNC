use serde::Deserialize;


#[derive(Debug, Clone, Deserialize)]
pub struct IngestLayoutContract {
    pub layout_id: String,
    pub application_id: String,
    /// The panels Tab walks, in order (user 2026-10-13: the walk belongs to the form).
    pub tab_order: Vec<String>,
    /// The row panels and their action ids left to right.
    pub focus_rows: std::collections::BTreeMap<String, Vec<String>>,
    /// The click actions of each panel: a click there puts the keyboard on that panel.
    pub focus_clicks: std::collections::BTreeMap<String, Vec<String>>,
    pub board: IngestBoard,
    pub preview: IngestPreviewPane,
    pub pool_head: IngestPoolHead,
    pub dir_browser: IngestDirBrowser,
    pub clip_grid: IngestClipGrid,
    pub source_dock: IngestSourceDock,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestBoard {
    pub left_ratio: f32,
    pub divider_width: f32,
    pub left_min_width: f32,
    pub right_min_width: f32,
    pub shell_margin_x: f32,
    pub block_pad: f32,
    #[allow(dead_code)]
    pub gap: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestPreviewPane {
    pub min_height: f32,
    pub reserve_below: f32,
    pub width_inset: f32,
    pub min_width: f32,
    pub aspect: String,
    pub empty_label: String,
}

impl IngestPreviewPane {
    pub fn aspect_ratio(&self) -> f32 {
        let Some((width, height)) = self.aspect.split_once(':') else {
            return 16.0 / 9.0;
        };
        let width = width.parse::<f32>().unwrap_or(16.0);
        let height = height.parse::<f32>().unwrap_or(9.0);
        (width / height.max(0.1)).max(0.1)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestPoolHead {
    pub tabs_left: Vec<TransportCommand>,
    pub tab_gap: f32,
    /// Width of the transport panel's place at the right of the row under the monitor.
    pub transport_width: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TransportCommand {
    Action { label: String, action_id: String },
    Label(String),
}

impl TransportCommand {
    pub fn label(&self) -> &str {
        match self {
            Self::Action { label, .. } | Self::Label(label) => label,
        }
    }

    pub fn action_id(&self) -> Option<&str> {
        match self {
            Self::Action { action_id, .. } => Some(action_id),
            Self::Label(_) => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestDirBrowser {
    pub sources_label: String,
    pub kinds: Vec<String>,
    pub up_label: String,
    pub disks_label: String,
    pub confirm_label: String,
    pub cancel_label: String,
    pub empty_lan: String,
    pub empty_internet: String,
    pub empty_local: String,
    pub empty_folder: String,
    pub loading_label: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestClipGrid {
    pub min_card_width: f32,
    pub card_text_height: f32,
    pub grid_gap: f32,
    pub empty_message: String,
    pub empty_new_message: String,
    pub loading_settings_message: String,
    pub saving_label: String,
    pub save_failed_label: String,
    pub imported_marker_rgb: [u8; 3],
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestSourceDock {
    pub clip_label_fallback: String,
    pub actions_rtl: Vec<TransportCommand>,
    pub clip_filter_labels: [String; 2],
    pub clip_filter_colors: [[u8; 3]; 2],
    pub clip_filter_width: f32,
    pub header_timeline_gap: f32,
    pub show_edit_actions: bool,
    pub show_import_actions: bool,
    pub archive_original_label: String,
    pub edit_actions: Vec<String>,
    pub ai_mining_label: String,
}

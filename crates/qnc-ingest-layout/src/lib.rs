//! The model of the Ingest layout contract (moved unchanged out of the Ingest form,
//! user rule 2026-09-30: forms are boards of public pieces) and the Ingest places on
//! the one desktop board.

use qnc_keyboard_shortcut::ShortcutCatalog;
use serde::Deserialize;

pub use qnc_desktop_theme as theme;
pub use qnc_desktop_theme::{ShellColors, ShellLayoutContract, ShellThemeMetrics};

const SHELL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/shell.layout.json");
const INGEST_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/ingest.layout.json");
const INGEST_APP_JSON: &str =
    include_str!("../../../contracts/applications/ingest.application.json");
const KEYBOARD_SHORTCUTS_JSON: &str =
    include_str!("../../../contracts/qnc-keyboard-shortcuts.json");

#[derive(Debug, Clone)]
pub struct IngestContracts {
    pub shell: ShellLayoutContract,
    pub ingest: IngestLayoutContract,
    pub shortcuts: ShortcutCatalog,
    pub shortcuts_loaded: bool,
}

impl IngestContracts {
    pub fn load() -> Result<Self, String> {
        report_to_result(qnc_contracts::validate_ui_layout_contract_json(
            "contracts/ui/shell.layout.json",
            SHELL_LAYOUT_JSON,
        ))?;
        report_to_result(qnc_contracts::validate_ui_layout_contract_json(
            "contracts/ui/ingest.layout.json",
            INGEST_LAYOUT_JSON,
        ))?;
        report_to_result(qnc_contracts::validate_application_manifest_json(
            "contracts/applications/ingest.application.json",
            INGEST_APP_JSON,
        ))?;

        let shell: ShellLayoutContract = serde_json::from_str(SHELL_LAYOUT_JSON)
            .map_err(|error| format!("shell layout parse error: {error}"))?;
        let ingest: IngestLayoutContract = serde_json::from_str(INGEST_LAYOUT_JSON)
            .map_err(|error| format!("ingest layout parse error: {error}"))?;
        let app: ApplicationManifest = serde_json::from_str(INGEST_APP_JSON)
            .map_err(|error| format!("ingest application parse error: {error}"))?;

        let shortcuts = ShortcutCatalog::from_json_str(KEYBOARD_SHORTCUTS_JSON)
            .map_err(|error| format!("keyboard shortcut catalog parse error: {error}"))?;
        let shortcuts_loaded = !shortcuts.actions.is_empty();

        if shell.layout_id != "qnc.ui.shell" {
            return Err(format!("unexpected shell layout_id {}", shell.layout_id));
        }
        if ingest.layout_id != "qnc.ui.ingest" {
            return Err(format!("unexpected ingest layout_id {}", ingest.layout_id));
        }
        if ingest.application_id != "qnc.ingest" || app.application_id != "qnc.ingest" {
            return Err("ingest application_id mismatch".to_string());
        }
        if ingest.board.left_ratio <= 0.0 || ingest.board.left_ratio >= 1.0 {
            return Err("ingest board left_ratio must split the desktop".to_string());
        }
        if ingest.board.shell_margin_x < 0.0 {
            return Err("ingest shell_margin_x must not be negative".to_string());
        }
        if ingest.source_dock.actions_rtl.is_empty() {
            return Err("ingest source dock must declare actions".to_string());
        }

        Ok(Self {
            shell,
            ingest,
            shortcuts,
            shortcuts_loaded,
        })
    }

    pub fn dock_height(&self) -> f32 {
        let timeline_height = 15.0 + 3.0 + 64.0 + 3.0 + 15.0 + 2.0;
        self.shell.theme_metrics.chrome_row_height
            + self.ingest.source_dock.header_timeline_gap
            + timeline_height
    }

    /// The blocks of Ingest in the places of the standard layout.
    pub fn board_names(&self) -> qnc_board::BoardNames<'static> {
        qnc_board::BoardNames { monitor: "preview", head: "pool-head", body: "source-browser", right: "clip-cards", dock: "source-dock" }
    }

    /// The sizes of Ingest on the one desktop board: the monitor keeps its picture
    /// shape, the pool head is one chrome row, the dock holds the source timeline.
    pub fn board_sizes(&self) -> qnc_board::BoardSizes {
        let board = &self.ingest.board;
        let preview = &self.ingest.preview;
        qnc_board::BoardSizes {
            left_ratio: board.left_ratio,
            divider_width: board.divider_width,
            left_min_width: board.left_min_width,
            right_min_width: board.right_min_width,
            monitor_height: preview.min_height,
            head_height: self.shell.theme_metrics.chrome_row_height,
            dock_height: self.dock_height(),
            monitor_aspect: Some(qnc_board::MonitorAspect {
                ratio: preview.aspect_ratio(),
                reserve_below: preview.reserve_below,
                min_height: preview.min_height,
                width_inset: preview.width_inset,
                min_width: preview.min_width,
            }),
        }
    }
}

pub fn check_contracts_message() -> Result<String, String> {
    let contracts = IngestContracts::load()?;
    Ok(format!(
        "qnc-ingest contracts ok: {} / {} / shortcuts={}",
        contracts.ingest.layout_id, contracts.ingest.application_id, contracts.shortcuts_loaded
    ))
}

fn report_to_result(report: qnc_contracts::ValidationReport) -> Result<(), String> {
    if report.is_ok() {
        Ok(())
    } else {
        Err(report
            .errors
            .iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; "))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestLayoutContract {
    pub layout_id: String,
    pub application_id: String,
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
    pub transport_right: Vec<TransportCommand>,
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
}

#[derive(Debug, Clone, Deserialize)]
pub struct IngestClipGrid {
    pub min_card_width: f32,
    pub card_text_height: f32,
    pub grid_gap: f32,
    pub empty_message: String,
    pub empty_new_message: String,
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
}

#[derive(Debug, Deserialize)]
struct ApplicationManifest {
    application_id: String,
}

// Shape copied from qnc-ingest-desktop/src/layout_contract.rs; reads
// `contracts/ui/editorial.layout.json` (same geometry as `ingest.layout.json`)
// and picks the composition of one group (e, g, l, o).
use std::collections::HashMap;

use serde::Deserialize;

const SHELL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/shell.layout.json");
const EDITORIAL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/editorial.layout.json");

#[derive(Debug, Clone)]
pub struct EditorialContracts {
    pub shell: ShellLayoutContract,
    pub editorial: EditorialLayoutContract,
    pub group: String,
}

impl EditorialContracts {
    pub fn load(group: &str) -> Result<Self, String> {
        report_to_result(qnc_contracts::validate_ui_layout_contract_json(
            "contracts/ui/shell.layout.json",
            SHELL_LAYOUT_JSON,
        ))?;
        report_to_result(qnc_contracts::validate_ui_layout_contract_json(
            "contracts/ui/editorial.layout.json",
            EDITORIAL_LAYOUT_JSON,
        ))?;

        let shell: ShellLayoutContract = serde_json::from_str(SHELL_LAYOUT_JSON)
            .map_err(|error| format!("shell layout parse error: {error}"))?;
        let editorial: EditorialLayoutContract = serde_json::from_str(EDITORIAL_LAYOUT_JSON)
            .map_err(|error| format!("editorial layout parse error: {error}"))?;

        if shell.layout_id != "qnc.ui.shell" {
            return Err(format!("unexpected shell layout_id {}", shell.layout_id));
        }
        if editorial.layout_id != "qnc.ui.editorial" {
            return Err(format!(
                "unexpected editorial layout_id {}",
                editorial.layout_id
            ));
        }
        if editorial.board.left_ratio <= 0.0 || editorial.board.left_ratio >= 1.0 {
            return Err("editorial board left_ratio must split the desktop".to_string());
        }
        if editorial.board.shell_margin_x < 0.0 {
            return Err("editorial shell_margin_x must not be negative".to_string());
        }
        let Some(composition) = editorial.groups.get(group) else {
            return Err(format!("editorial layout has no group '{group}'"));
        };
        if composition.source_dock.actions_rtl.is_empty() {
            return Err(format!("group '{group}' source dock must declare actions"));
        }

        Ok(Self {
            shell,
            editorial,
            group: group.to_string(),
        })
    }

    pub fn composition(&self) -> &GroupComposition {
        &self.editorial.groups[&self.group]
    }

    pub fn dock_height(&self) -> f32 {
        let timeline_height = 15.0 + 3.0 + 64.0 + 3.0 + 15.0 + 2.0;
        self.shell.theme_metrics.chrome_row_height
            + self.editorial.source_dock.header_timeline_gap
            + timeline_height
    }
}

pub fn check_contracts_message(group: &str) -> Result<String, String> {
    let contracts = EditorialContracts::load(group)?;
    Ok(format!(
        "qnc-editorial contracts ok: {} / group {} / right panel {}",
        contracts.editorial.layout_id,
        contracts.group,
        contracts.composition().right_panel
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
pub struct ShellLayoutContract {
    pub layout_id: String,
    pub colors: ShellColors,
    pub theme_metrics: ShellThemeMetrics,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellThemeMetrics {
    pub font_ui: f32,
    pub font_timecode: f32,
    pub chrome_row_height: f32,
    pub chrome_control_height: f32,
    pub chrome_pad_x: i8,
    pub chrome_pad_y: i8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellColors {
    pub bg: [u8; 3],
    pub surface: [u8; 3],
    pub raised: [u8; 3],
    pub border: [u8; 3],
    pub text: [u8; 3],
    pub muted: [u8; 3],
    pub accent: [u8; 3],
    pub focus: [u8; 3],
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditorialLayoutContract {
    pub layout_id: String,
    #[serde(rename = "shell")]
    pub board: EditorialBoard,
    pub preview: EditorialPreview,
    pub pool_head: EditorialPoolHead,
    pub clip_list: EditorialClipList,
    pub source_dock: EditorialSourceDock,
    pub groups: HashMap<String, GroupComposition>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditorialBoard {
    pub left_ratio: f32,
    pub divider_width: f32,
    pub left_min_width: f32,
    pub right_min_width: f32,
    pub shell_margin_x: f32,
    pub block_pad: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditorialPreview {
    pub min_height: f32,
    pub reserve_below: f32,
    pub aspect: String,
    pub empty_label: String,
}

impl EditorialPreview {
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
pub struct EditorialPoolHead {
    pub tabs_left: Vec<String>,
    pub transport_right: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditorialSourceDock {
    pub clip_label_fallback: String,
    pub header_timeline_gap: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupComposition {
    pub role: String,
    pub right_panel: String,
    pub source_dock: GroupSourceDock,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupSourceDock {
    pub actions_rtl: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditorialClipList {
    pub row_height: f32,
    pub row_pad_x: f32,
}

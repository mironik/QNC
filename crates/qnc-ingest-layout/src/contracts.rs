use crate::*;
use qnc_keyboard_shortcut::ShortcutCatalog;
use serde::Deserialize;

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
        // The texts of the contract are codes, in the language of this computer (qnc-ui-text).
        let ingest: IngestLayoutContract = serde_json::from_value(qnc_ui_text::resolve_contract(INGEST_LAYOUT_JSON)?)
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

    /// The Tab walk of Ingest (user 2026-10-14: source, folder, Select or Cancel, pool,
    /// timeline, its buttons): its panels in order, its row panels and their actions, and
    /// the click actions of each panel.
    pub fn focus_names(&self) -> (Vec<&str>, Vec<(&str, Vec<&str>)>, Vec<(&str, &str)>) {
        let order = self.ingest.tab_order.iter().map(String::as_str).collect();
        let rows = self.ingest.focus_rows.iter().map(|(panel, actions)| (panel.as_str(), actions.iter().map(String::as_str).collect())).collect();
        let clicks = self.ingest.focus_clicks.iter().flat_map(|(panel, actions)| actions.iter().map(move |action| (panel.as_str(), action.as_str()))).collect();
        (order, rows, clicks)
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

    /// The board of Ingest: the standard layout with the row under the monitor in two
    /// places, the pool tabs (`pool-head`) and the transport panel (`transport`).
    pub fn board_layout(&self) -> qnc_board::Layout {
        self.board_sizes().standard_layout(&self.board_names()).split_place("pool-head", "transport", self.ingest.pool_head.transport_width)
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

#[derive(Debug, Deserialize)]
struct ApplicationManifest {
    application_id: String,
}


//! Public model of the editorial layout contract: reads
//! `contracts/ui/editorial.layout.json` (same geometry as `ingest.layout.json`), the
//! shell layout and the QNC keyboard catalog, validates them and picks the
//! composition of one group (e, g, l, o). No paint, no state, no application.
use std::collections::HashMap;

use qnc_keyboard_shortcut::ShortcutCatalog;
use serde::Deserialize;

pub use qnc_desktop_theme as theme;
pub use qnc_desktop_theme::{ShellColors, ShellLayoutContract, ShellThemeMetrics};

const SHELL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/shell.layout.json");
const EDITORIAL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/editorial.layout.json");
const KEYBOARD_SHORTCUTS_JSON: &str =
    include_str!("../../../contracts/qnc-keyboard-shortcuts.json");

#[derive(Debug, Clone)]
pub struct EditorialContracts {
    pub shell: ShellLayoutContract,
    pub editorial: EditorialLayoutContract,
    pub shortcuts: ShortcutCatalog,
    pub shortcuts_loaded: bool,
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
        // The texts of the contract are codes, in the language of this computer (qnc-ui-text).
        let editorial: EditorialLayoutContract = serde_json::from_value(qnc_ui_text::resolve_contract(EDITORIAL_LAYOUT_JSON)?)
            .map_err(|error| format!("editorial layout parse error: {error}"))?;
        let shortcuts = ShortcutCatalog::from_json_str(KEYBOARD_SHORTCUTS_JSON)
            .map_err(|error| format!("keyboard shortcut catalog parse error: {error}"))?;
        let shortcuts_loaded = !shortcuts.actions.is_empty();

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
        if let Some(name) = composition.tab_order.iter().find(|name| !FOCUS_PANELS.contains(&name.as_str())) {
            return Err(format!("group '{group}' tab_order has unknown panel '{name}'"));
        }

        Ok(Self {
            shell,
            editorial,
            shortcuts,
            shortcuts_loaded,
            group: group.to_string(),
        })
    }

    pub fn composition(&self) -> &GroupComposition {
        &self.editorial.groups[&self.group]
    }

    /// Pool head tabs of this group (All, Virtual, B-roll, Segment ...).
    pub fn pool_tabs(&self) -> &[TransportCommand] {
        let own = self.composition().pool_head.tabs_left.as_deref();
        own.unwrap_or(&self.editorial.pool_head.tabs_left)
    }

    /// Pool head transport buttons of this group, left to right.
    pub fn pool_transport(&self) -> &[TransportCommand] {
        let own = self.composition().pool_head.transport_right.as_deref();
        own.unwrap_or(&self.editorial.pool_head.transport_right)
    }

    /// The Tab walk of this group (user 2026-10-13: it belongs to the form): its panels in
    /// order, the action ids of its pool tabs, the action ids of its source dock buttons
    /// left to right as they are drawn.
    pub fn focus_names(&self) -> (Vec<&str>, Vec<&str>, Vec<&str>) {
        let order = self.composition().tab_order.iter().map(String::as_str).collect();
        let tabs = self.pool_tabs().iter().filter_map(TransportCommand::action_id).collect();
        let buttons = self.composition().source_dock.actions_rtl.iter().rev().filter_map(TransportCommand::action_id).collect();
        (order, tabs, buttons)
    }

    /// The look of the row of the pool tabs and of the transport panel.
    pub fn row_style(&self, theme: &theme::Theme) -> qnc_media_pool_head::RowStyle {
        qnc_media_pool_head::RowStyle {
            text: theme.text,
            muted: theme.text_muted,
            accent: theme.accent,
            border: theme.border,
            font_ui: theme.font_ui,
            control_height: theme.chrome_control_height,
            tab_gap: 10.0,
        }
    }

    /// The look of the source dock of this layout.
    pub fn dock_style(&self, theme: &theme::Theme) -> qnc_source_dock::TimelineDockStyle {
        qnc_source_dock::TimelineDockStyle {
            fill: theme.panel_alt,
            border: theme.border,
            text: theme.text,
            font_ui: theme.font_ui,
            chrome_row_height: theme.chrome_row_height,
            chrome_control_height: theme.chrome_control_height,
            chrome_pad_x: theme.chrome_pad_x,
            chrome_pad_y: theme.chrome_pad_y,
            header_timeline_gap: self.editorial.source_dock.header_timeline_gap,
        }
    }

    pub fn dock_height(&self) -> f32 {
        let timeline_height = 15.0 + 3.0 + 64.0 + 3.0 + 15.0 + 2.0;
        self.shell.theme_metrics.chrome_row_height
            + self.editorial.source_dock.header_timeline_gap
            + timeline_height
    }

    /// The blocks of Media Assist and Story in the places of the standard layout.
    pub fn board_names(&self) -> qnc_board::BoardNames<'static> {
        qnc_board::BoardNames {
            monitor: "preview",
            head: "pool-head",
            body: "clip-cards",
            right: "right-panel",
            dock: "source-dock",
        }
    }

    /// The board of this group: the standard layout with the row under the monitor in two
    /// places, the pool tabs (`pool-head`) and the transport panel (`transport`).
    pub fn board_layout(&self) -> qnc_board::Layout {
        let width = self.composition().pool_head.transport_width.unwrap_or(self.editorial.pool_head.transport_width);
        self.board_sizes().standard_layout(&self.board_names()).split_place("pool-head", "transport", width)
    }

    /// The sizes of Media Assist and Story on the one desktop board: the monitor keeps
    /// its picture shape, the pool head is one chrome row, the dock holds the source
    /// timeline.
    pub fn board_sizes(&self) -> qnc_board::BoardSizes {
        let board = &self.editorial.board;
        let preview = &self.editorial.preview;
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

pub fn check_contracts_message(group: &str) -> Result<String, String> {
    let contracts = EditorialContracts::load(group)?;
    Ok(format!(
        "qnc-editorial contracts ok: {} / group {} / right panel {} / shortcuts={}",
        contracts.editorial.layout_id,
        contracts.group,
        contracts.composition().right_panel,
        contracts.shortcuts_loaded
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
pub struct EditorialLayoutContract {
    pub layout_id: String,
    #[serde(rename = "shell")]
    pub board: EditorialBoard,
    pub preview: EditorialPreview,
    pub pool_head: EditorialPoolHead,
    pub media_card: EditorialMediaCard,
    pub source_dock: EditorialSourceDock,
    pub groups: HashMap<String, GroupComposition>,
    /// The colour of a saved cover by its length, first band that holds it (user
    /// 2026-10-12): under 2 s orange, under 3 s yellow, under 4 s green, up to 5 s cyan,
    /// longer blue.
    #[serde(default)]
    pub cover_lengths: Vec<qnc_segment_panel::CoverLengthBand>,
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
    pub width_inset: f32,
    pub min_width: f32,
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
    pub tabs_left: Vec<TransportCommand>,
    pub transport_right: Vec<TransportCommand>,
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
pub struct EditorialSourceDock {
    pub clip_label_fallback: String,
    pub header_timeline_gap: f32,
}

/// The panels a group's `tab_order` may name.
pub const FOCUS_PANELS: [&str; 5] = ["pool_tabs", "pool", "source_timeline", "source_buttons", "segments"];

/// The timeline colours of the editorial theme.
pub fn timeline_theme(theme: &theme::Theme) -> qnc_timeline::TimelineTheme {
    qnc_timeline::TimelineTheme::from_qnc_theme(theme.bg, theme.surface, theme.surface_alt, theme.border_soft, theme.text, theme.text_muted, theme.accent)
}

/// The chrome buttons of the editorial theme.
pub fn button_style(theme: &theme::Theme) -> qnc_ui_kit::ChromeButtonStyle {
    qnc_ui_kit::ChromeButtonStyle { text: theme.text, border: theme.border, focus: theme.focus, height: theme.chrome_control_height }
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupComposition {
    pub role: String,
    pub right_panel: String,
    /// The panels Tab walks in this group, in order (`FOCUS_PANELS`).
    pub tab_order: Vec<String>,
    /// Tabs and transport of this group; the shared pool head when absent.
    #[serde(default)]
    pub pool_head: GroupPoolHead,
    pub source_dock: GroupSourceDock,
    pub media_card: GroupMediaCard,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct GroupPoolHead {
    pub tabs_left: Option<Vec<TransportCommand>>,
    pub transport_right: Option<Vec<TransportCommand>>,
    pub transport_width: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupSourceDock {
    pub actions_rtl: Vec<TransportCommand>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EditorialMediaCard {
    pub min_card_width: f32,
    pub card_text_height: f32,
    pub grid_gap: f32,
    pub empty_message: String,
    pub broll_empty_message: String,
    pub virtual_empty_message: String,
    pub loading_message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroupMediaCard {
    pub selection_check: bool,
    pub status_dots: String,
}

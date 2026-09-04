use qnc_contracts::validate_ui_layout_contract_json;
use qnc_keyboard_shortcut::ShortcutCatalog;
use serde::Deserialize;

const SHELL_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/shell.layout.json");
const PROJECT_LAYOUT_JSON: &str = include_str!("../../../contracts/ui/project.layout.json");
const KEYBOARD_SHORTCUTS_JSON: &str =
    include_str!("../../../contracts/qnc-keyboard-shortcuts.json");

#[derive(Debug, Clone)]
pub struct AppContracts {
    pub shell: ShellLayoutContract,
    pub project: ProjectLayoutContract,
    pub shortcuts: ShortcutCatalog,
    pub project_open_hint: Option<String>,
}

impl AppContracts {
    pub fn load_embedded() -> Result<Self, String> {
        validate_layout_json("contracts/ui/shell.layout.json", SHELL_LAYOUT_JSON)?;
        validate_layout_json("contracts/ui/project.layout.json", PROJECT_LAYOUT_JSON)?;

        let shell = serde_json::from_str::<ShellLayoutContract>(SHELL_LAYOUT_JSON)
            .map_err(|error| format!("shell layout parse failed: {error}"))?;
        let project = serde_json::from_str::<ProjectLayoutContract>(PROJECT_LAYOUT_JSON)
            .map_err(|error| format!("project layout parse failed: {error}"))?;
        let shortcuts = ShortcutCatalog::from_json_str(KEYBOARD_SHORTCUTS_JSON)?;
        let project_open_hint = shortcuts.chord_hint("project", "project_open_selected");
        if project_open_hint.is_none() {
            return Err("keyboard catalog missing project_open_selected binding".to_string());
        }
        validate_metrics(&shell, &project)?;

        Ok(Self {
            shell,
            project,
            shortcuts,
            project_open_hint,
        })
    }
}

fn validate_metrics(
    shell: &ShellLayoutContract,
    project: &ProjectLayoutContract,
) -> Result<(), String> {
    if shell.shell_metrics.footer_height <= 0.0
        || shell.shell_metrics.workspace_status_height <= 0.0
        || shell.shell_metrics.workspace_footer_columns == 0
    {
        return Err("shell metrics must be positive".to_string());
    }
    if shell.theme_metrics.font_ui <= 0.0
        || shell.theme_metrics.font_timecode <= 0.0
        || shell.theme_metrics.chrome_row_height <= 0.0
        || shell.theme_metrics.chrome_control_height <= 0.0
    {
        return Err("theme metrics must be positive".to_string());
    }
    if project.left_project_list.ready_text.trim().is_empty() {
        return Err("project list ready_text is required".to_string());
    }
    if project.right_settings_panel.field_min_width <= 0.0 {
        return Err("project settings field_min_width must be positive".to_string());
    }
    Ok(())
}

fn validate_layout_json(name: &str, contents: &str) -> Result<(), String> {
    let report = validate_ui_layout_contract_json(name, contents);
    if report.is_ok() {
        Ok(())
    } else {
        Err(report.errors.join("; "))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellLayoutContract {
    pub layout_id: String,
    pub shell_metrics: ShellMetrics,
    pub theme_metrics: ThemeMetrics,
    pub colors: ThemeColors,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellMetrics {
    pub footer_height: f32,
    pub workspace_status_height: f32,
    pub workspace_footer_columns: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeMetrics {
    pub font_ui: f32,
    pub font_timecode: f32,
    pub chrome_row_height: f32,
    pub chrome_pad_x: i8,
    pub chrome_pad_y: i8,
    pub chrome_control_height: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThemeColors {
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
pub struct ProjectLayoutContract {
    pub layout_id: String,
    pub board: BoardMetrics,
    pub left_project_list: ProjectListMetrics,
    pub right_settings_panel: SettingsPanelMetrics,
    pub pts_slots: PtsSlots,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BoardMetrics {
    pub left_ratio: f32,
    pub divider_width: f32,
    pub left_min_width: f32,
    pub right_min_width: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectListMetrics {
    pub panel_pad: f32,
    pub title: String,
    pub title_row_height: f32,
    pub below_title_pad: f32,
    pub row_height: f32,
    pub row_gap: f32,
    pub delete_column_width: f32,
    pub column_gap: f32,
    pub empty_text: String,
    pub ready_text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SettingsPanelMetrics {
    pub panel_pad: f32,
    pub title: String,
    pub subtitle: String,
    pub inner_pad_x: f32,
    pub inner_pad_y: f32,
    pub section_gap: f32,
    pub inline_label_width: f32,
    pub inline_button_width: f32,
    pub inline_column_gap: f32,
    pub field_min_width: f32,
    pub label_font_size: f32,
    pub group_title_font_size: f32,
    pub row_height: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PtsSlots {
    pub fixed_order: Vec<String>,
    pub scroll_order: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_project_contracts() {
        let contracts = AppContracts::load_embedded().expect("contracts");
        assert_eq!(contracts.project.layout_id, "qnc.ui.project");
        assert_eq!(contracts.project.board.left_ratio, 0.31);
        assert_eq!(
            contracts.project.pts_slots.fixed_order,
            [
                "TemplatePicker",
                "ProjectCreate",
                "AiSettings",
                "ProjectsRoot",
                "ExportDirectory",
                "TemplateActions",
            ]
        );
    }

    #[test]
    fn shortcut_hint_comes_from_external_catalog() {
        let contracts = AppContracts::load_embedded().expect("contracts");
        assert_eq!(contracts.project_open_hint.as_deref(), Some("Enter"));
    }
}

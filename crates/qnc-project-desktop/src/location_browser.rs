use eframe::egui::{self, RichText, Sense, Vec2};
use qnc_dir_browser::BrowserState;

use crate::{
    layout_contract::ShellLayoutContract,
    theme::{self, Theme},
};

const UP_COL_W: f32 = 42.0;
const DISKS_COL_W: f32 = 58.0;
const NAV_GAP_W: f32 = 12.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LocationSourceKind {
    #[default]
    Local,
    Lan,
    Internet,
}

impl LocationSourceKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Local => "Računalo",
            Self::Lan => "LAN",
            Self::Internet => "Internet",
        }
    }
}

pub struct LocationBrowserInput<'a> {
    pub id_salt: &'a str,
    pub kind: LocationSourceKind,
    pub browser: &'a BrowserState,
    pub error: Option<&'a str>,
    pub max_tree_height: Option<f32>,
    pub shell: &'a ShellLayoutContract,
}

pub enum LocationBrowserAction {
    None,
    SelectKind(LocationSourceKind),
    OpenUri(String),
    OpenParent,
    OpenRoots,
}

pub fn show(ui: &mut egui::Ui, input: LocationBrowserInput<'_>) -> LocationBrowserAction {
    let mut action = LocationBrowserAction::None;
    let t = Theme::from_contract(&input.shell.colors);
    let font_ui = input.shell.theme_metrics.font_ui;

    ui.horizontal(|ui| {
        theme::label(ui, "Izvori", font_ui, t.muted);
        ui.add_space(12.0);
        for kind in [
            LocationSourceKind::Local,
            LocationSourceKind::Lan,
            LocationSourceKind::Internet,
        ] {
            let selected = input.kind == kind;
            if link_tab(ui, kind.label(), selected, input.shell).clicked() && !selected {
                action = LocationBrowserAction::SelectKind(kind);
            }
            ui.add_space(10.0);
        }
    });

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let can_up = matches!(input.kind, LocationSourceKind::Local)
            && !input.browser.roots
            && input.browser.parent_available;
        if fixed_text_link(ui, "Gore", can_up, UP_COL_W, input.shell).clicked() {
            action = LocationBrowserAction::OpenParent;
        }
        ui.add_space(NAV_GAP_W);
        let can_disks = matches!(input.kind, LocationSourceKind::Local);
        if fixed_text_link(ui, "Diskovi", can_disks, DISKS_COL_W, input.shell).clicked() {
            action = LocationBrowserAction::OpenRoots;
        }
        ui.add_space(NAV_GAP_W);
        show_location_breadcrumb(ui, &input, &mut action);
    });

    if let Some(error) = input.error {
        ui.add_space(4.0);
        ui.colored_label(egui::Color32::from_rgb(220, 100, 80), error);
    }

    ui.add_space(6.0);
    let available_tree_h = ui.available_height().max(40.0);
    let tree_h = input
        .max_tree_height
        .map(|max_h| available_tree_h.min(max_h).max(40.0))
        .unwrap_or(available_tree_h);
    egui::ScrollArea::vertical()
        .id_salt(format!("{}_location_browser", input.id_salt))
        .max_height(tree_h)
        .min_scrolled_height(tree_h)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            match input.kind {
                LocationSourceKind::Local => {
                    show_local_tree(ui, &input, &mut action);
                }
                LocationSourceKind::Lan => {
                    theme::label(ui, "Nema konfiguriranih LAN izvora.", font_ui, t.muted);
                }
                LocationSourceKind::Internet => {
                    theme::label(ui, "Nema konfiguriranih Internet izvora.", font_ui, t.muted);
                }
            }
        });

    action
}

fn fixed_text_link(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    width: f32,
    shell: &ShellLayoutContract,
) -> egui::Response {
    ui.allocate_ui_with_layout(
        Vec2::new(width, shell.theme_metrics.chrome_control_height),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| text_link(ui, label, enabled, shell),
    )
    .inner
}

fn link_tab(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    shell: &ShellLayoutContract,
) -> egui::Response {
    let t = Theme::from_contract(&shell.colors);
    let color = if selected { t.text } else { t.muted };
    let response = ui.add(
        egui::Label::new(
            RichText::new(label)
                .size(shell.theme_metrics.font_ui)
                .color(color),
        )
        .sense(Sense::click()),
    );
    if selected {
        let y = response.rect.bottom() + 2.0;
        ui.painter()
            .hline(response.rect.x_range(), y, egui::Stroke::new(2.0, t.accent));
    }
    response
}

fn text_link(
    ui: &mut egui::Ui,
    label: &str,
    enabled: bool,
    shell: &ShellLayoutContract,
) -> egui::Response {
    let t = Theme::from_contract(&shell.colors);
    let color = if !enabled {
        t.muted.linear_multiply(0.55)
    } else {
        t.text
    };
    ui.add_enabled(
        enabled,
        egui::Label::new(
            RichText::new(label)
                .size(shell.theme_metrics.font_ui)
                .color(color),
        )
        .sense(Sense::click()),
    )
}

fn show_local_tree(
    ui: &mut egui::Ui,
    input: &LocationBrowserInput<'_>,
    action: &mut LocationBrowserAction,
) {
    let t = Theme::from_contract(&input.shell.colors);
    if input.browser.roots {
        return;
    }

    if input.browser.entries.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(path_tree_offset());
            theme::label(
                ui,
                "Nema podmapa.",
                input.shell.theme_metrics.font_ui,
                t.muted,
            );
        });
        return;
    }

    for entry in &input.browser.entries {
        if location_tree_row(ui, path_tree_offset(), &entry.name, input.shell) {
            *action = LocationBrowserAction::OpenUri(entry.qnc_uri.clone());
        }
    }
}

fn path_tree_offset() -> f32 {
    UP_COL_W + NAV_GAP_W + DISKS_COL_W + NAV_GAP_W
}

fn location_tree_row(
    ui: &mut egui::Ui,
    offset: f32,
    label: &str,
    shell: &ShellLayoutContract,
) -> bool {
    ui.horizontal(|ui| {
        ui.add_space(offset);
        text_link(ui, label, true, shell).clicked()
    })
    .inner
}

fn show_location_breadcrumb(
    ui: &mut egui::Ui,
    input: &LocationBrowserInput<'_>,
    action: &mut LocationBrowserAction,
) {
    let t = Theme::from_contract(&input.shell.colors);
    if matches!(input.kind, LocationSourceKind::Local) && input.browser.roots {
        show_root_disks_table(ui, input, action);
        return;
    }

    if !matches!(input.kind, LocationSourceKind::Local) {
        theme::label(
            ui,
            location_label(input),
            input.shell.theme_metrics.font_ui,
            t.text,
        );
        return;
    }

    if input.browser.breadcrumbs.is_empty() {
        theme::label(
            ui,
            &short_path(&input.browser.path_label),
            input.shell.theme_metrics.font_ui,
            t.text,
        );
        return;
    }

    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for (index, crumb) in input.browser.breadcrumbs.iter().enumerate() {
            if index > 0 {
                theme::label(ui, "\\", input.shell.theme_metrics.font_ui, t.muted);
            }
            if text_link(ui, &crumb.label, true, input.shell).clicked() {
                *action = LocationBrowserAction::OpenUri(crumb.qnc_uri.clone());
            }
        }
    });
}

fn show_root_disks_table(
    ui: &mut egui::Ui,
    input: &LocationBrowserInput<'_>,
    action: &mut LocationBrowserAction,
) {
    let t = Theme::from_contract(&input.shell.colors);
    if input.browser.entries.is_empty() {
        theme::label(
            ui,
            "Nema diskova.",
            input.shell.theme_metrics.font_ui,
            t.muted,
        );
        return;
    }

    egui::Grid::new(format!("{}_root_disk_table", input.id_salt))
        .num_columns(3)
        .spacing(Vec2::new(14.0, 4.0))
        .striped(false)
        .show(ui, |ui| {
            for entry in &input.browser.entries {
                let mut clicked = false;
                clicked |= root_disk_cell(ui, &entry.name, input.shell).clicked();
                clicked |= root_disk_cell(ui, &entry.serial_number, input.shell).clicked();
                clicked |= root_disk_cell(ui, &entry.volume_name, input.shell).clicked();
                ui.end_row();

                if clicked {
                    *action = LocationBrowserAction::OpenUri(entry.qnc_uri.clone());
                }
            }
        });
}

fn root_disk_cell(ui: &mut egui::Ui, text: &str, shell: &ShellLayoutContract) -> egui::Response {
    ui.add(
        egui::Label::new(
            RichText::new(text)
                .size(shell.theme_metrics.font_ui)
                .color(Theme::from_contract(&shell.colors).text),
        )
        .sense(Sense::click())
        .selectable(false),
    )
}

fn location_label(input: &LocationBrowserInput<'_>) -> String {
    match input.kind {
        LocationSourceKind::Local if input.browser.roots => "Diskovi".to_string(),
        LocationSourceKind::Local if !input.browser.path_label.trim().is_empty() => {
            short_path(&input.browser.path_label)
        }
        LocationSourceKind::Lan => "LAN".to_string(),
        LocationSourceKind::Internet => "Internet".to_string(),
        _ => "-".to_string(),
    }
}

fn short_path(path: &str) -> String {
    if path.chars().count() <= 42 {
        return path.to_string();
    }
    let tail: String = path
        .chars()
        .rev()
        .take(36)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("...{tail}")
}

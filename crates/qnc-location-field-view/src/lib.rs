//! A location field's browser as it is drawn (docs/99 step 3): the open browser with Select and
//! Cancel under it. The field's state and steps are the core piece `qnc-location-field`.

use eframe::egui;
use qnc_dir_browser_panel::{short_path, Body, BrowserAnswer, BrowserInput, BrowserLabels, BrowserStyle, Crumb, Entry, Nav};
use qnc_location_field::{LocationField, SourceKind, KINDS};

pub const MODULE_ID: &str = "qnc.module.location-field-view";

/// The words of the browser.
pub struct FieldLabels<'a> {
    pub sources: &'a str,
    pub up: &'a str,
    pub disks: &'a str,
    pub internet: &'a str,
    pub empty_folder: &'a str,
    pub empty_disks: &'a str,
    pub empty_lan: &'a str,
    pub empty_internet: &'a str,
    pub confirm: &'a str,
    pub cancel: &'a str,
}

/// The open browser of the field with Select/Cancel under it; what the user asked.
pub fn show_browser(
    ui: &mut egui::Ui,
    field: &LocationField,
    id_salt: &str,
    max_tree_height: f32,
    labels: &FieldLabels<'_>,
    theme: &qnc_desktop_theme::Theme,
) -> Option<BrowserAnswer> {
    let browser = &field.browser;
    let local = field.kind == SourceKind::Local;
    let path = short_path(&browser.path_label);
    let (nav, body) = match field.kind {
        SourceKind::Local if browser.roots => (Nav::Roots { empty: labels.empty_disks }, Body::Nothing),
        SourceKind::Local if browser.breadcrumbs.is_empty() => (Nav::Path(&path), Body::Folders),
        SourceKind::Local => (
            Nav::Crumbs(browser.breadcrumbs.iter().map(|crumb| Crumb { label: &crumb.label, uri: &crumb.qnc_uri }).collect()),
            Body::Folders,
        ),
        SourceKind::Lan => (Nav::Path(SourceKind::Lan.label()), Body::Message(labels.empty_lan)),
        SourceKind::Internet => (Nav::Path(labels.internet), Body::Message(labels.empty_internet)),
    };
    let kinds: Vec<&str> = KINDS.iter().map(|kind| kind.label()).collect();
    let input = BrowserInput {
        id_salt,
        kinds: &kinds,
        kind: KINDS.iter().position(|kind| *kind == field.kind).unwrap_or(0),
        nav,
        body,
        entries: browser
            .entries
            .iter()
            .map(|entry| Entry { name: &entry.name, serial: &entry.serial_number, volume: &entry.volume_name, uri: &entry.qnc_uri })
            .collect(),
        can_up: local && !browser.roots && browser.parent_available,
        can_disks: local,
        busy: false,
        error: field.error.as_deref(),
        max_tree_height: Some(max_tree_height),
        focused_kind: None,
        focused_entry: None,
    };
    let words = BrowserLabels { sources: labels.sources, up: labels.up, disks: labels.disks, empty_folder: labels.empty_folder, loading: "" };
    let style = BrowserStyle { disabled: theme.text_muted.linear_multiply(0.55), ..BrowserStyle::from_theme(theme) };
    ui.add_space(6.0);
    if let Some(action) = qnc_dir_browser_panel::show(ui, input, &words, &style) {
        return Some(BrowserAnswer::Browser(action));
    }
    ui.add_space(8.0);
    let bar = qnc_ui_kit::FormActionBarStyle::new(theme.text, theme.accent, theme.border, theme.font_ui, theme.chrome_control_height);
    let pressed = qnc_ui_kit::show_form_action_bar(ui, &bar, labels.confirm, field.can_confirm(), labels.cancel);
    if pressed.cancel_clicked {
        Some(BrowserAnswer::Cancel)
    } else if pressed.confirm_clicked {
        Some(BrowserAnswer::Confirm)
    } else {
        None
    }
}

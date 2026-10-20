//! The location browser of the Project form: the public panel `qnc-dir-browser-panel`
//! (the same one Ingest places), given this form's browser state and words.

use eframe::egui;
use qnc_dir_browser::BrowserState;
use qnc_dir_browser_panel::{short_path, Body, BrowserAction, BrowserInput, BrowserLabels, BrowserStyle, Crumb, Entry, Nav};

use crate::{layout_contract::ShellLayoutContract, theme::Theme};

pub use qnc_project_session::LocationSourceKind;

/// The source tabs in order.
const KINDS: [LocationSourceKind; 3] = [LocationSourceKind::Local, LocationSourceKind::Lan, LocationSourceKind::Internet];

pub struct LocationBrowserInput<'a> {
    pub id_salt: &'a str,
    pub kind: LocationSourceKind,
    pub browser: &'a BrowserState,
    pub error: Option<&'a str>,
    pub max_tree_height: Option<f32>,
    pub shell: &'a ShellLayoutContract,
    pub texts: &'a crate::layout_contract::ProjectTexts,
}

pub enum LocationBrowserAction {
    None,
    SelectKind(LocationSourceKind),
    OpenUri(String),
    OpenParent,
    OpenRoots,
}

pub fn show(ui: &mut egui::Ui, input: LocationBrowserInput<'_>) -> LocationBrowserAction {
    let browser = input.browser;
    let texts = input.texts;
    let local = input.kind == LocationSourceKind::Local;
    let path = short_path(&browser.path_label);
    let (nav, body) = match input.kind {
        LocationSourceKind::Local if browser.roots => (Nav::Roots { empty: &texts.empty_disks }, Body::Nothing),
        LocationSourceKind::Local if browser.breadcrumbs.is_empty() => (Nav::Path(&path), Body::Folders),
        LocationSourceKind::Local => (
            Nav::Crumbs(browser.breadcrumbs.iter().map(|crumb| Crumb { label: &crumb.label, uri: &crumb.qnc_uri }).collect()),
            Body::Folders,
        ),
        LocationSourceKind::Lan => (Nav::Path(LocationSourceKind::Lan.label()), Body::Message(&texts.empty_lan)),
        LocationSourceKind::Internet => (Nav::Path(&texts.internet), Body::Message(&texts.empty_internet)),
    };
    let kinds: Vec<&str> = KINDS.iter().map(|kind| kind.label()).collect();
    let panel = BrowserInput {
        id_salt: input.id_salt,
        kinds: &kinds,
        kind: KINDS.iter().position(|kind| *kind == input.kind).unwrap_or(0),
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
        error: input.error,
        max_tree_height: input.max_tree_height,
        focused_kind: None,
        focused_entry: None,
    };
    let words = BrowserLabels {
        sources: &texts.sources,
        up: &texts.up,
        disks: &texts.disks,
        empty_folder: &texts.empty_folder,
        loading: "",
    };
    let t = Theme::from_contract(&input.shell.colors);
    let style = BrowserStyle {
        text: t.text,
        muted: t.muted,
        disabled: t.muted.linear_multiply(0.55),
        accent: t.accent,
        border: t.border,
        danger: qnc_desktop_theme::danger(),
        focus: t.focus,
        font_ui: input.shell.theme_metrics.font_ui,
        control_height: input.shell.theme_metrics.chrome_control_height,
    };
    match qnc_dir_browser_panel::show(ui, panel, &words, &style) {
        None => LocationBrowserAction::None,
        Some(BrowserAction::Kind(index)) => KINDS.get(index).map_or(LocationBrowserAction::None, |kind| LocationBrowserAction::SelectKind(*kind)),
        Some(BrowserAction::Open(uri)) => LocationBrowserAction::OpenUri(uri),
        Some(BrowserAction::Parent) => LocationBrowserAction::OpenParent,
        Some(BrowserAction::Roots) => LocationBrowserAction::OpenRoots,
    }
}

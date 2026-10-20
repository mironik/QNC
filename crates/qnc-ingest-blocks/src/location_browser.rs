use super::*;

use qnc_dir_browser_panel::{Body, BrowserAction, BrowserInput, BrowserLabels, BrowserStyle, Entry, Nav};

/// The source tabs in the order of the contract's labels.
const KINDS: [SourceKind; 3] = [SourceKind::Local, SourceKind::Lan, SourceKind::Internet];

/// The location browser: the public panel `qnc-dir-browser-panel` on this form's place.
pub(super) fn render_location_browser(
    ui: &mut Ui,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let outer = qnc_ui_kit::fill_block(ui, theme.bg);
    ui.allocate_rect(outer, Sense::hover());
    let labels = &contracts.ingest.dir_browser;
    let kinds: Vec<&str> = KINDS.iter().map(|kind| source_kind_label(labels, *kind)).collect();
    let kind = KINDS.iter().position(|kind| *kind == view.source_kind).unwrap_or(0);
    let empty = match view.source_kind {
        SourceKind::Local => &labels.empty_local,
        SourceKind::Lan => &labels.empty_lan,
        SourceKind::Internet => &labels.empty_internet,
    };
    let browser = &view.browser;
    let mut opened = None;
    let action = ui
        .scope_builder(egui::UiBuilder::new().max_rect(outer.shrink(contracts.ingest.board.block_pad)), |ui| {
            let (focused_entry, open) = qnc_focus_walk::list_keys(ui, "qnc-ingest-browser", browser.entries.len(), &view.focus, qnc_focus_walk::BROWSER);
            opened = open.and_then(|index| browser.entries.get(index)).map(|entry| entry.qnc_uri.clone());
            let input = BrowserInput {
                id_salt: "qnc-ingest-browser",
                kinds: &kinds,
                kind,
                nav: if browser.roots { Nav::Roots { empty } } else { Nav::Path(&browser.path_label) },
                body: if browser.roots { Body::Nothing } else { Body::Folders },
                entries: browser
                    .entries
                    .iter()
                    .map(|entry| Entry { name: &entry.name, serial: &entry.serial_number, volume: &entry.volume_name, uri: &entry.qnc_uri })
                    .collect(),
                can_up: browser.parent_available,
                can_disks: !view.command_busy,
                busy: browser.busy,
                error: browser.error.as_deref(),
                max_tree_height: None,
                focused_kind: view
                    .focus
                    .is(qnc_focus_walk::SOURCE_KINDS)
                    .then(|| KINDS.iter().position(|kind| view.focus.chosen(kind.action_id())))
                    .flatten(),
                focused_entry,
            };
            let words = BrowserLabels {
                sources: &labels.sources_label,
                up: &labels.up_label,
                disks: &labels.disks_label,
                empty_folder: &labels.empty_folder,
                loading: &labels.loading_label,
            };
            qnc_dir_browser_panel::show(ui, input, &words, &browser_style(theme))
        })
        .inner;
    match action {
        Some(BrowserAction::Kind(index)) => KINDS.get(index).map(|kind| IngestIntent::new(kind.action_id(), IngestPayload::SourceKind(*kind))),
        Some(BrowserAction::Open(uri)) => Some(IngestIntent::new(action_ids::INGEST_DIR_OPEN, IngestPayload::LocationUri(uri))),
        Some(BrowserAction::Parent) => Some(IngestIntent::empty(action_ids::INGEST_DIR_UP)),
        Some(BrowserAction::Roots) => Some(IngestIntent::empty(action_ids::INGEST_DIR_ROOTS)),
        // Enter on a folder opens it as a click.
        None => opened.map(|uri| IngestIntent::new(action_ids::INGEST_DIR_OPEN, IngestPayload::LocationUri(uri))),
    }
}

fn browser_style(theme: &Theme) -> BrowserStyle {
    BrowserStyle {
        text: theme.text,
        muted: theme.text_muted,
        disabled: theme.text_muted,
        accent: theme.accent,
        border: theme.border,
        danger: theme.danger,
        focus: theme.focus,
        font_ui: theme.font_ui,
        control_height: theme.chrome_control_height,
    }
}

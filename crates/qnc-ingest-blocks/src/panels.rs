//! Each Ingest place with its public panel: the Ingest layout's measures and texts and the
//! Ingest view go in, the panel's answer comes back as an Ingest intent.

use super::*;

use qnc_dir_browser_panel::{Body, BrowserAction, BrowserAnswer, BrowserInput, BrowserLabels, ConfirmBar, Entry, Nav};
use qnc_ingest_import_bar::{ImportBarAction, ImportBarNow, ImportBarSpec};
use qnc_media_card::{CardGridAction, CardMetrics, StatusDotsMode, Ticks};
use qnc_source_timeline::HeaderAction;

/// The source tabs in the order of the contract's labels.
const KINDS: [SourceKind; 3] = [SourceKind::Local, SourceKind::Lan, SourceKind::Internet];

pub(super) fn preview(ui: &mut Ui, rect: Rect, contracts: &IngestContracts, theme: &Theme, view: &IngestViewModel) -> Option<IngestIntent> {
    let label = if view.preview_clip_id.is_some() { view.current_clip_label() } else { contracts.ingest.preview.empty_label.as_str() };
    let shown = view.shown_clip().map(|clip| clip as &dyn qnc_clip_row::PoolItem);
    qnc_monitor_panel::show(ui, rect, egui::Id::new(("qnc-monitor", "ingest-source")), theme, &view.monitor, shown, label);
    None // the monitor gives no intent
}

pub(super) fn pool_tabs(ui: &mut Ui, rect: Rect, contracts: &IngestContracts, theme: &Theme, view: &IngestViewModel) -> Option<IngestIntent> {
    let head = &contracts.ingest.pool_head;
    let tabs: Vec<_> = head.tabs_left.iter().map(|tab| qnc_pool_tabs_panel::PoolTab { label: tab.label(), action_id: tab.action_id() }).collect();
    qnc_pool_tabs_panel::show(ui, rect, (theme, head.tab_gap), &tabs, |id| view.pool_tab.as_deref() == Some(id), |_| true).map(IngestIntent::empty)
}

pub(super) fn browser(ui: &mut Ui, rect: Rect, contracts: &IngestContracts, theme: &Theme, view: &IngestViewModel) -> Option<IngestIntent> {
    let labels = &contracts.ingest.dir_browser;
    let kinds: Vec<&str> = KINDS.iter().enumerate().map(|(index, kind)| labels.kinds.get(index).map_or(kind.label(), String::as_str)).collect();
    let empty = match view.source_kind {
        SourceKind::Local => &labels.empty_local,
        SourceKind::Lan => &labels.empty_lan,
        SourceKind::Internet => &labels.empty_internet,
    };
    let browser = &view.browser;
    let (focused_entry, opened) = qnc_focus_walk::list_keys(ui, "qnc-ingest-browser", browser.entries.len(), &view.focus, qnc_focus_walk::BROWSER);
    let input = BrowserInput {
        id_salt: "qnc-ingest-browser",
        kinds: &kinds,
        kind: KINDS.iter().position(|kind| *kind == view.source_kind).unwrap_or(0),
        nav: if browser.roots { Nav::Roots { empty } } else { Nav::Path(&browser.path_label) },
        body: if browser.roots { Body::Nothing } else { Body::Folders },
        entries: browser.entries.iter().map(|entry| Entry { name: &entry.name, serial: &entry.serial_number, volume: &entry.volume_name, uri: &entry.qnc_uri }).collect(),
        can_up: browser.parent_available,
        can_disks: !view.command_busy,
        busy: browser.busy,
        error: browser.error.as_deref(),
        max_tree_height: None,
        focused_kind: view.focus.is(qnc_focus_walk::SOURCE_KINDS).then(|| KINDS.iter().position(|kind| view.focus.chosen(kind.action_id()))).flatten(),
        focused_entry,
    };
    let words = BrowserLabels { sources: &labels.sources_label, up: &labels.up_label, disks: &labels.disks_label, empty_folder: &labels.empty_folder, loading: &labels.loading_label };
    let on_bar = |action_id| view.focus.is(qnc_focus_walk::BROWSER_BUTTONS) && view.focus.chosen(action_id);
    let bar = ConfirmBar {
        confirm: &labels.confirm_label,
        cancel: &labels.cancel_label,
        can_confirm: !browser.roots && !browser.path_label.is_empty() && !view.work_settings_loading && !browser.busy && !view.command_busy,
        focused: [true, false].into_iter().find(|confirm| on_bar(if *confirm { action_ids::INGEST_DIR_CONFIRM } else { action_ids::INGEST_DIR_CANCEL })),
    };
    let open = |uri| IngestIntent::new(action_ids::INGEST_DIR_OPEN, IngestPayload::LocationUri(uri));
    match qnc_dir_browser_panel::show_with_confirm(ui, rect, (theme, contracts.ingest.board.block_pad), input, &words, &bar) {
        Some(BrowserAnswer::Browser(BrowserAction::Kind(index))) => KINDS.get(index).map(|kind| IngestIntent::new(kind.action_id(), IngestPayload::SourceKind(*kind))),
        Some(BrowserAnswer::Browser(BrowserAction::Open(uri))) => Some(open(uri)),
        Some(BrowserAnswer::Browser(BrowserAction::Parent)) => Some(IngestIntent::empty(action_ids::INGEST_DIR_UP)),
        Some(BrowserAnswer::Browser(BrowserAction::Roots)) => Some(IngestIntent::empty(action_ids::INGEST_DIR_ROOTS)),
        Some(BrowserAnswer::Confirm) => browser.current_uri.clone().map(|uri| IngestIntent::new(action_ids::INGEST_DIR_CONFIRM, IngestPayload::LocationUri(uri))),
        Some(BrowserAnswer::Cancel) => Some(IngestIntent::empty(action_ids::INGEST_DIR_CANCEL)),
        // Enter on a folder opens it as a click.
        None => opened.and_then(|index| browser.entries.get(index)).map(|entry| open(entry.qnc_uri.clone())),
    }
}

pub(super) fn pool(ui: &mut Ui, contracts: &IngestContracts, theme: &Theme, view: &IngestViewModel) -> Option<IngestIntent> {
    let grid = &contracts.ingest.clip_grid;
    let spec = qnc_media_pool_panel::PoolSpec {
        pad: contracts.ingest.board.block_pad,
        metrics: CardMetrics { min_card_width: grid.min_card_width, card_text_height: grid.card_text_height, grid_gap: grid.grid_gap },
        ticks: Ticks::Selected,
        status_dots: StatusDotsMode::Off,
        imported_marker: Some(qnc_ingest_layout::rgb(grid.imported_marker_rgb)),
        saving: &grid.saving_label,
        save_failed: &grid.save_failed_label,
    };
    let empty_message = view.work_settings_error.as_deref().unwrap_or_else(|| match () {
        _ if view.work_settings_loading => &grid.loading_settings_message,
        _ if view.clip_filter == ClipFilter::New => &grid.empty_new_message,
        _ if view.command_busy || view.selected_source_uri.is_some() => &view.message,
        _ => &grid.empty_message,
    });
    let now = qnc_media_pool_panel::PoolNow {
        chosen_id: view.preview_clip_id.as_deref().unwrap_or(""),
        panel_focused: view.focus.is(qnc_focus_walk::CARDS),
        step: view.focus.step_of(qnc_focus_walk::CARDS),
        empty_message,
        id_salt: if view.clip_filter == ClipFilter::New { "ingest_grid_new" } else { "ingest_grid_all" },
    };
    let clips: Vec<&ClipView> = view.visible_clips().collect();
    match qnc_media_pool_panel::show(ui, theme, &spec, &now, &clips)? {
        CardGridAction::ToggleSelection(id) => Some(IngestIntent::new(action_ids::INGEST_CLIP_TOGGLE, IngestPayload::ClipId(id))),
        CardGridAction::Activate(id) => Some(IngestIntent::new(action_ids::INGEST_PREVIEW_FOCUS, IngestPayload::ClipId(id))),
    }
}

/// The one source timeline with the Ingest import bar in its header.
pub(super) fn source_dock(ui: &mut Ui, contracts: &IngestContracts, theme: &Theme, view: &IngestViewModel) -> Option<IngestIntent> {
    let dock = &contracts.ingest.source_dock;
    let rect = ui.available_rect_before_wrap();
    let label = if view.preview_clip_id.is_some() { view.current_clip_label() } else { &dock.clip_label_fallback };
    let spec = ImportBarSpec {
        show_import_actions: dock.show_import_actions,
        show_edit_actions: dock.show_edit_actions,
        filter_labels: [&dock.clip_filter_labels[0], &dock.clip_filter_labels[1]],
        filter_colors: dock.clip_filter_colors.map(qnc_ingest_layout::rgb),
        filter_width: dock.clip_filter_width,
        // Import (the strong one), Select all, Clear, drawn right to left.
        actions: dock.actions_rtl.iter().rev().filter(|action| action.action_id().is_some()).map(|action| HeaderAction { label: action.label(), action_id: action.action_id(), strong: action.action_id() == Some(action_ids::INGEST_IMPORT_SELECTED) }).collect(),
        edit_actions: &dock.edit_actions,
        archive_label: &dock.archive_original_label,
        ai_label: &dock.ai_mining_label,
    };
    let status = view.status_label();
    let now = ImportBarNow {
        status: &status,
        status_detail: &view.message,
        filter_all: view.clip_filter == ClipFilter::All,
        focused: view.focus.button.as_deref(),
        archive_available: view.archive_original_available,
        archive: view.archive_original,
        ai_mining: view.ai_mining,
    };
    let mut asked = None;
    let header = |ui: &mut Ui| asked = qnc_ingest_import_bar::show(ui, theme, &spec, &now, |id| view.action_enabled(id));
    let timeline = qnc_source_timeline::show(ui, rect, (theme, dock.header_timeline_gap), label, header, (&view.timeline, &view.timeline_assets), &view.source_timeline);
    match asked {
        Some(ImportBarAction::Filter(all)) => Some(IngestIntent::new(action_ids::INGEST_SET_CLIP_FILTER, IngestPayload::ClipFilter(if all { ClipFilter::All } else { ClipFilter::New }))),
        Some(ImportBarAction::Action(action_id)) => Some(IngestIntent::empty(&action_id)),
        Some(ImportBarAction::Archive(on)) => Some(IngestIntent::new(action_ids::INGEST_SET_ARCHIVE, IngestPayload::Bool(on))),
        Some(ImportBarAction::AiMining(on)) => Some(IngestIntent::new(action_ids::INGEST_SET_AI_MINING, IngestPayload::Bool(on))),
        None => timeline_intent_to_ingest_intent(timeline),
    }
}

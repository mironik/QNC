use super::*;

pub(super) fn render_browser_head_entries(
    ui: &mut Ui,
    labels: &IngestDirBrowser,
    theme: &Theme,
    view: &IngestViewModel,
    intent: &mut Option<IngestIntent>,
) {
    match view.source_kind {
        _ if view.browser.roots => {
            if view.browser.entries.is_empty() {
                let empty = match view.source_kind {
                    SourceKind::Local => &labels.empty_local,
                    SourceKind::Lan => &labels.empty_lan,
                    SourceKind::Internet => &labels.empty_internet,
                };
                ui.label(muted(empty, theme));
            } else {
                render_root_disk_grid(ui, view, theme, intent);
            }
        }
        _ => {
            ui.label(RichText::new(&view.browser.path_label).color(theme.text));
        }
    }
}

pub(super) fn render_browser_body(
    ui: &mut Ui,
    labels: &IngestDirBrowser,
    theme: &Theme,
    view: &IngestViewModel,
    intent: &mut Option<IngestIntent>,
) {
    if view.browser.busy {
        ui.label(muted(&labels.loading_label, theme));
        return;
    }
    if let Some(error) = &view.browser.error {
        ui.label(RichText::new(error).color(theme.danger));
        return;
    }
    match view.source_kind {
        _ if view.browser.roots => {}
        _ => {
            if view.browser.entries.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(path_tree_offset());
                    ui.label(muted(&labels.empty_folder, theme));
                });
            } else {
                let focused = keyboard_entry(ui, view, intent);
                ScrollArea::vertical().show(ui, |ui| {
                    for (index, entry) in view.browser.entries.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.add_space(path_tree_offset());
                            let response = browser_entry_button(ui, entry, theme);
                            qnc_focus_walk::paint_focus(ui, response.rect.expand(3.0), focused == Some(index), theme.focus);
                            if response.clicked() {
                                *intent = Some(IngestIntent::new(
                                    action_ids::INGEST_DIR_OPEN,
                                    IngestPayload::LocationUri(entry.qnc_uri.clone()),
                                ));
                            }
                        });
                    }
                });
            }
        }
    }
}

/// The entry the keyboard is on when the folders have the focus; Enter opens it as a click.
fn keyboard_entry(ui: &Ui, view: &IngestViewModel, intent: &mut Option<IngestIntent>) -> Option<usize> {
    let (focused, opened) = qnc_focus_walk::list_keys(ui, "qnc-ingest-browser", view.browser.entries.len(), &view.focus, qnc_focus_walk::BROWSER);
    if let Some(entry) = opened.and_then(|index| view.browser.entries.get(index)) {
        *intent = Some(IngestIntent::new(action_ids::INGEST_DIR_OPEN, IngestPayload::LocationUri(entry.qnc_uri.clone())));
    }
    focused
}

pub(super) fn browser_entry_button(
    ui: &mut Ui,
    entry: &LocationEntry,
    theme: &Theme,
) -> egui::Response {
    ui.add(Label::new(RichText::new(&entry.name).color(theme.text)).sense(Sense::click()))
}

pub(super) fn render_root_disk_grid(
    ui: &mut Ui,
    view: &IngestViewModel,
    theme: &Theme,
    intent: &mut Option<IngestIntent>,
) {
    egui::Grid::new("qnc_ingest_root_disk_table")
        .num_columns(3)
        .spacing(Vec2::new(14.0, 4.0))
        .striped(false)
        .show(ui, |ui| {
            let focused = keyboard_entry(ui, view, intent);
            for (index, entry) in view.browser.entries.iter().enumerate() {
                let mut clicked = false;
                let name = root_disk_cell(ui, &entry.name, theme);
                qnc_focus_walk::paint_focus(ui, name.rect.expand(3.0), focused == Some(index), theme.focus);
                clicked |= name.clicked();
                clicked |= root_disk_cell(ui, &entry.serial_number, theme).clicked();
                clicked |= root_disk_cell(ui, &entry.volume_name, theme).clicked();
                ui.end_row();

                if clicked {
                    *intent = Some(IngestIntent::new(
                        action_ids::INGEST_DIR_OPEN,
                        IngestPayload::LocationUri(entry.qnc_uri.clone()),
                    ));
                }
            }
        });
}

pub(super) fn root_disk_cell(ui: &mut Ui, text: &str, theme: &Theme) -> egui::Response {
    ui.add(
        Label::new(RichText::new(text).color(theme.text).size(theme.font_ui))
            .sense(Sense::click())
            .selectable(false),
    )
}

pub(super) fn path_tree_offset() -> f32 {
    42.0 + 12.0 + 58.0 + 12.0
}

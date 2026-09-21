use super::*;

pub(super) fn render_browser_head_entries(
    ui: &mut Ui,
    labels: &IngestDirBrowser,
    theme: &Theme,
    view: &IngestViewModel,
    intent: &mut Option<IngestIntent>,
) {
    match view.source_kind {
        _ if view.browser_roots => {
            if view.browser_entries.is_empty() {
                let empty = match view.source_kind {
                    SourceKind::Local => "Nema diskova.",
                    SourceKind::Lan => &labels.empty_lan,
                    SourceKind::Internet => &labels.empty_internet,
                };
                ui.label(muted(empty, theme));
            } else {
                render_root_disk_grid(ui, view, theme, intent);
            }
        }
        _ => {
            ui.label(RichText::new(&view.browser_path_label).color(theme.text));
        }
    }
}

pub(super) fn render_browser_body(
    ui: &mut Ui,
    _labels: &IngestDirBrowser,
    theme: &Theme,
    view: &IngestViewModel,
    intent: &mut Option<IngestIntent>,
) {
    if view.browser_busy {
        ui.label(muted("Učitavam...", theme));
        return;
    }
    if let Some(error) = &view.browser_error {
        ui.label(RichText::new(error).color(theme.danger));
        return;
    }
    match view.source_kind {
        _ if view.browser_roots => {}
        _ => {
            if view.browser_entries.is_empty() {
                ui.horizontal(|ui| {
                    ui.add_space(path_tree_offset());
                    ui.label(muted("Nema podmapa.", theme));
                });
            } else {
                ScrollArea::vertical().show(ui, |ui| {
                    for entry in &view.browser_entries {
                        ui.horizontal(|ui| {
                            ui.add_space(path_tree_offset());
                            if browser_entry_button(ui, entry, theme).clicked() {
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
            for entry in &view.browser_entries {
                let mut clicked = false;
                clicked |= root_disk_cell(ui, &entry.name, theme).clicked();
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

use super::*;

pub(super) fn show_chrome_row(
    ui: &mut Ui,
    rect: Rect,
    theme: &Theme,
    fill: Color32,
    draw_bottom_rule: bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    ui.painter().rect_filled(rect, 0.0, fill);
    if draw_bottom_rule {
        ui.painter().hline(
            rect.x_range(),
            rect.bottom() - 0.5,
            Stroke::new(1.0, theme.border),
        );
    }
    let inner = Rect::from_min_max(
        egui::pos2(
            rect.left() + theme.chrome_pad_x,
            rect.top() + theme.chrome_pad_y,
        ),
        egui::pos2(
            rect.right() - theme.chrome_pad_x,
            rect.bottom() - theme.chrome_pad_y,
        ),
    );
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.set_clip_rect(rect);
            ui.set_min_height(theme.chrome_control_height);
            ui.spacing_mut().button_padding = Vec2::new(8.0, 2.0);
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 0.0);
            add_contents(ui);
        },
    );
}

pub(super) fn action_enabled(action: &str, view: &IngestViewModel) -> bool {
    match action {
        "Uvezi" => view.selected_count() > 0 && !view.command_busy,
        "Očisti" => view.visible_clips().any(|clip| clip.selected),
        "Odaberi sve" => view.visible_clips().next().is_some(),
        "Generiraj postere" => view.total_count() > 0,
        _ => !view.command_busy,
    }
}

pub(super) fn source_kind_label(labels: &IngestDirBrowser, kind: SourceKind) -> &str {
    let index = match kind {
        SourceKind::Local => 0,
        SourceKind::Lan => 1,
        SourceKind::Internet => 2,
    };
    labels
        .kinds
        .get(index)
        .map(String::as_str)
        .unwrap_or_else(|| kind.label())
}

pub(super) fn source_dock_clip_label<'a>(
    contracts: &'a IngestContracts,
    view: &'a IngestViewModel,
) -> &'a str {
    if view.preview_clip_id.is_some() {
        view.current_clip_label()
    } else {
        &contracts.ingest.source_dock.clip_label_fallback
    }
}

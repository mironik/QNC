use super::*;

pub(super) fn dock_style(
    contracts: &IngestContracts,
    theme: &Theme,
) -> qnc_source_dock::TimelineDockStyle {
    qnc_source_dock::TimelineDockStyle {
        fill: theme.panel_alt,
        border: theme.border,
        text: theme.text,
        font_ui: theme.font_ui,
        chrome_row_height: theme.chrome_row_height,
        chrome_control_height: theme.chrome_control_height,
        chrome_pad_x: theme.chrome_pad_x,
        chrome_pad_y: theme.chrome_pad_y,
        header_timeline_gap: contracts.ingest.source_dock.header_timeline_gap,
    }
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

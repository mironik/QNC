use super::*;

pub(super) fn render_preview(
    ui: &mut Ui,
    rect: Rect,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) -> Option<IngestIntent> {
    let chrome = MonitorChrome {
        fill: theme.black,
        border: theme.border,
        muted: theme.text_muted,
        font_size: theme.font_ui,
    };
    let poster = view.shown_clip().and_then(|clip| qnc_monitor_panel::poster(clip.thumb_uri.as_ref(), clip.thumb_image.as_deref()));
    let label = if view.preview_clip_id.is_some() {
        view.current_clip_label()
    } else {
        contracts.ingest.preview.empty_label.as_str()
    };
    let id = egui::Id::new(("qnc-monitor", "ingest-source"));
    qnc_monitor_panel::show(ui, rect, id, chrome, &view.monitor, poster, label);
    None // the monitor gives no intent
}

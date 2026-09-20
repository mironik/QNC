use super::*;

pub(super) fn preview_height(rect: Rect, contracts: &IngestContracts) -> f32 {
    let preview = &contracts.ingest.preview;
    let preview_width = (rect.width() - 32.0).max(240.0);
    let aspect_height = preview_width / preview.aspect_ratio();
    let available = (rect.height() - preview.reserve_below).max(preview.min_height);
    aspect_height.clamp(preview.min_height, available)
}

pub(super) fn render_preview(
    ui: &mut Ui,
    rect: Rect,
    contracts: &IngestContracts,
    theme: &Theme,
    view: &IngestViewModel,
) {
    let chrome = MonitorChrome {
        fill: theme.black,
        border: theme.border,
        muted: theme.text_muted,
        font_size: theme.font_ui,
    };
    let picture = view
        .playback
        .picture
        .as_ref()
        .filter(|_| view.playback.video_visible)
        .map(|picture| MonitorPicture {
            session_id: &picture.header.session_id,
            generation: picture.header.output_generation,
            sequence: picture.header.sequence,
            size: [
                picture.header.width as usize,
                picture.header.height as usize,
            ],
            rgba: &picture.rgba,
        });
    match qnc_monitor::paint_monitor(
        ui,
        rect,
        MonitorSurface {
            id: egui::Id::new(("qnc-monitor", "ingest-source")),
            chrome,
            picture,
            message: view.playback.error.as_deref(),
        },
    ) {
        MonitorPaint::Picture | MonitorPaint::Message => return,
        MonitorPaint::Empty => {}
    }
    if let Some(clip) = view
        .clips
        .iter()
        .find(|c| Some(&c.clip_id) == view.preview_clip_id.as_ref())
    {
        if let (Some(uri), Some(image)) = (&clip.thumb_uri, &clip.thumb_image) {
            if qnc_ui_kit::paint_rgba_image(
                ui,
                rect.shrink(1.0),
                uri,
                image.content_key,
                image.size,
                &image.pixels,
            ) {
                return;
            }
        }
    }
    let label = if view.preview_clip_id.is_some() {
        view.current_clip_label()
    } else {
        contracts.ingest.preview.empty_label.as_str()
    };
    qnc_monitor::paint_placeholder(ui, rect, chrome, label);
}
